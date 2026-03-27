//! C parser using tree-sitter.
//!
//! Basic function and struct extraction only. No standard test markers.
//! Feature-gated behind `lang-c`.

use std::path::Path;

use tree_sitter::{Parser, Tree};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Parser for C source files using tree-sitter.
#[derive(Debug, Default)]
pub struct CParser;

impl CParser {
    /// Create a new C parser.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse source with tree-sitter, returning the tree.
    fn parse_tree(&self, source: &str) -> Result<Tree, AstDocError> {
        let mut parser = Parser::new();
        let language = tree_sitter_c::LANGUAGE;
        parser.set_language(&language.into()).map_err(|e| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: format!("Failed to set C language: {e}"),
        })?;
        parser.parse(source.as_bytes(), None).ok_or_else(|| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: "Failed to parse C source".to_string(),
        })
    }
}

impl LanguageParser for CParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let tree = self.parse_tree(source)?;
        let root_node = tree.root_node();

        // C has no standard test markers
        let test_ranges = Vec::new();
        let summary_ranges = collect_summary_ranges(&root_node);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::C,
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Collect byte ranges for Summary mode: function bodies.
fn collect_summary_ranges(root: &tree_sitter::Node<'_>) -> Vec<RemovalRange> {
    let mut ranges = Vec::new();
    collect_summary_ranges_recursive(root, &mut ranges);
    ranges
}

fn collect_summary_ranges_recursive(node: &tree_sitter::Node<'_>, ranges: &mut Vec<RemovalRange>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(range) = extract_function_body(child) {
                    ranges.push(range);
                }
            }
            _ => {
                collect_summary_ranges_recursive(&child, ranges);
            }
        }
    }
}

/// Extract the body range of a function (the `compound_statement` node).
fn extract_function_body(node: tree_sitter::Node<'_>) -> Option<RemovalRange> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "compound_statement" {
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

    fn parse_c(source: &str) -> ParsedFile {
        let parser = CParser::new();
        parser.parse(source, Path::new("main.c")).unwrap()
    }

    #[test]
    fn test_c_parser_creates_three_strategies() {
        let source = "int main() {\n    return 0;\n}\n";
        let parsed = parse_c(source);
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_c_parser_full_is_verbatim() {
        let source = "int main() {\n    return 0;\n}\n";
        let parsed = parse_c(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, source);
    }

    #[test]
    fn test_c_parser_no_tests_equals_full() {
        // C has no test markers, so NoTests should equal Full
        let source = "int main() {\n    return 0;\n}\n";
        let parsed = parse_c(source);
        assert_eq!(
            parsed.strategies_data[&OutputStrategy::NoTests].content,
            source,
            "C has no test markers, NoTests should equal Full"
        );
    }

    #[test]
    fn test_c_parser_summary_extracts_signatures() {
        let source = "int add(int a, int b) {\n    return a + b;\n}\n";
        let parsed = parse_c(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("int add(int a, int b)"), "should preserve signature");
        assert!(!summary.contains("return a + b"), "should remove body");
        assert!(summary.contains("✂️ implementations omitted"), "should insert marker");
    }

    #[test]
    fn test_c_parser_language_is_c() {
        let source = "int main() { return 0; }\n";
        let parsed = parse_c(source);
        assert_eq!(parsed.language, Language::C);
    }

    #[test]
    fn test_c_parser_empty_file() {
        let source = "";
        let parsed = parse_c(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, "");
    }
}
