//! CLI binary for generating optimized `llms.txt` documentation from codebases.
//!
//! This crate wraps the [ast-doc-core](https://crates.io/crates/ast-doc-core) library
//! with a command-line interface built on `clap`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod report;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use eyre::{Result, WrapErr as _};

/// Output strategy for code extraction.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum CliOutputStrategy {
    /// Include all source code verbatim.
    #[default]
    Full,
    /// Strip test modules and test functions.
    NoTests,
    /// Extract signatures only, omit implementations.
    Summary,
}

impl From<CliOutputStrategy> for ast_doc_core::OutputStrategy {
    fn from(value: CliOutputStrategy) -> Self {
        match value {
            CliOutputStrategy::Full => Self::Full,
            CliOutputStrategy::NoTests => Self::NoTests,
            CliOutputStrategy::Summary => Self::Summary,
        }
    }
}

/// CLI arguments for ast-doc.
#[derive(Debug, Parser)]
#[command(
    name = "ast-doc",
    version,
    about = "Generate optimized llms.txt documentation from codebases"
)]
pub struct Args {
    /// Path to the project root directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output file path (default: stdout).
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Maximum token budget for the output.
    #[arg(short = 'm', long, default_value_t = 128_000)]
    pub max_tokens: usize,

    /// Glob patterns for core files that should never be degraded.
    #[arg(short, long)]
    pub core: Vec<String>,

    /// Default output strategy for non-core files.
    #[arg(short, long, value_enum, default_value_t = CliOutputStrategy::Full)]
    pub strategy: CliOutputStrategy,

    /// Glob patterns to include (e.g., "*.rs").
    #[arg(long)]
    pub include: Vec<String>,

    /// Glob patterns to exclude (e.g., "*.txt").
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Skip git context collection.
    #[arg(long)]
    pub no_git: bool,

    /// Skip directory tree generation.
    #[arg(long)]
    pub no_tree: bool,

    /// Copy output to clipboard.
    #[arg(long)]
    pub copy: bool,

    /// Enable verbose logging.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Build an [`ast_doc_core::AstDocConfig`] from parsed CLI arguments.
#[must_use]
pub fn build_config(args: &Args) -> ast_doc_core::AstDocConfig {
    ast_doc_core::AstDocConfig {
        path: args.path.clone(),
        output: args.output.clone(),
        max_tokens: args.max_tokens,
        core_patterns: args.core.clone(),
        default_strategy: args.strategy.into(),
        include_patterns: args.include.clone(),
        exclude_patterns: args.exclude.clone(),
        no_git: args.no_git,
        no_tree: args.no_tree,
        copy: args.copy,
        verbose: args.verbose,
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(if args.verbose { tracing::Level::DEBUG } else { tracing::Level::WARN })
        .init();

    tracing::debug!(?args, "ast-doc CLI started");

    let config = build_config(&args);

    if config.copy {
        tracing::warn!(
            "--copy flag is set but clipboard support is not yet implemented. Output will not be copied to clipboard."
        );
    }

    let result = ast_doc_core::run_pipeline(&config).wrap_err("pipeline execution failed")?;

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &result.output)
            .wrap_err_with(|| format!("failed to write output to {}", output_path.display()))?;
        tracing::info!("Output written to {}", output_path.display());
    } else {
        println!("{}", result.output);
    }

    report::print_report(&result.schedule, args.max_tokens);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_args() {
        let args = Args::try_parse_from(["ast-doc"]).expect("default args should parse");
        assert_eq!(args.path, PathBuf::from("."));
        assert!(args.output.is_none());
        assert_eq!(args.max_tokens, 128_000);
        assert!(args.core.is_empty());
        assert!(args.include.is_empty());
        assert!(args.exclude.is_empty());
        assert!(!args.no_git);
        assert!(!args.no_tree);
        assert!(!args.copy);
        assert!(!args.verbose);
    }

    #[test]
    fn parse_explicit_path() {
        let args =
            Args::try_parse_from(["ast-doc", "/tmp/project"]).expect("path arg should parse");
        assert_eq!(args.path, PathBuf::from("/tmp/project"));
    }

    #[test]
    fn parse_all_flags() {
        let args = Args::try_parse_from([
            "ast-doc",
            ".",
            "--output",
            "out.txt",
            "--max-tokens",
            "64000",
            "--core",
            "src/main.rs",
            "--core",
            "src/lib.rs",
            "--strategy",
            "summary",
            "--include",
            "*.rs",
            "--exclude",
            "*.txt",
            "--no-git",
            "--no-tree",
            "--copy",
            "--verbose",
        ])
        .expect("full args should parse");

        assert_eq!(args.output, Some(PathBuf::from("out.txt")));
        assert_eq!(args.max_tokens, 64_000);
        assert_eq!(args.core, vec!["src/main.rs", "src/lib.rs"]);
        assert_eq!(args.include, vec!["*.rs"]);
        assert_eq!(args.exclude, vec!["*.txt"]);
        assert!(args.no_git);
        assert!(args.no_tree);
        assert!(args.copy);
        assert!(args.verbose);
    }

    #[test]
    fn build_config_preserves_args() {
        let args = Args::try_parse_from([
            "ast-doc",
            "src",
            "--max-tokens",
            "32000",
            "--strategy",
            "no-tests",
            "--no-git",
            "--verbose",
        ])
        .expect("args should parse");

        let config = build_config(&args);
        assert_eq!(config.path, PathBuf::from("src"));
        assert_eq!(config.max_tokens, 32_000);
        assert_eq!(config.default_strategy, ast_doc_core::OutputStrategy::NoTests);
        assert!(config.no_git);
        assert!(config.verbose);
    }

    #[test]
    fn strategy_conversion() {
        assert_eq!(
            ast_doc_core::OutputStrategy::from(CliOutputStrategy::Full),
            ast_doc_core::OutputStrategy::Full
        );
        assert_eq!(
            ast_doc_core::OutputStrategy::from(CliOutputStrategy::NoTests),
            ast_doc_core::OutputStrategy::NoTests
        );
        assert_eq!(
            ast_doc_core::OutputStrategy::from(CliOutputStrategy::Summary),
            ast_doc_core::OutputStrategy::Summary
        );
    }
}
