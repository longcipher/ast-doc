//! Core library for [ast-doc](https://crates.io/crates/ast-doc): a four-stage pipeline
//! for generating optimized `llms.txt` documentation from codebases.
//!
//! # Pipeline
//!
//! 1. **Ingestion** — File discovery, git metadata capture, directory tree generation.
//! 2. **Parser** — tree-sitter AST extraction with pre-computed strategy variants.
//! 3. **Scheduler** — Token budget optimization with intelligent degradation.
//! 4. **Renderer** — Markdown assembly with anti-bloat rules.
//!
//! # Quick Start
//!
//! ```no_run
//! use std::path::PathBuf;
//!
//! use ast_doc_core::{AstDocConfig, OutputStrategy};
//!
//! let config = AstDocConfig {
//!     path: PathBuf::from("."),
//!     output: None,
//!     max_tokens: 128_000,
//!     core_patterns: vec![],
//!     default_strategy: OutputStrategy::Full,
//!     include_patterns: vec![],
//!     exclude_patterns: vec![],
//!     no_git: false,
//!     no_tree: false,
//!     copy: false,
//!     verbose: false,
//! };
//!
//! let result = ast_doc_core::run_pipeline(&config).expect("pipeline failed");
//! println!("{}", result.output);
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

pub mod config;
pub mod error;
pub mod ingestion;
pub mod parser;
pub mod renderer;
pub mod scheduler;

pub use config::{AstDocConfig, OutputStrategy};
pub use error::AstDocError;
pub use ingestion::{DiscoveredFile, GitContext, IngestionResult};
pub use parser::{Language, ParsedFile, StrategyData};
use rayon::prelude::*;
pub use scheduler::{ScheduleResult, ScheduledFile};

/// Maximum tokens allowed for a git diff before truncation.
const MAX_DIFF_TOKENS: usize = 1000;

/// Result of running the full pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    /// The rendered `llms.txt` output.
    pub output: String,
    /// The scheduling result with token breakdowns.
    pub schedule: ScheduleResult,
}

/// Run the full ast-doc pipeline and return the rendered output plus scheduling metadata.
///
/// # Errors
///
/// Returns an error if any pipeline stage fails.
pub fn run_pipeline(config: &AstDocConfig) -> eyre::Result<PipelineResult> {
    // Phase 1: Ingestion — file discovery, git metadata, directory tree
    let ingestion = ingestion::run_ingestion(config)?;

    // Phase 2: Parser — tree-sitter extraction + pre-compute all strategy variants
    let parsed: Vec<ParsedFile> = ingestion
        .files
        .par_iter()
        .filter_map(|f| f.language.map(|lang| (f, lang)))
        .map(|(f, lang)| parser::parse_file(f, lang).map_err(eyre::Report::from))
        .collect::<eyre::Result<Vec<_>>>()?;

    // Compute base overhead from ingestion non-file content
    let base_overhead_tokens = compute_base_overhead(&ingestion);

    // Phase 3: Scheduler — pure optimization using pre-computed token counts
    let scheduled = scheduler::run_scheduler(&parsed, config, base_overhead_tokens)?;

    // Phase 4: Renderer — assemble final markdown
    let output = renderer::render_llms_txt(&scheduled, &ingestion, config)?;

    Ok(PipelineResult { output, schedule: scheduled })
}

/// Compute token overhead from directory tree and git context.
///
/// If the git diff exceeds `MAX_DIFF_TOKENS`, it is truncated
/// with a `"... (diff truncated)"` suffix.
fn compute_base_overhead(ingestion: &IngestionResult) -> usize {
    let mut overhead = count_tokens(&ingestion.directory_tree);

    if let Some(ref git) = ingestion.git_context {
        overhead += count_tokens(&git.branch);
        overhead += count_tokens(&git.latest_commit);
        if let Some(ref diff) = git.diff {
            let diff_tokens = count_tokens(diff);
            if diff_tokens > MAX_DIFF_TOKENS {
                let suffix = "... (diff truncated)";
                // Approximate: use MAX_DIFF_TOKENS + suffix token count
                overhead += MAX_DIFF_TOKENS + count_tokens(suffix);
            } else {
                overhead += diff_tokens;
            }
        }
    }

    overhead
}

/// Count tokens in a string using `tiktoken-rs`.
fn count_tokens(text: &str) -> usize {
    tiktoken_rs::cl100k_base().map_or(0, |bpe| bpe.encode_with_special_tokens(text).len())
}
