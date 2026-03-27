//! Strategy engine: byte-range slicing for NoTests and Summary modes.
//!
//! Given a source string and sorted removal ranges, produces transformed
//! output for each strategy variant using byte-range slicing.

use crate::config::OutputStrategy;

/// Reason a byte range is marked for removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemovalReason {
    /// A `#[cfg(test)]` module (Rust) or equivalent test module.
    TestModule,
    /// A test function/method (e.g., `#[test]`, `def test_*`, `func Test*`).
    TestFunction,
    /// Implementation body to be replaced with a marker (Summary mode).
    Implementation,
}

/// A byte range in source to be removed or replaced.
#[derive(Debug, Clone)]
pub struct RemovalRange {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
    /// Why this range is being removed.
    pub reason: RemovalReason,
}

/// Apply the NoTests strategy: remove test ranges, insert omission markers.
///
/// Returns the transformed source with test code replaced by markers.
#[must_use]
pub fn apply_no_tests(source: &str, ranges: &[RemovalRange]) -> String {
    let test_ranges: Vec<&RemovalRange> = ranges
        .iter()
        .filter(|r| {
            r.reason == RemovalReason::TestModule || r.reason == RemovalReason::TestFunction
        })
        .collect();

    if test_ranges.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let mut last_end = 0;

    for range in &test_ranges {
        if range.start > last_end {
            result.push_str(&source[last_end..range.start]);
        }
        result.push_str("// ✂️ test module omitted\n");
        last_end = range.end;
    }

    if last_end < source.len() {
        result.push_str(&source[last_end..]);
    }

    result
}

/// Apply the Summary strategy: extract signatures only, replace bodies with markers.
///
/// Returns the transformed source with implementations replaced by markers.
#[must_use]
pub fn apply_summary(source: &str, ranges: &[RemovalRange]) -> String {
    let impl_ranges: Vec<&RemovalRange> =
        ranges.iter().filter(|r| r.reason == RemovalReason::Implementation).collect();

    if impl_ranges.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let mut last_end = 0;

    for range in &impl_ranges {
        if range.start > last_end {
            result.push_str(&source[last_end..range.start]);
        }
        result.push_str("// ✂️ implementations omitted");
        last_end = range.end;
    }

    if last_end < source.len() {
        result.push_str(&source[last_end..]);
    }

    result
}

/// Compute the token count of a string using `tiktoken-rs`.
#[must_use]
pub fn compute_token_count(content: &str) -> usize {
    tiktoken_rs::cl100k_base().map_or(0, |bpe| bpe.encode_with_special_tokens(content).len())
}

/// Build strategy data for all three output modes.
///
/// This is the main entry point called by language parsers. Given the full
/// source and computed removal ranges, produces `StrategyData` for each mode.
#[must_use]
pub fn build_strategies(
    source: &str,
    test_ranges: &[RemovalRange],
    summary_ranges: &[RemovalRange],
) -> std::collections::HashMap<OutputStrategy, crate::parser::StrategyData> {
    use std::collections::HashMap;

    let mut all_ranges = Vec::new();
    all_ranges.extend_from_slice(test_ranges);
    all_ranges.extend_from_slice(summary_ranges);

    // Full: verbatim source
    let full_content = source.to_string();
    let full_tokens = compute_token_count(&full_content);

    // NoTests: remove test ranges
    let no_tests_content = apply_no_tests(source, &all_ranges);
    let no_tests_tokens = compute_token_count(&no_tests_content);

    // Summary: extract signatures only
    let summary_content = apply_summary(source, &all_ranges);
    let summary_tokens = compute_token_count(&summary_content);

    let mut map = HashMap::new();
    map.insert(
        OutputStrategy::Full,
        crate::parser::StrategyData { content: full_content, token_count: full_tokens },
    );
    map.insert(
        OutputStrategy::NoTests,
        crate::parser::StrategyData { content: no_tests_content, token_count: no_tests_tokens },
    );
    map.insert(
        OutputStrategy::Summary,
        crate::parser::StrategyData { content: summary_content, token_count: summary_tokens },
    );
    map
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_no_tests_empty_ranges() {
        let source = "fn main() {}\n";
        let result = apply_no_tests(source, &[]);
        assert_eq!(result, source);
    }

    #[test]
    fn test_apply_no_tests_removes_test_module() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_add() {\n        assert_eq!(add(1, 2), 3);\n    }\n}\n";
        let ranges = vec![RemovalRange {
            start: source.find("#[cfg(test)]").unwrap(),
            end: source.len(),
            reason: RemovalReason::TestModule,
        }];
        let result = apply_no_tests(source, &ranges);
        assert!(!result.contains("#[cfg(test)]"));
        assert!(!result.contains("test_add"));
        assert!(result.contains("pub fn add"));
        assert!(result.contains("✂️ test module omitted"));
    }

    #[test]
    fn test_apply_no_tests_preserves_non_test_code() {
        let source = "pub fn lib() -> i32 {\n    42\n}\n";
        let result = apply_no_tests(source, &[]);
        assert_eq!(result, source);
    }

    #[test]
    fn test_apply_summary_empty_ranges() {
        let source = "fn main() {}\n";
        let result = apply_summary(source, &[]);
        assert_eq!(result, source);
    }

    #[test]
    fn test_apply_summary_replaces_bodies() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let body_start = source.find('{').unwrap();
        let ranges = vec![RemovalRange {
            start: body_start,
            end: source.len() - 1, // exclude trailing newline for precision
            reason: RemovalReason::Implementation,
        }];
        let result = apply_summary(source, &ranges);
        assert!(result.contains("pub fn add(a: i32, b: i32) -> i32"));
        assert!(result.contains("✂️ implementations omitted"));
        assert!(!result.contains("a + b"));
    }

    #[test]
    fn test_compute_token_count() {
        let count = compute_token_count("fn main() {}");
        assert!(count > 0, "token count should be > 0");
    }

    #[test]
    fn test_compute_token_count_empty() {
        let count = compute_token_count("");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_build_strategies_produces_three_variants() {
        let source = "pub fn lib() -> i32 {\n    42\n}\n";
        let strategies = build_strategies(source, &[], &[]);
        assert!(strategies.contains_key(&OutputStrategy::Full));
        assert!(strategies.contains_key(&OutputStrategy::NoTests));
        assert!(strategies.contains_key(&OutputStrategy::Summary));
        // With no ranges, all should match source
        assert_eq!(strategies[&OutputStrategy::Full].content, source);
        assert_eq!(strategies[&OutputStrategy::NoTests].content, source);
        assert_eq!(strategies[&OutputStrategy::Summary].content, source);
    }

    #[test]
    fn test_full_mode_is_verbatim() {
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let strategies = build_strategies(source, &[], &[]);
        assert_eq!(strategies[&OutputStrategy::Full].content, source);
        assert_eq!(strategies[&OutputStrategy::Full].token_count, compute_token_count(source));
    }

    #[test]
    fn test_no_tests_less_than_full_tokens() {
        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn test_add() {\n        assert_eq!(add(1, 2), 3);\n    }\n}\n";
        let test_range_start = source.find("#[cfg(test)]").unwrap();
        let test_ranges = vec![RemovalRange {
            start: test_range_start,
            end: source.len(),
            reason: RemovalReason::TestModule,
        }];
        let strategies = build_strategies(source, &test_ranges, &[]);
        assert!(
            strategies[&OutputStrategy::NoTests].token_count <
                strategies[&OutputStrategy::Full].token_count,
            "NoTests should have fewer tokens than Full"
        );
    }
}
