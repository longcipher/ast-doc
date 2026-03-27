//! Configuration types for the ast-doc pipeline.

use std::path::PathBuf;

/// Output strategy for code extraction.
///
/// Ordering: `Full < NoTests < Summary` (increasing degradation level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum OutputStrategy {
    /// Include all source code verbatim.
    #[default]
    Full,
    /// Strip test modules and test functions.
    NoTests,
    /// Extract signatures only, omit implementations.
    Summary,
}

impl OutputStrategy {
    /// Return the next more-degraded strategy, or `None` if already at `Summary`.
    #[must_use]
    pub const fn degrade(self) -> Option<Self> {
        match self {
            Self::Full => Some(Self::NoTests),
            Self::NoTests => Some(Self::Summary),
            Self::Summary => None,
        }
    }
}

impl std::fmt::Display for OutputStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "Full"),
            Self::NoTests => write!(f, "NoTests"),
            Self::Summary => write!(f, "Summary"),
        }
    }
}

/// Top-level configuration for the ast-doc pipeline.
#[derive(Debug, Clone)]
pub struct AstDocConfig {
    /// Path to the project root directory.
    pub path: PathBuf,
    /// Output file path (None = stdout).
    pub output: Option<PathBuf>,
    /// Maximum token budget for the output.
    pub max_tokens: usize,
    /// Glob patterns for core files that should never be degraded.
    pub core_patterns: Vec<String>,
    /// Default output strategy for non-core files.
    pub default_strategy: OutputStrategy,
    /// Glob patterns to include (e.g., "*.rs").
    pub include_patterns: Vec<String>,
    /// Glob patterns to exclude (e.g., "*.txt").
    pub exclude_patterns: Vec<String>,
    /// Skip git context collection.
    pub no_git: bool,
    /// Skip directory tree generation.
    pub no_tree: bool,
    /// Copy output to clipboard.
    pub copy: bool,
    /// Enable verbose logging.
    pub verbose: bool,
}
