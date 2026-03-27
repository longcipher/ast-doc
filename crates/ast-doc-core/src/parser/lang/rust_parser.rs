//! Rust parser using tree-sitter.
//!
//! Detects `#[cfg(test)]` modules and `#[test]` functions for NoTests mode.
//! Extracts function/struct/trait/enum/impl signatures for Summary mode.

use std::path::Path;

use tree_sitter::{Parser, Tree};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Parser for Rust source files using tree-sitter.
#[derive(Debug, Default)]
pub struct RustParser;

impl RustParser {
    /// Create a new Rust parser.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parse source with tree-sitter, returning the tree.
    fn parse_tree(source: &str) -> Result<Tree, AstDocError> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser.set_language(&language.into()).map_err(|e| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: format!("Failed to set Rust language: {e}"),
        })?;
        parser.parse(source.as_bytes(), None).ok_or_else(|| AstDocError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            message: "Failed to parse Rust source".to_string(),
        })
    }
}

impl LanguageParser for RustParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let tree = Self::parse_tree(source)?;
        let root_node = tree.root_node();

        let test_ranges = collect_test_ranges(&root_node, source);
        let summary_ranges = collect_summary_ranges(&root_node, source);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Rust,
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Check if a node has a specific attribute in its preceding siblings.
fn has_attribute(node: tree_sitter::Node<'_>, source: &str, attr_name: &str) -> bool {
    // Check attribute_item children of this node
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_item" {
            let text = &source[child.start_byte()..child.end_byte()];
            if text.contains(attr_name) {
                return true;
            }
        }
    }

    // Check preceding siblings in parent
    if let Some(parent) = node.parent() {
        let mut pcursor = parent.walk();
        for sibling in parent.children(&mut pcursor) {
            if sibling.id() == node.id() {
                break;
            }
            if sibling.kind() == "attribute_item" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                if text.contains(attr_name) {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a module is annotated with `#[cfg(test)]`.
fn is_test_module(node: tree_sitter::Node<'_>, source: &str) -> bool {
    has_attribute(node, source, "cfg(test)")
}

/// Check if a function is annotated with `#[test]`.
///
/// In tree-sitter-rust, `#[test]` is an `attribute_item` sibling that
/// precedes the `function_item`, not a child of it.
fn is_test_function(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        for sibling in parent.children(&mut cursor) {
            if sibling.id() == node.id() {
                break;
            }
            if sibling.kind() == "attribute_item" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                if text == "#[test]" {
                    return true;
                }
            }
        }
    }
    false
}

/// Collect byte ranges for test modules and test functions.
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
            "mod_item" => {
                if is_test_module(child, source) {
                    let start = find_attr_start(&child, source);
                    ranges.push(RemovalRange {
                        start,
                        end: child.end_byte(),
                        reason: RemovalReason::TestModule,
                    });
                    continue;
                }
                collect_test_ranges_recursive(&child, source, ranges);
            }
            "function_item" => {
                if is_test_function(child, source) {
                    let start = find_attr_start(&child, source);
                    ranges.push(RemovalRange {
                        start,
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

/// Find the start byte of attributes preceding a node.
fn find_attr_start(node: &tree_sitter::Node<'_>, source: &str) -> usize {
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        let mut first_attr_start = node.start_byte();

        for sibling in parent.children(&mut cursor) {
            if sibling.id() == node.id() {
                break;
            }
            if sibling.kind() == "attribute_item" && sibling.end_byte() <= node.start_byte() {
                let between = &source[sibling.end_byte()..node.start_byte()];
                if between.trim().is_empty() {
                    first_attr_start = sibling.start_byte();
                }
            }
        }

        return first_attr_start;
    }

    node.start_byte()
}

/// Collect byte ranges for Summary mode: replace implementation bodies.
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
            "function_item" => {
                if is_test_function(child, source) {
                    continue;
                }
                if let Some(range) = extract_implementation_range(child) {
                    ranges.push(range);
                }
            }
            "impl_item" => {
                if let Some(range) = extract_impl_body_range(child) {
                    ranges.push(range);
                }
                collect_summary_ranges_recursive(&child, source, ranges);
            }
            "mod_item" => {
                if is_test_module(child, source) {
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

/// Extract the implementation body range of a function (the `block` node).
fn extract_implementation_range(node: tree_sitter::Node<'_>) -> Option<RemovalRange> {
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

/// Extract the body range of an impl block (the `declaration_list` node).
fn extract_impl_body_range(node: tree_sitter::Node<'_>) -> Option<RemovalRange> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "declaration_list" {
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

    fn parse_rust(source: &str) -> ParsedFile {
        let parser = RustParser::new();
        parser.parse(source, Path::new("test.rs")).unwrap()
    }

    #[test]
    fn test_rust_parser_creates_three_strategies() {
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let parsed = parse_rust(source);
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_rust_parser_full_is_verbatim() {
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let parsed = parse_rust(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, source);
    }

    #[test]
    fn test_rust_parser_detects_cfg_test_module() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_add() {\n        assert_eq!(add(1, 2), 3);\n    }\n}\n";
        let parsed = parse_rust(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(!no_tests.contains("#[cfg(test)]"), "NoTests should remove #[cfg(test)] module");
        assert!(!no_tests.contains("test_add"), "NoTests should remove test function");
        assert!(no_tests.contains("pub fn add"), "NoTests should preserve non-test code");
    }

    #[test]
    fn test_rust_parser_removes_test_function() {
        let source = "pub fn helper() -> i32 {\n    42\n}\n\n#[test]\nfn test_helper() {\n    assert_eq!(helper(), 42);\n}\n";
        let parsed = parse_rust(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("pub fn helper"), "should preserve helper");
        assert!(!no_tests.contains("test_helper"), "should remove test function");
    }

    #[test]
    fn test_rust_parser_summary_extracts_signatures() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let parsed = parse_rust(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("pub fn add(a: i32, b: i32) -> i32"), "should preserve signature");
        assert!(!summary.contains("a + b"), "should remove body");
        assert!(summary.contains("✂️ implementations omitted"), "should insert marker");
    }

    #[test]
    fn test_rust_parser_summary_handles_struct() {
        let source = "#[derive(Debug)]\npub struct Point {\n    x: f64,\n    y: f64,\n}\n";
        let parsed = parse_rust(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("struct Point"), "should contain struct");
    }

    #[test]
    fn test_rust_parser_no_tests_fewer_tokens_than_full() {
        let source = "pub fn lib() -> i32 {\n    42\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_lib() {\n        assert_eq!(lib(), 42);\n    }\n}\n";
        let parsed = parse_rust(source);
        let full_tokens = parsed.strategies_data[&OutputStrategy::Full].token_count;
        let no_tests_tokens = parsed.strategies_data[&OutputStrategy::NoTests].token_count;
        assert!(
            no_tests_tokens < full_tokens,
            "NoTests ({no_tests_tokens}) should have fewer tokens than Full ({full_tokens})"
        );
    }

    #[test]
    fn test_rust_parser_path_stored() {
        let source = "fn main() {}\n";
        let parser = RustParser::new();
        let parsed = parser.parse(source, Path::new("src/main.rs")).unwrap();
        assert_eq!(parsed.path, Path::new("src/main.rs"));
    }

    #[test]
    fn test_rust_parser_language_is_rust() {
        let source = "fn main() {}\n";
        let parsed = parse_rust(source);
        assert_eq!(parsed.language, Language::Rust);
    }

    #[test]
    fn test_rust_parser_empty_file() {
        let source = "";
        let parsed = parse_rust(source);
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].content, "");
        assert_eq!(parsed.strategies_data[&OutputStrategy::Full].token_count, 0);
    }

    #[test]
    fn test_rust_parser_multiple_test_functions() {
        let source = "pub fn add(a: i32, b: i32) -> i32 { a + b }\npub fn sub(a: i32, b: i32) -> i32 { a - b }\n\n#[test]\nfn test_add() { assert_eq!(add(1, 2), 3); }\n\n#[test]\nfn test_sub() { assert_eq!(sub(3, 1), 2); }\n";
        let parsed = parse_rust(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("pub fn add"), "should preserve add");
        assert!(no_tests.contains("pub fn sub"), "should preserve sub");
        assert!(!no_tests.contains("test_add"), "should remove test_add");
        assert!(!no_tests.contains("test_sub"), "should remove test_sub");
    }

    #[test]
    fn test_rust_parser_nested_test_module() {
        let source = "pub fn helper() {}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_helper() {\n        helper();\n    }\n}\n";
        let parsed = parse_rust(source);
        let no_tests = &parsed.strategies_data[&OutputStrategy::NoTests].content;
        assert!(no_tests.contains("pub fn helper"));
        assert!(!no_tests.contains("test_helper"));
    }

    #[test]
    fn test_rust_parser_impl_block_summary() {
        let source = "pub struct Counter {\n    count: u32,\n}\n\nimpl Counter {\n    pub fn new() -> Self {\n        Self { count: 0 }\n    }\n\n    pub fn increment(&mut self) {\n        self.count += 1;\n    }\n}\n";
        let parsed = parse_rust(source);
        let summary = &parsed.strategies_data[&OutputStrategy::Summary].content;
        assert!(summary.contains("impl Counter"), "should contain impl");
        assert!(summary.contains("struct Counter"), "should contain struct");
    }

    use proptest::prelude::*;

    fn rust_source_strategy() -> impl Strategy<Value = String> {
        (
            proptest::collection::vec(proptest::string::string_regex("[a-z_]{1,10}").unwrap(), 1..5),
            proptest::collection::vec(proptest::string::string_regex("[a-z0-9_ +\\-*/;(){}\n\t]{0,50}").unwrap(), 1..5),
            proptest::bool::ANY,
        ).prop_map(|(fn_names, bodies, add_test_module)| {
            let mut source = String::new();
            for (i, name) in fn_names.iter().enumerate() {
                let body = &bodies[i % bodies.len()];
                source.push_str(&format!("pub fn {name}() {{\n    {body}\n}}\n\n"));
            }
            if add_test_module {
                source.push_str("#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_something() {\n        assert!(true);\n    }\n}\n");
            }
            source
        })
    }

    /// Strip known marker strings from content to verify that remaining
    /// characters form a subsequence of the original source.
    fn strip_markers(text: &str) -> String {
        let markers = ["// ✂️ test module omitted\n", "// ✂️ implementations omitted"];
        let mut result = text.to_string();
        for marker in &markers {
            result = result.replace(marker, "");
        }
        result
    }

    /// Check if `candidate` is a subsequence of `source`
    /// (all characters appear in the same relative order).
    fn is_subsequence(source: &str, candidate: &str) -> bool {
        let mut source_iter = source.chars();
        let mut src_char = source_iter.next();
        for ch in candidate.chars() {
            loop {
                match src_char {
                    Some(s) if s == ch => {
                        src_char = source_iter.next();
                        break;
                    }
                    Some(_) => {
                        src_char = source_iter.next();
                    }
                    None => return false,
                }
            }
        }
        true
    }

    proptest! {
        #[test]
        fn parser_content_subset_invariant(source in rust_source_strategy()) {
            let parsed = parse_rust(&source);
            for strategy in [OutputStrategy::Full, OutputStrategy::NoTests, OutputStrategy::Summary] {
                if let Some(data) = parsed.strategies_data.get(&strategy) {
                    let stripped = strip_markers(&data.content);
                    prop_assert!(
                        is_subsequence(&source, &stripped),
                        "strategy {strategy}: stripped content is not a subsequence of source.\n\
                         source len={}, stripped len={}",
                        source.len(),
                        stripped.len(),
                    );
                }
            }
        }
    }
}
