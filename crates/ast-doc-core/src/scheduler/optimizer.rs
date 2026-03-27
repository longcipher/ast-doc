//! Degradation optimizer for token budget enforcement.
//!
//! Pure mathematical optimizer: selects from pre-computed `strategies_data`
//! entries per `ParsedFile`. No string manipulation, just token arithmetic.

use std::{collections::HashMap, path::Path};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{
    config::{AstDocConfig, OutputStrategy},
    error::AstDocError,
    parser::ParsedFile,
    scheduler::{ScheduleResult, ScheduledFile},
};

/// Run the degradation optimizer.
///
/// # Errors
///
/// Returns `AstDocError::BudgetExceeded` if even minimum strategies
/// exceed the token budget, or if glob compilation fails.
pub fn optimize(
    parsed: &[ParsedFile],
    config: &AstDocConfig,
    base_overhead_tokens: usize,
) -> Result<ScheduleResult, AstDocError> {
    let core_set = build_core_globset(&config.core_patterns)?;

    // 1. Check base overhead
    if base_overhead_tokens >= config.max_tokens {
        return Err(AstDocError::BudgetExceeded {
            message: format!(
                "Base overhead ({base_overhead_tokens} tokens) exceeds or equals budget ({})",
                config.max_tokens
            ),
        });
    }
    let remaining_budget = config.max_tokens - base_overhead_tokens;

    // 2. Assign initial strategies
    let mut assignments: Vec<(usize, OutputStrategy)> = parsed
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if is_core(&core_set, &f.path) {
                (i, OutputStrategy::Full)
            } else {
                (i, config.default_strategy)
            }
        })
        .collect();

    // 3. Compute initial total
    let raw_tokens: usize = parsed
        .iter()
        .filter_map(|f| f.strategies_data.get(&OutputStrategy::Full))
        .map(|sd| sd.token_count)
        .sum();

    let mut total_tokens = compute_total(parsed, &assignments);

    // 4. Degradation loop
    while total_tokens > remaining_budget {
        // Collect degradable files
        let mut degradable: Vec<(usize, OutputStrategy, usize)> = assignments
            .iter()
            .filter(|(_, strategy)| strategy.degrade().is_some())
            .filter(|(i, _)| !is_core(&core_set, &parsed[*i].path))
            .map(|(i, strategy)| {
                let tc = parsed[*i].strategies_data.get(strategy).map_or(0, |sd| sd.token_count);
                (*i, *strategy, tc)
            })
            .collect();

        if degradable.is_empty() {
            return Err(AstDocError::BudgetExceeded {
                message: format!(
                    "All files at minimum strategy but still over budget: {total_tokens} > {remaining_budget}"
                ),
            });
        }

        // Sort: files with tests first (NoTests saves more), then by token count desc
        degradable.sort_by(|a, b| {
            let a_has_tests = has_test_content(&parsed[a.0]);
            let b_has_tests = has_test_content(&parsed[b.0]);
            // Test-heavy files first (descending by has_tests)
            b_has_tests.cmp(&a_has_tests).then_with(|| {
                // Then descending by current token count
                b.2.cmp(&a.2)
            })
        });

        // Degrade the first candidate
        let (idx, current_strategy, _) = degradable[0];
        if let Some(next) = current_strategy.degrade() {
            assignments[idx] = (idx, next);
        }

        let new_total = compute_total(parsed, &assignments);

        // Safety valve: if no reduction, force-skip (set to Summary if not already)
        if new_total == total_tokens {
            // Force the file to its absolute minimum
            if let Some(min_strategy) = assignments[idx].1.degrade() {
                assignments[idx] = (idx, min_strategy);
            } else {
                // Already at minimum and no reduction — this shouldn't happen
                // but guard against infinite loops
                return Err(AstDocError::BudgetExceeded {
                    message: format!(
                        "No token reduction possible for file at index {idx}, stuck at {total_tokens} tokens"
                    ),
                });
            }
        }

        total_tokens = compute_total(parsed, &assignments);
    }

    // 5. Build result
    build_result(parsed, &assignments, raw_tokens)
}

/// Build a `GlobSet` from core patterns.
fn build_core_globset(patterns: &[String]) -> Result<GlobSet, AstDocError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)?;
        builder.add(glob);
    }
    Ok(builder.build()?)
}

/// Check if a file path matches any core pattern.
fn is_core(core_set: &GlobSet, path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    core_set.is_match(&*path_str)
}

/// Compute total tokens for current assignments.
fn compute_total(parsed: &[ParsedFile], assignments: &[(usize, OutputStrategy)]) -> usize {
    assignments
        .iter()
        .filter_map(|(i, strategy)| parsed[*i].strategies_data.get(strategy))
        .map(|sd| sd.token_count)
        .sum()
}

/// Check if a file has test-related content (heuristic: strategies_data for NoTests
/// saves tokens compared to Full).
fn has_test_content(parsed: &ParsedFile) -> bool {
    let full_tc = parsed.strategies_data.get(&OutputStrategy::Full).map_or(0, |sd| sd.token_count);
    let notests_tc =
        parsed.strategies_data.get(&OutputStrategy::NoTests).map_or(0, |sd| sd.token_count);
    notests_tc < full_tc
}

/// Build the final `ScheduleResult` from assignments.
fn build_result(
    parsed: &[ParsedFile],
    assignments: &[(usize, OutputStrategy)],
    raw_tokens: usize,
) -> Result<ScheduleResult, AstDocError> {
    let mut files = Vec::with_capacity(assignments.len());
    let mut strategy_counts: HashMap<OutputStrategy, usize> = HashMap::new();

    for (i, strategy) in assignments {
        let rendered_tokens =
            parsed[*i].strategies_data.get(strategy).map_or(0, |sd| sd.token_count);
        let full_tokens =
            parsed[*i].strategies_data.get(&OutputStrategy::Full).map_or(0, |sd| sd.token_count);

        *strategy_counts.entry(*strategy).or_insert(0) += 1;

        files.push(ScheduledFile {
            parsed: parsed[*i].clone(),
            strategy: *strategy,
            rendered_tokens,
            saved_tokens: full_tokens.saturating_sub(rendered_tokens),
        });
    }

    let total_tokens: usize = files.iter().map(|f| f.rendered_tokens).sum();

    Ok(ScheduleResult { files, total_tokens, raw_tokens, strategy_counts })
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use super::*;
    use crate::{
        config::OutputStrategy,
        parser::{Language, ParsedFile, StrategyData},
    };

    /// Helper: build a `ParsedFile` with given token counts for each strategy.
    fn make_parsed(path: &str, full: usize, notests: usize, summary: usize) -> ParsedFile {
        let mut strategies_data = HashMap::new();
        strategies_data.insert(
            OutputStrategy::Full,
            StrategyData { content: String::new(), token_count: full },
        );
        strategies_data.insert(
            OutputStrategy::NoTests,
            StrategyData { content: String::new(), token_count: notests },
        );
        strategies_data.insert(
            OutputStrategy::Summary,
            StrategyData { content: String::new(), token_count: summary },
        );
        ParsedFile {
            path: PathBuf::from(path),
            language: Language::Rust,
            source: String::new(),
            strategies_data,
        }
    }

    fn make_config(max_tokens: usize, core_patterns: Vec<&str>) -> AstDocConfig {
        AstDocConfig {
            path: PathBuf::from("."),
            output: None,
            max_tokens,
            core_patterns: core_patterns.into_iter().map(String::from).collect(),
            default_strategy: OutputStrategy::Full,
            include_patterns: vec![],
            exclude_patterns: vec![],
            no_git: true,
            no_tree: true,
            copy: false,
            verbose: false,
        }
    }

    #[test]
    fn test_budget_larger_than_total_all_full() {
        // All files fit within budget → all get Full strategy
        let files =
            vec![make_parsed("src/a.rs", 100, 80, 50), make_parsed("src/b.rs", 200, 150, 80)];
        let config = make_config(1000, vec![]);
        let result = optimize(&files, &config, 50).unwrap();

        assert_eq!(result.total_tokens, 300);
        assert_eq!(result.strategy_counts.get(&OutputStrategy::Full), Some(&2));
        for f in &result.files {
            assert_eq!(f.strategy, OutputStrategy::Full);
        }
    }

    #[test]
    fn test_budget_forces_some_to_notests() {
        // Budget forces some files to NoTests
        // file_a: Full=500, NoTests=300, Summary=100
        // file_b: Full=500, NoTests=400, Summary=100
        // Total Full = 1000, budget = 800 (base_overhead = 0)
        // After degrading file_a (test-heavy first): 500 + 400 = 900, still over
        // After degrading file_b: 300 + 400 = 700, fits
        let files =
            vec![make_parsed("src/a.rs", 500, 300, 100), make_parsed("src/b.rs", 500, 400, 100)];
        let config = make_config(800, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert!(result.total_tokens <= 800);
        // At least one file should be degraded
        assert!(result.strategy_counts.get(&OutputStrategy::Full).copied().unwrap_or(0) < 2);
    }

    #[test]
    fn test_budget_forces_some_to_summary() {
        // Budget forces some files all the way to Summary
        let files = vec![
            make_parsed("src/a.rs", 400, 300, 50),
            make_parsed("src/b.rs", 400, 300, 50),
            make_parsed("src/c.rs", 400, 300, 50),
        ];
        // Total Full = 1200, budget = 500
        let config = make_config(500, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert!(result.total_tokens <= 500);
        // Some files should be at Summary
        assert!(result.strategy_counts.get(&OutputStrategy::Summary).copied().unwrap_or(0) > 0);
    }

    #[test]
    fn test_core_files_never_degrade() {
        // Core files should always get Full strategy
        let files = vec![
            make_parsed("src/lib.rs", 500, 300, 100),
            make_parsed("src/utils.rs", 500, 300, 100),
        ];
        let config = make_config(600, vec!["src/lib.rs"]);
        let result = optimize(&files, &config, 0).unwrap();

        let lib_file =
            result.files.iter().find(|f| f.parsed.path == PathBuf::from("src/lib.rs")).unwrap();
        assert_eq!(lib_file.strategy, OutputStrategy::Full);

        // utils.rs should be degraded since budget is tight
        let utils_file =
            result.files.iter().find(|f| f.parsed.path == PathBuf::from("src/utils.rs")).unwrap();
        assert_ne!(utils_file.strategy, OutputStrategy::Full);
    }

    #[test]
    fn test_base_overhead_exceeds_budget() {
        let files = vec![make_parsed("src/a.rs", 100, 80, 50)];
        let config = make_config(100, vec![]);
        let result = optimize(&files, &config, 100);
        assert!(matches!(result, Err(AstDocError::BudgetExceeded { .. })));
    }

    #[test]
    fn test_all_summary_still_over_budget() {
        // Even at Summary, still over budget → BudgetExceeded
        let files =
            vec![make_parsed("src/a.rs", 400, 300, 200), make_parsed("src/b.rs", 400, 300, 200)];
        // Summary total = 400, budget = 300
        let config = make_config(300, vec![]);
        let result = optimize(&files, &config, 0);
        assert!(matches!(result, Err(AstDocError::BudgetExceeded { .. })));
    }

    #[test]
    fn test_single_file() {
        let files = vec![make_parsed("src/main.rs", 500, 300, 100)];
        let config = make_config(400, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.total_tokens <= 400);
        // Should be degraded to NoTests (300 <= 400)
        assert_eq!(result.files[0].strategy, OutputStrategy::NoTests);
    }

    #[test]
    fn test_all_core_nothing_to_degrade() {
        // All files are core, but they fit in budget
        let files =
            vec![make_parsed("src/lib.rs", 200, 150, 50), make_parsed("src/core.rs", 200, 150, 50)];
        let config = make_config(1000, vec!["**/*.rs"]);
        let result = optimize(&files, &config, 0).unwrap();

        for f in &result.files {
            assert_eq!(f.strategy, OutputStrategy::Full);
        }
    }

    #[test]
    fn test_all_core_over_budget_errors() {
        // All files are core, over budget → error
        let files = vec![
            make_parsed("src/lib.rs", 500, 400, 300),
            make_parsed("src/core.rs", 500, 400, 300),
        ];
        let config = make_config(500, vec!["**/*.rs"]);
        let result = optimize(&files, &config, 0);
        assert!(matches!(result, Err(AstDocError::BudgetExceeded { .. })));
    }

    #[test]
    fn test_empty_files() {
        let files: Vec<ParsedFile> = vec![];
        let config = make_config(1000, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.raw_tokens, 0);
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_base_overhead_deducted_from_budget() {
        let files = vec![make_parsed("src/a.rs", 300, 200, 100)];
        // max_tokens = 400, base_overhead = 200, remaining = 200
        // Full = 300 > 200, so must degrade to NoTests (200)
        let config = make_config(400, vec![]);
        let result = optimize(&files, &config, 200).unwrap();

        assert_eq!(result.files[0].strategy, OutputStrategy::NoTests);
        assert_eq!(result.total_tokens, 200);
    }

    #[test]
    fn test_saved_tokens_computed_correctly() {
        let files = vec![make_parsed("src/a.rs", 500, 300, 100)];
        let config = make_config(400, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert_eq!(result.files[0].saved_tokens, 200); // 500 - 300
    }

    #[test]
    fn test_raw_tokens_is_sum_of_full() {
        let files =
            vec![make_parsed("src/a.rs", 100, 80, 50), make_parsed("src/b.rs", 200, 150, 80)];
        let config = make_config(1000, vec![]);
        let result = optimize(&files, &config, 0).unwrap();

        assert_eq!(result.raw_tokens, 300);
    }

    use proptest::prelude::*;

    fn file_strategy() -> impl Strategy<Value = ParsedFile> {
        (proptest::string::string_regex("[a-z]{1,8}").unwrap(), 1usize..10_000)
            .prop_flat_map(|(name, full)| (Just(name), Just(full), 0..full, 0..full))
            .prop_map(|(name, full, notests_delta, summary_delta)| {
                let notests = full.saturating_sub(notests_delta);
                let summary = notests.saturating_sub(summary_delta);
                make_parsed(&format!("src/{name}.rs"), full, notests, summary)
            })
    }

    proptest! {
        #[test]
        fn budget_invariant(
            files in proptest::collection::vec(file_strategy(), 1..10),
            max_tokens in 50_usize..20_000,
        ) {
            let config = make_config(max_tokens, vec![]);
            match optimize(&files, &config, 0) {
                Ok(result) => {
                    prop_assert!(
                        result.total_tokens <= max_tokens,
                        "total_tokens ({}) > max_tokens ({})",
                        result.total_tokens,
                        max_tokens,
                    );
                }
                Err(AstDocError::BudgetExceeded { .. }) => {
                    // Acceptable: even minimum strategies exceed budget
                }
                Err(e) => {
                    panic!("unexpected error: {e:?}");
                }
            }
        }

        #[test]
        fn core_files_always_full(
            files in proptest::collection::vec(file_strategy(), 1..10),
            max_tokens in 50_usize..20_000,
        ) {
            let config = make_config(max_tokens, vec!["src/core*.rs"]);
            if let Ok(result) = optimize(&files, &config, 0) {
                for f in &result.files {
                    if f.parsed.path.to_string_lossy().starts_with("src/core") {
                        prop_assert_eq!(
                            f.strategy,
                            OutputStrategy::Full,
                            "core file {:?} degraded to {}",
                            f.parsed.path,
                            f.strategy,
                        );
                    }
                }
            }
        }

        #[test]
        fn bpe_tokenizer_invariants(
            s in proptest::string::string_regex("[a-zA-Z0-9_ \t\n]{0,200}").unwrap(),
            t in proptest::string::string_regex("[a-zA-Z0-9_ \t\n]{0,200}").unwrap(),
        ) {
            let bpe = tiktoken_rs::cl100k_base().unwrap();
            let count_s = bpe.encode_with_special_tokens(&s).len();
            // Property 1: empty string encodes to 0 tokens
            if s.is_empty() {
                prop_assert_eq!(count_s, 0);
            }
            // Property 2: non-empty string encodes to at least 1 token
            if !s.is_empty() {
                prop_assert!(count_s >= 1);
            }
            // Property 3: prefix monotonicity — if prefix, token count does not decrease
            let extended = format!("{s}{t}");
            let count_extended = bpe.encode_with_special_tokens(&extended).len();
            prop_assert!(
                count_extended >= count_s,
                "token count decreased for extension: {count_extended} < {count_s}",
            );
        }
    }
}
