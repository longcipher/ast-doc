//! Go parser using tree-sitter.
//!
//! Detects test markers: `func Test*`, `func Benchmark*`, `_test.go` suffix.
//! Extracts function/method/struct/interface signatures for Summary mode.

use std::path::Path;

use tree_sitter::{Parser, Tree};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Parser for Go source files using tree-sitter.
#[derive(Debug, Default)]
pub struct GoParser;

impl GoParser {
    /// Create a new Go parser.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse source with tree-sitter, returning the tree.
    fn parse_tree(&self, source: &str) -> Result<Tree, AstDocError> {
        let mut parser = Parser::new();
        let language = tree_sitter_go::LANGUAGE;
        parser.set_language(&language.into()).map_err(|e| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: format!("Failed to set Go language: {e}"),
        })?;
        parser.parse(source.as_bytes(), None).ok_or_else(|| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: "Failed to parse Go source".to_string(),
        })
    }
}

impl LanguageParser for GoParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let tree = self.parse_tree(source)?;
        let root_node = tree.root_node();

        let is_test_file = path.to_string_lossy().ends_with("_test.go");

        let test_ranges = collect_test_ranges(&root_node, source, is_test_file);
        let summary_ranges = collect_summary_ranges(&root_node, source, is_test_file);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Go,
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Check if a function declaration name starts with `Test` or `Benchmark`.
fn is_test_function(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.start_byte()..child.end_byte()];
            return name.starts_with("Test") || name.starts_with("Benchmark");
        }
    }
    false
}

/// Collect byte ranges for test code.
fn collect_test_ranges(
    root: &tree_sitter::Node<'_>,
    source: &str,
    is_test_file: bool,
) -> Vec<RemovalRange> {
    if is_test_file {
        // Entire file is a test file — mark the whole source
        return vec![RemovalRange {
            start: 0,
            end: source.len(),
            reason: RemovalReason::TestModule,
        }];
    }

    let mut ranges = Vec::new();
    collect_test_ranges_recursive(root, source, &mut ranges);
    ranges
}

fn collect_test_ranges_recursive(
    node: &tree_sitter::Node<'_>,
    source: &str,
    ranges: &mut Vec<RemovalRange>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declaration" && is_test_function(child, source) {
            ranges.push(RemovalRange {
                start: child.start_byte(),
                end: child.end_byte(),
                reason: RemovalReason::TestFunction,
            });
        } else {
            collect_test_ranges_recursive(&child, source, ranges);
        }
    }
}

/// Collect byte ranges for Summary mode.
fn collect_summary_ranges(
    root: &tree_sitter::Node<'_>,
    source: &str,
    is_test_file: bool,
) -> Vec<RemovalRange> {
    if is_test_file {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    collect_summary_ranges_recursive(root, source, &mut ranges);
    ranges
}

fn collect_summary_ranges_recursive(
    node: &tree_sitter::Node<'_>,
    source: &str,
    ranges: &mut Vec<RemovalRange>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" | "method_declaration" => {
                if is_test_function(child, source) {
                    continue;
                }
                if let Some(range) = extract_function_body(child) {
                    ranges.push(range);
                }
            }
            "type_declaration" => {
                // struct and interface declarations — keep as-is, no body to strip
            }
            _ => {
                collect_summary_ranges_recursive(&child, source, ranges);
            }
        }
    }
}

/// Extract the body range of a function (the `block` node).
fn extract_function_body(node: tree_sitter::Node<'_>) -> Option<RemovalRange> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "block" {
            return Some(RemovalRange {
                start: child.start_byte(),
                end: child.end_byte(),
                reason: RemovalReason::Implementation,
            });
        }
    }
    None
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::OutputStrategy;

    fn parse_go(source: &str) -> ParsedFile {
        let parser = GoParser::new();
        parser.parse(source, Path::new("main.go")).unwrap()
    }

    #[test]
    fn test_go_parser_creates_three_strategies() {
        let source = "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let parsed = parse_go(source);
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_go_parser_full_is_verbatim() {
        let source = "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let parsed = parse_go(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, source);
    }

    #[test]
    fn test_go_parser_removes_test_function() {
        let source = "package main\n\nfunc Add(a, b int) int {\n\treturn a + b\n}\n\nfunc TestAdd(t *testing.T) {\n\tif Add(1, 2) != 3 {\n\t\tt.Fatal(\"wrong\")\n\t}\n}\n";
        let parsed = parse_go(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("func Add"), "should preserve Add");
        assert!(!no_tests.contains("TestAdd"), "should remove TestAdd");
    }

    #[test]
    fn test_go_parser_removes_test_file_entirely() {
        let source = "package main\n\nfunc TestSomething(t *testing.T) {\n\t// test\n}\n";
        let parser = GoParser::new();
        let parsed = parser.parse(source, Path::new("main_test.go")).unwrap();
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("✂️ test module omitted"), "should mark as omitted");
    }

    #[test]
    fn test_go_parser_summary_extracts_signatures() {
        let source = "package main\n\nfunc Add(a, b int) int {\n\treturn a + b\n}\n";
        let parsed = parse_go(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("func Add(a, b int) int"), "should preserve signature");
        assert!(!summary.contains("return a + b"), "should remove body");
        assert!(summary.contains("✂️ implementations omitted"), "should insert marker");
    }

    #[test]
    fn test_go_parser_language_is_go() {
        let source = "package main\n";
        let parsed = parse_go(source);
        assert_eq!(parsed.language, Language::Go);
    }

    #[test]
    fn test_go_parser_empty_file() {
        let source = "";
        let parsed = parse_go(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, "");
    }
}
