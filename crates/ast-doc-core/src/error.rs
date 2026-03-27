//! Error types for the ast-doc pipeline.

/// Errors that can occur during the ast-doc pipeline.
#[derive(Debug, thiserror::Error)]
pub enum AstDocError {
    /// The token budget is exceeded even with minimum strategies.
    #[error("Budget exceeded: {message}")]
    BudgetExceeded {
        /// Human-readable explanation.
        message: String,
    },

    /// An unsupported language was requested.
    #[error("Unsupported language: {language}")]
    UnsupportedLanguage {
        /// The language identifier that was not recognized.
        language: String,
    },

    /// A file could not be read.
    #[error("Failed to read file {path}: {source}")]
    FileRead {
        /// Path to the file.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// A tree-sitter parse error.
    #[error("Parse error in {path}: {message}")]
    Parse {
        /// Path to the file.
        path: std::path::PathBuf,
        /// Error description.
        message: String,
    },

    /// Git operation failed.
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// Glob pattern compilation failed.
    #[error("Invalid glob pattern: {0}")]
    InvalidGlob(#[from] globset::Error),

    /// Generic I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON serialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
