//! Phase 3: Token budget scheduling.
//!
//! Pure mathematical optimizer that selects strategies to fit within
//! the token budget while protecting core files.

pub mod optimizer;

use std::collections::HashMap;

use crate::{
    config::{AstDocConfig, OutputStrategy},
    error::AstDocError,
    parser::ParsedFile,
};

/// A file with its assigned strategy and token counts.
#[derive(Debug, Clone)]
pub struct ScheduledFile {
    /// The parsed file data.
    pub parsed: ParsedFile,
    /// The assigned output strategy.
    pub strategy: OutputStrategy,
    /// Token count for the assigned strategy.
    pub rendered_tokens: usize,
    /// Tokens saved compared to Full strategy.
    pub saved_tokens: usize,
}

/// Result of the scheduling phase.
#[derive(Debug)]
pub struct ScheduleResult {
    /// All scheduled files.
    pub files: Vec<ScheduledFile>,
    /// Total tokens after scheduling.
    pub total_tokens: usize,
    /// Raw tokens before scheduling (all files at Full).
    pub raw_tokens: usize,
    /// Count of files per strategy.
    pub strategy_counts: HashMap<OutputStrategy, usize>,
}

/// Run the token scheduling phase.
///
/// `base_overhead_tokens` is the token cost of non-file content
/// (directory tree + git context) computed during ingestion.
///
/// # Errors
///
/// Returns `AstDocError::BudgetExceeded` if even minimum strategies
/// exceed the token budget.
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn run_scheduler(
    parsed: &[ParsedFile],
    config: &AstDocConfig,
    base_overhead_tokens: usize,
) -> Result<ScheduleResult, AstDocError> {
    optimizer::optimize(parsed, config, base_overhead_tokens)
}
