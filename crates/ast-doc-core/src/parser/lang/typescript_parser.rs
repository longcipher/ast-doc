//! TypeScript/JavaScript parser using tree-sitter.
//!
//! Detects test markers: `it(`, `test(`, `describe(` in call expressions.
//! Extracts function/class/interface/type/export signatures for Summary mode.

use std::path::Path;

use tree_sitter::{Parser, Tree};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Parser for TypeScript/JavaScript source files using tree-sitter.
#[derive(Debug, Default)]
pub struct TypeScriptParser;

impl TypeScriptParser {
    /// Create a new TypeScript parser.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parse source with tree-sitter, returning the tree.
    fn parse_tree(source: &str) -> Result<Tree, AstDocError> {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
        parser.set_language(&language.into()).map_err(|e| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: format!("Failed to set TypeScript language: {e}"),
        })?;
        parser.parse(source.as_bytes(), None).ok_or_else(|| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: "Failed to parse TypeScript source".to_string(),
        })
    }
}

impl LanguageParser for TypeScriptParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let tree = Self::parse_tree(source)?;
        let root_node = tree.root_node();

        let test_ranges = collect_test_ranges(&root_node, source);
        let summary_ranges = collect_summary_ranges(&root_node, source);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::TypeScript,
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Check if a call expression is a test function call (`it`, `test`, `describe`, `xtest`, `xit`).
fn is_test_call(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.start_byte()..child.end_byte()];
            return matches!(name, "it" | "test" | "describe" | "xtest" | "xit" | "xtest_each");
        }
        if child.kind() == "member_expression" {
            // Handle `it.only(...)` etc.
            let text = &source[child.start_byte()..child.end_byte()];
            return text.starts_with("it.") ||
                text.starts_with("test.") ||
                text.starts_with("describe.");
        }
    }
    false
}

/// Collect byte ranges for test code.
fn collect_test_ranges(root: &tree_sitter::Node<'_>, source: &str) -> Vec<RemovalRange> {
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
        match child.kind() {
            "expression_statement" => {
                // Check if it contains a test call
                let mut ecursor = child.walk();
                for sub in child.children(&mut ecursor) {
                    if sub.kind() == "call_expression" && is_test_call(sub, source) {
                        ranges.push(RemovalRange {
                            start: child.start_byte(),
                            end: child.end_byte(),
                            reason: RemovalReason::TestFunction,
                        });
                        break;
                    }
                }
            }
            "call_expression" => {
                if is_test_call(child, source) {
                    ranges.push(RemovalRange {
                        start: child.start_byte(),
                        end: child.end_byte(),
                        reason: RemovalReason::TestFunction,
                    });
                }
            }
            _ => {
                collect_test_ranges_recursive(&child, source, ranges);
            }
        }
    }
}

/// Collect byte ranges for Summary mode.
fn collect_summary_ranges(root: &tree_sitter::Node<'_>, source: &str) -> Vec<RemovalRange> {
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
            "function_declaration" | "method_definition" | "arrow_function" => {
                if let Some(range) = extract_function_body(child) {
                    ranges.push(range);
                }
            }
            "class_declaration" => {
                // Recurse into class body for methods
                collect_summary_ranges_recursive(&child, source, ranges);
            }
            "expression_statement" => {
                // Check for arrow function assignments: `const fn = () => { ... }`
                let mut ecursor = child.walk();
                for sub in child.children(&mut ecursor) {
                    if sub.kind() == "call_expression" && is_test_call(sub, source) {
                        // Skip test calls
                        break;
                    }
                }
                collect_summary_ranges_recursive(&child, source, ranges);
            }
            _ => {
                collect_summary_ranges_recursive(&child, source, ranges);
            }
        }
    }
}

/// Extract the body range of a function.
fn extract_function_body(node: tree_sitter::Node<'_>) -> Option<RemovalRange> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "statement_block" {
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

    fn parse_ts(source: &str) -> ParsedFile {
        let parser = TypeScriptParser::new();
        parser.parse(source, Path::new("test.ts")).unwrap()
    }

    #[test]
    fn test_ts_parser_creates_three_strategies() {
        let source = "function main() {\n    console.log('hello');\n}\n";
        let parsed = parse_ts(source);
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_ts_parser_full_is_verbatim() {
        let source = "function main() {\n    console.log('hello');\n}\n";
        let parsed = parse_ts(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, source);
    }

    #[test]
    fn test_ts_parser_removes_it_calls() {
        let source = "function add(a: number, b: number): number {\n    return a + b;\n}\n\nit('adds numbers', () => {\n    expect(add(1, 2)).toBe(3);\n});\n";
        let parsed = parse_ts(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("function add"), "should preserve add");
        assert!(!no_tests.contains("it("), "should remove it() call");
    }

    #[test]
    fn test_ts_parser_removes_test_calls() {
        let source = "export function helper(): string {\n    return 'hello';\n}\n\ntest('helper returns hello', () => {\n    expect(helper()).toBe('hello');\n});\n";
        let parsed = parse_ts(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("function helper"), "should preserve helper");
        assert!(!no_tests.contains("test("), "should remove test() call");
    }

    #[test]
    fn test_ts_parser_removes_describe_blocks() {
        let source = "function helper(): string {\n    return 'hello';\n}\n\ndescribe('Helper', () => {\n    it('works', () => {\n        expect(helper()).toBe('hello');\n    });\n});\n";
        let parsed = parse_ts(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("function helper"), "should preserve helper");
        assert!(!no_tests.contains("describe("), "should remove describe block");
    }

    #[test]
    fn test_ts_parser_summary_extracts_signatures() {
        let source = "function add(a: number, b: number): number {\n    return a + b;\n}\n";
        let parsed = parse_ts(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("function add"), "should preserve function");
        assert!(!summary.contains("return a + b"), "should remove body");
        assert!(summary.contains("✂️ implementations omitted"), "should insert marker");
    }

    #[test]
    fn test_ts_parser_language_is_typescript() {
        let source = "function main() {}\n";
        let parsed = parse_ts(source);
        assert_eq!(parsed.language, Language::TypeScript);
    }

    #[test]
    fn test_ts_parser_empty_file() {
        let source = "";
        let parsed = parse_ts(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, "");
    }
}
