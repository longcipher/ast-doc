//! Python parser using tree-sitter.
//!
//! Detects test markers: `def test_*`, `class Test*`, `@pytest` decorators.
//! Extracts function/class signatures for Summary mode.

use std::path::Path;

use tree_sitter::{Parser, Tree};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Parser for Python source files using tree-sitter.
#[derive(Debug, Default)]
pub struct PythonParser;

impl PythonParser {
    /// Create a new Python parser.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parse source with tree-sitter, returning the tree.
    fn parse_tree(source: &str) -> Result<Tree, AstDocError> {
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        parser.set_language(&language.into()).map_err(|e| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: format!("Failed to set Python language: {e}"),
        })?;
        parser.parse(source.as_bytes(), None).ok_or_else(|| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: "Failed to parse Python source".to_string(),
        })
    }
}

impl LanguageParser for PythonParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let tree = Self::parse_tree(source)?;
        let root_node = tree.root_node();

        let test_ranges = collect_test_ranges(&root_node, source);
        let summary_ranges = collect_summary_ranges(&root_node, source);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Python,
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Check if a function name starts with `test_`.
fn is_test_function_name(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.start_byte()..child.end_byte()];
            return name.starts_with("test_");
        }
    }
    false
}

/// Check if a class name starts with `Test`.
fn is_test_class_name(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.start_byte()..child.end_byte()];
            return name.starts_with("Test");
        }
    }
    false
}

/// Check if a `decorated_definition` has a pytest decorator.
fn has_pytest_decorator(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            let text = &source[child.start_byte()..child.end_byte()];
            if text.contains("pytest") {
                return true;
            }
        }
    }
    false
}

/// Check if a `decorated_definition` wraps a test function or class.
fn is_test_decorated(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if has_pytest_decorator(node, source) {
        return true;
    }
    // Check if the wrapped definition is a test function/class
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition" && is_test_function_name(child, source) {
            return true;
        }
        if child.kind() == "class_definition" && is_test_class_name(child, source) {
            return true;
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
            "function_definition" => {
                if is_test_function_name(child, source) {
                    ranges.push(RemovalRange {
                        start: child.start_byte(),
                        end: child.end_byte(),
                        reason: RemovalReason::TestFunction,
                    });
                }
            }
            "class_definition" => {
                if is_test_class_name(child, source) {
                    ranges.push(RemovalRange {
                        start: child.start_byte(),
                        end: child.end_byte(),
                        reason: RemovalReason::TestModule,
                    });
                } else {
                    // Recurse into non-test class bodies
                    collect_test_ranges_recursive(&child, source, ranges);
                }
            }
            "decorated_definition" => {
                if is_test_decorated(child, source) {
                    ranges.push(RemovalRange {
                        start: child.start_byte(),
                        end: child.end_byte(),
                        reason: RemovalReason::TestFunction,
                    });
                } else {
                    // Not a test decoration, recurse into the definition
                    collect_test_ranges_recursive(&child, source, ranges);
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
            "function_definition" => {
                if is_test_function_name(child, source) {
                    continue;
                }
                if let Some(range) = extract_function_body(child) {
                    ranges.push(range);
                }
            }
            "class_definition" => {
                if is_test_class_name(child, source) {
                    continue;
                }
                // Recurse into class body for methods
                collect_summary_ranges_recursive(&child, source, ranges);
            }
            "decorated_definition" => {
                if is_test_decorated(child, source) {
                    continue;
                }
                collect_summary_ranges_recursive(&child, source, ranges);
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

    fn parse_python(source: &str) -> ParsedFile {
        let parser = PythonParser::new();
        parser.parse(source, Path::new("test.py")).unwrap()
    }

    #[test]
    fn test_python_parser_creates_three_strategies() {
        let source = "def main():\n    pass\n";
        let parsed = parse_python(source);
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_python_parser_full_is_verbatim() {
        let source = "def main():\n    pass\n";
        let parsed = parse_python(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, source);
    }

    #[test]
    fn test_python_parser_removes_test_function() {
        let source =
            "def add(a, b):\n    return a + b\n\ndef test_add():\n    assert add(1, 2) == 3\n";
        let parsed = parse_python(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("def add"), "should preserve add");
        assert!(!no_tests.contains("test_add"), "should remove test function");
    }

    #[test]
    fn test_python_parser_removes_test_class() {
        let source = "def helper():\n    pass\n\nclass TestHelper:\n    def test_something(self):\n        pass\n";
        let parsed = parse_python(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("def helper"), "should preserve helper");
        assert!(!no_tests.contains("TestHelper"), "should remove Test class");
    }

    #[test]
    fn test_python_parser_removes_pytest_decorated() {
        let source = "import pytest\n\ndef helper():\n    pass\n\n@pytest.fixture\ndef sample_data():\n    return [1, 2, 3]\n";
        let parsed = parse_python(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("def helper"), "should preserve helper");
        assert!(!no_tests.contains("sample_data"), "should remove pytest decorated function");
    }

    #[test]
    fn test_python_parser_summary_extracts_signatures() {
        let source = "def add(a, b):\n    return a + b\n";
        let parsed = parse_python(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("def add(a, b)"), "should preserve signature");
        assert!(!summary.contains("return a + b"), "should remove body");
        assert!(summary.contains("✂️ implementations omitted"), "should insert marker");
    }

    #[test]
    fn test_python_parser_preserves_non_test_code() {
        let source = "class MyClass:\n    def method(self):\n        pass\n";
        let parsed = parse_python(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert_eq!(no_tests, source);
    }

    #[test]
    fn test_python_parser_language_is_python() {
        let source = "def main():\n    pass\n";
        let parsed = parse_python(source);
        assert_eq!(parsed.language, Language::Python);
    }

    #[test]
    fn test_python_parser_empty_file() {
        let source = "";
        let parsed = parse_python(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, "");
    }
}
