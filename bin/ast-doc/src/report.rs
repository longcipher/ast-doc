//! Terminal optimization report formatting.

use std::collections::HashMap;

use ast_doc_core::{OutputStrategy, ScheduleResult};

/// Format and print the optimization report to stderr.
pub(crate) fn print_report(result: &ScheduleResult, max_tokens: usize) {
    eprintln!("{}", format_report(result, max_tokens));
}

/// Format the optimization report as a string.
#[must_use]
pub(crate) fn format_report(result: &ScheduleResult, max_tokens: usize) -> String {
    let savings_pct = if result.raw_tokens > 0 {
        (result.raw_tokens.saturating_sub(result.total_tokens) as f64 / result.raw_tokens as f64) *
            100.0
    } else {
        0.0
    };

    let budget_ok = result.total_tokens <= max_tokens;
    let budget_icon = if budget_ok { "\u{1f7e2}" } else { "\u{1f534}" };
    let budget_status = if budget_ok { "within budget" } else { "exceeded" };

    let mut lines = Vec::new();
    lines.push("\u{1f4ca} Optimization Report".to_string());
    lines.push("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());
    lines.push(format!("  Total files: {}", result.files.len()));
    lines.push(format!(
        "  Raw tokens: {}  \u{2192}  Final tokens: {}",
        result.raw_tokens, result.total_tokens
    ));
    lines.push(format!("  Savings: {savings_pct:.1}%"));
    lines.push(String::new());
    lines.push("  Strategy breakdown:".to_string());

    let mut tokens_by_strategy: HashMap<OutputStrategy, usize> = HashMap::new();
    for file in &result.files {
        *tokens_by_strategy.entry(file.strategy).or_insert(0) += file.rendered_tokens;
    }

    for strategy in [OutputStrategy::Full, OutputStrategy::NoTests, OutputStrategy::Summary] {
        let count = result.strategy_counts.get(&strategy).copied().unwrap_or(0);
        let tokens = tokens_by_strategy.get(&strategy).copied().unwrap_or(0);
        lines.push(format!("    {strategy}:     {count} files ({tokens} tokens)"));
    }

    lines.push(String::new());
    lines.push(format!(
        "  Budget: {budget_icon} {budget_status} ({}/{max_tokens} tokens)",
        result.total_tokens
    ));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use ast_doc_core::{ScheduledFile, parser::ParsedFile};

    use super::*;

    fn make_schedule_result(
        files: Vec<ScheduledFile>,
        raw_tokens: usize,
        total_tokens: usize,
        strategy_counts: HashMap<OutputStrategy, usize>,
    ) -> ScheduleResult {
        ScheduleResult { files, total_tokens, raw_tokens, strategy_counts }
    }

    #[test]
    fn test_empty_report() {
        let result = make_schedule_result(vec![], 0, 0, HashMap::new());
        let report = format_report(&result, 1000);
        assert!(report.contains("Total files: 0"));
        assert!(report.contains("Raw tokens: 0"));
        assert!(report.contains("Savings: 0.0%"));
        assert!(report.contains("\u{1f7e2} within budget"));
    }

    #[test]
    fn test_all_full_within_budget() {
        let mut strategy_counts = HashMap::new();
        strategy_counts.insert(OutputStrategy::Full, 2);

        let files = vec![
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/a.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Full,
                rendered_tokens: 100,
                saved_tokens: 0,
            },
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/b.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Full,
                rendered_tokens: 200,
                saved_tokens: 0,
            },
        ];

        let result = make_schedule_result(files, 300, 300, strategy_counts);
        let report = format_report(&result, 1000);

        assert!(report.contains("Total files: 2"));
        assert!(report.contains("Raw tokens: 300"));
        assert!(report.contains("Final tokens: 300"));
        assert!(report.contains("Savings: 0.0%"));
        assert!(report.contains("Full:     2 files (300 tokens)"));
        assert!(report.contains("NoTests:     0 files (0 tokens)"));
        assert!(report.contains("\u{1f7e2} within budget"));
    }

    #[test]
    fn test_mixed_strategies_with_savings() {
        let mut strategy_counts = HashMap::new();
        strategy_counts.insert(OutputStrategy::Full, 1);
        strategy_counts.insert(OutputStrategy::NoTests, 1);
        strategy_counts.insert(OutputStrategy::Summary, 1);

        let files = vec![
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/a.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Full,
                rendered_tokens: 500,
                saved_tokens: 0,
            },
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/b.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::NoTests,
                rendered_tokens: 300,
                saved_tokens: 200,
            },
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/c.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Summary,
                rendered_tokens: 100,
                saved_tokens: 400,
            },
        ];

        // raw_tokens = 500+500+500 = 1500 (all at Full)
        // total_tokens = 500+300+100 = 900
        let result = make_schedule_result(files, 1500, 900, strategy_counts);
        let report = format_report(&result, 1000);

        assert!(report.contains("Total files: 3"));
        assert!(report.contains("Raw tokens: 1500"));
        assert!(report.contains("Final tokens: 900"));
        assert!(report.contains("Savings: 40.0%"));
        assert!(report.contains("Full:     1 files (500 tokens)"));
        assert!(report.contains("NoTests:     1 files (300 tokens)"));
        assert!(report.contains("Summary:     1 files (100 tokens)"));
        assert!(report.contains("\u{1f7e2} within budget"));
    }

    #[test]
    fn test_budget_exceeded() {
        let mut strategy_counts = HashMap::new();
        strategy_counts.insert(OutputStrategy::Summary, 2);

        let files = vec![
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/a.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Summary,
                rendered_tokens: 300,
                saved_tokens: 200,
            },
            ScheduledFile {
                parsed: ParsedFile {
                    path: PathBuf::from("src/b.rs"),
                    language: ast_doc_core::Language::Rust,
                    source: String::new(),
                    strategies_data: HashMap::new(),
                },
                strategy: OutputStrategy::Summary,
                rendered_tokens: 300,
                saved_tokens: 200,
            },
        ];

        let result = make_schedule_result(files, 1000, 600, strategy_counts);
        let report = format_report(&result, 500);

        assert!(report.contains("\u{1f534} exceeded"));
        assert!(report.contains("(600/500 tokens)"));
    }

    #[test]
    fn test_report_header_format() {
        let result = make_schedule_result(vec![], 0, 0, HashMap::new());
        let report = format_report(&result, 1000);

        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines[0], "\u{1f4ca} Optimization Report");
        assert!(lines[1].starts_with('\u{2500}'));
    }
}
