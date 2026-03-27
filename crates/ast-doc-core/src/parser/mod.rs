//! Phase 2: AST parsing and strategy extraction.
//!
//! Uses tree-sitter to parse source files and pre-compute
//! Full/NoTests/Summary strategy variants with token counts.

pub mod lang;
pub mod strategy;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{config::OutputStrategy, error::AstDocError, ingestion::DiscoveredFile};

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Rust source files (.rs).
    Rust,
    /// Python source files (.py).
    Python,
    /// TypeScript/JavaScript source files (.ts, .tsx, .js, .jsx).
    TypeScript,
    /// Go source files (.go).
    Go,
    /// C source files (.c, .h).
    C,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Python => write!(f, "Python"),
            Self::TypeScript => write!(f, "TypeScript"),
            Self::Go => write!(f, "Go"),
            Self::C => write!(f, "C"),
        }
    }
}

/// Pre-computed content and token count for a single strategy.
#[derive(Debug, Clone)]
pub struct StrategyData {
    /// The rendered source text for this strategy.
    pub content: String,
    /// Token count of content (computed once via tiktoken-rs during parsing).
    pub token_count: usize,
}

/// A parsed file with pre-computed strategy data for all output modes.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    /// Relative path from the project root.
    pub path: PathBuf,
    /// Detected language.
    pub language: Language,
    /// Original source content.
    pub source: String,
    /// Pre-computed strategy data for each output mode.
    pub strategies_data: HashMap<OutputStrategy, StrategyData>,
}

/// Trait for language-specific parsers.
pub trait LanguageParser {
    /// Parse the source code and produce a `ParsedFile`.
    ///
    /// # Errors
    ///
    /// Returns an error if tree-sitter parsing fails.
    fn parse(&self, source: &str, path: &Path) -> Result<ParsedFile, AstDocError>;
}

/// Detect the language from a file extension.
#[must_use]
pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Some(Language::Rust),
        Some("py") => Some(Language::Python),
        Some("ts" | "tsx" | "js" | "jsx") => Some(Language::TypeScript),
        Some("go") => Some(Language::Go),
        Some("c" | "h") => Some(Language::C),
        _ => None,
    }
}

/// Parse a discovered file into a `ParsedFile`.
///
/// Dispatches to the appropriate language parser based on the detected language.
///
/// # Errors
///
/// Returns an error if the language feature is not enabled or parsing fails.
pub fn parse_file(file: &DiscoveredFile, lang: Language) -> Result<ParsedFile, AstDocError> {
    match lang {
        #[cfg(feature = "lang-rust")]
        Language::Rust => lang::rust_parser::RustParser::new().parse(&file.content, &file.path),
        #[cfg(feature = "lang-python")]
        Language::Python => {
            lang::python_parser::PythonParser::new().parse(&file.content, &file.path)
        }
        #[cfg(feature = "lang-typescript")]
        Language::TypeScript => {
            lang::typescript_parser::TypeScriptParser::new().parse(&file.content, &file.path)
        }
        #[cfg(feature = "lang-go")]
        Language::Go => lang::go_parser::GoParser::new().parse(&file.content, &file.path),
        #[cfg(feature = "lang-c")]
        Language::C => lang::c_parser::CParser::new().parse(&file.content, &file.path),
        #[cfg(not(all(
            feature = "lang-rust",
            feature = "lang-python",
            feature = "lang-typescript",
            feature = "lang-go",
            feature = "lang-c"
        )))]
        _ => Err(AstDocError::UnsupportedLanguage { language: lang.to_string() }),
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(detect_language(Path::new("main.rs")), Some(Language::Rust));
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(detect_language(Path::new("app.py")), Some(Language::Python));
    }

    #[test]
    fn test_detect_language_typescript() {
        assert_eq!(detect_language(Path::new("index.ts")), Some(Language::TypeScript));
        assert_eq!(detect_language(Path::new("app.tsx")), Some(Language::TypeScript));
        assert_eq!(detect_language(Path::new("script.js")), Some(Language::TypeScript));
    }

    #[test]
    fn test_detect_language_go() {
        assert_eq!(detect_language(Path::new("main.go")), Some(Language::Go));
    }

    #[test]
    fn test_detect_language_c() {
        assert_eq!(detect_language(Path::new("main.c")), Some(Language::C));
        assert_eq!(detect_language(Path::new("header.h")), Some(Language::C));
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language(Path::new("readme.md")), None);
        assert_eq!(detect_language(Path::new("data.json")), None);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_parse_file_rust() {
        let file = DiscoveredFile {
            path: PathBuf::from("src/main.rs"),
            content: "fn main() {\n    println!(\"hello\");\n}\n".to_string(),
            language: Some(Language::Rust),
            raw_token_count: 10,
        };
        let result = parse_file(&file, Language::Rust).unwrap();
        assert_eq!(result.language, Language::Rust);
        assert!(result.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(result.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(result.strategies_data.contains_key(&OutputStrategy::Summary));
    }
}
