//! Generic parser for any language supported by `tree-sitter-language-pack`.
//!
//! Uses the `process()` API to extract structured code intelligence
//! (functions, classes, imports, etc.) and generates Full/NoTests/Summary
//! strategy variants without requiring language-specific tree-sitter queries.

use std::path::Path;

use tree_sitter_language_pack::{ProcessConfig, StructureKind};

use crate::{
    error::AstDocError,
    parser::{
        Language, LanguageParser, ParsedFile,
        strategy::{self, RemovalRange, RemovalReason},
    },
};

/// Generic parser for languages beyond the core 5.
///
/// Uses `tree-sitter-language-pack`'s `process()` API for structure extraction.
#[derive(Debug)]
pub struct GenericParser {
    /// The language name as recognized by `tree-sitter-language-pack`.
    language_name: String,
}

impl GenericParser {
    /// Create a new generic parser for the given language name.
    #[must_use]
    pub fn new(language_name: &str) -> Self {
        Self { language_name: language_name.to_string() }
    }
}

impl LanguageParser for GenericParser {
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError> {
        let config = ProcessConfig::new(&self.language_name).all();

        let result = tree_sitter_language_pack::process(source, &config).map_err(|e| {
            AstDocError::Parse {
                path: path.to_path_buf(),
                message: format!("Failed to process {} source: {e}", self.language_name),
            }
        })?;

        let test_ranges = collect_test_ranges_from_structure(&result, source);
        let summary_ranges = collect_summary_ranges_from_structure(&result, source);

        let strategies_data = strategy::build_strategies(source, &test_ranges, &summary_ranges);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Generic(self.language_name.clone()),
            source: source.to_string(),
            strategies_data,
        })
    }
}

/// Heuristic: check if a function/method name indicates a test.
fn is_test_name(name: &str) -> bool {
    // Common test naming conventions across languages
    name.starts_with("test_") ||           // Python, Rust
    name.starts_with("Test") ||             // Go
    name.starts_with("test") && name.len() > 4 && name.as_bytes()[4].is_ascii_uppercase() || // Java/Kotlin testXxx
    name.starts_with("it_") ||
    name.starts_with("should_") ||          // BDD style
    name.starts_with("bench_") ||           // Rust benchmarks
    name.starts_with("Benchmark") // Go benchmarks
}

/// Heuristic: check if a structure item looks like a test-related node.
fn is_test_structure_item(item: &tree_sitter_language_pack::StructureItem) -> bool {
    match &item.name {
        Some(name) => is_test_name(name),
        None => false,
    }
}

/// Collect byte ranges for test functions/classes from the `process()` result.
fn collect_test_ranges_from_structure(
    result: &tree_sitter_language_pack::ProcessResult,
    _source: &str,
) -> Vec<RemovalRange> {
    let mut ranges = Vec::new();

    for item in &result.structure {
        if is_test_structure_item(item) {
            ranges.push(RemovalRange {
                start: item.span.start_byte,
                end: item.span.end_byte,
                reason: match item.kind {
                    StructureKind::Function | StructureKind::Method => RemovalReason::TestFunction,
                    StructureKind::Class | StructureKind::Struct => RemovalReason::TestModule,
                    _ => RemovalReason::TestFunction,
                },
            });
        }
    }

    ranges
}

/// Collect byte ranges for Summary mode: implementation bodies of non-test functions.
fn collect_summary_ranges_from_structure(
    result: &tree_sitter_language_pack::ProcessResult,
    source: &str,
) -> Vec<RemovalRange> {
    let mut ranges = Vec::new();

    for item in &result.structure {
        if is_test_structure_item(item) {
            continue;
        }

        match item.kind {
            StructureKind::Function | StructureKind::Method => {
                // For functions/methods, try to find the body (the `{...}` block).
                // The structure item span covers the entire function.
                // We want to keep the signature and replace the body.
                if let Some(range) = extract_body_range(source, &item.span) {
                    ranges.push(range);
                }
            }
            StructureKind::Class |
            StructureKind::Struct |
            StructureKind::Interface |
            StructureKind::Enum |
            StructureKind::Module |
            StructureKind::Trait |
            StructureKind::Impl |
            StructureKind::Namespace |
            StructureKind::Other(_) => {
                // Keep these as-is in summary mode (just the declaration)
            }
        }
    }

    ranges
}

/// Try to find the implementation body range within a structure item.
///
/// Looks for the first `{` after the declaration keyword and matches it
/// to the closing `}` to identify the body range.
fn extract_body_range(
    source: &str,
    span: &tree_sitter_language_pack::Span,
) -> Option<RemovalRange> {
    let start = span.start_byte;
    let end = span.end_byte;
    if end > source.len() || start >= end {
        return None;
    }

    let item_text = &source[start..end];

    // Find the first `{` that likely starts the body
    // Skip the first line (signature) and find the body
    let body_start_in_item = find_body_open_brace(item_text)?;

    let abs_body_start = start + body_start_in_item;
    let abs_body_end = start + find_matching_brace(item_text, body_start_in_item)?;

    if abs_body_end <= abs_body_start {
        return None;
    }

    Some(RemovalRange {
        start: abs_body_start,
        end: abs_body_end + 1, // include the closing brace
        reason: RemovalReason::Implementation,
    })
}

/// Find the position of the opening `{` that starts a function body.
/// Skips generic parameters, arguments, and return types that may contain `{` in type expressions.
fn find_body_open_brace(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    let mut found_sig_end = false;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth_paren += 1,
            b')' => depth_paren -= 1,
            b'<' => depth_angle += 1,
            b'>' => depth_angle -= 1,
            b':' if !found_sig_end => {
                // Could be return type separator or label
            }
            b'{' if depth_paren == 0 && depth_angle <= 0 => {
                return Some(i);
            }
            b'\n' if depth_paren == 0 && depth_angle <= 0 => {
                found_sig_end = true;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Find the matching closing `}` for an opening `{` at the given position.
fn find_matching_brace(text: &str, open_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;

    for (i, &byte) in bytes.iter().enumerate().skip(open_pos) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::OutputStrategy;

    fn get_strategy_content<'a>(parsed: &'a ParsedFile, strategy: &OutputStrategy) -> &'a str {
        parsed.strategies_data.get(strategy).map_or("", |s| s.content.as_str())
    }

    #[test]
    fn test_generic_parser_full_is_verbatim() {
        if !tree_sitter_language_pack::has_language("rust") {
            return;
        }
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let parser = GenericParser::new("rust");
        let parsed = parser.parse(source, Path::new("test.rs")).unwrap();
        assert_eq!(get_strategy_content(&parsed, &OutputStrategy::Full), source);
    }

    #[test]
    fn test_generic_parser_creates_three_strategies() {
        if !tree_sitter_language_pack::has_language("rust") {
            return;
        }
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let parser = GenericParser::new("rust");
        let parsed = parser.parse(source, Path::new("test.rs")).unwrap();
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(parsed.strategies_data.contains_key(&OutputStrategy::Summary));
    }

    #[test]
    fn test_generic_parser_language_stored() {
        if !tree_sitter_language_pack::has_language("java") {
            return;
        }
        let source = "fn main() {}\n";
        let parser = GenericParser::new("java");
        let parsed = parser.parse(source, Path::new("Main.java")).unwrap();
        assert_eq!(parsed.language, Language::Generic("java".to_string()));
    }

    #[test]
    fn test_is_test_name() {
        assert!(is_test_name("test_add"));
        assert!(is_test_name("TestAdd"));
        assert!(is_test_name("should_work"));
        assert!(is_test_name("bench_sort"));
        assert!(is_test_name("BenchmarkSort"));
        assert!(!is_test_name("add"));
        assert!(!is_test_name("main"));
        assert!(!is_test_name("get_test_value")); // contains "test" but doesn't match pattern
    }

    #[test]
    fn test_find_matching_brace() {
        let text = "{ body }";
        assert_eq!(find_matching_brace(text, 0), Some(7));

        let text = "{ { nested } }";
        assert_eq!(find_matching_brace(text, 0), Some(13));

        let text = "{ unclosed";
        assert_eq!(find_matching_brace(text, 0), None);
    }

    #[test]
    fn test_generic_parser_with_python() {
        if !tree_sitter_language_pack::has_language("python") {
            return;
        }
        let source = "def hello():\n    pass\n";
        let parser = GenericParser::new("python");
        let parsed = parser.parse(source, Path::new("test.py")).unwrap();
        assert_eq!(get_strategy_content(&parsed, &OutputStrategy::Full), source);
    }

    #[test]
    fn test_generic_parser_empty_source() {
        if !tree_sitter_language_pack::has_language("java") {
            return;
        }
        let source = "";
        let parser = GenericParser::new("java");
        let parsed = parser.parse(source, Path::new("Empty.java")).unwrap();
        assert_eq!(get_strategy_content(&parsed, &OutputStrategy::Full), "");
    }

    proptest::proptest! {
        #[test]
        fn test_generic_parser_full_matches_source(source in "[a-zA-Z0-9 {}();\n\t]{0,200}") {
            if tree_sitter_language_pack::has_language("c") {
                let parser = GenericParser::new("c");
                let parsed = parser.parse(&source, Path::new("test.c")).unwrap();
                let full_data = parsed.strategies_data.get(&OutputStrategy::Full);
                proptest::prop_assert!(full_data.is_some());
                proptest::prop_assert_eq!(&full_data.unwrap().content, &source);
            }
        }
    }
}
