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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    /// Any other language supported by tree-sitter-language-pack.
    /// Contains the language name (e.g., "java", "kotlin", "ruby").
    Generic(String),
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Python => write!(f, "Python"),
            Self::TypeScript => write!(f, "TypeScript"),
            Self::Go => write!(f, "Go"),
            Self::C => write!(f, "C"),
            Self::Generic(name) => write!(f, "{name}"),
        }
    }
}

impl Language {
    /// Return the tree-sitter language name used by `tree-sitter-language-pack`.
    #[must_use]
    #[expect(clippy::missing_const_for_fn)]
    pub fn ts_pack_name(&self) -> &str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::C => "c",
            Self::Generic(name) => name.as_str(),
        }
    }

    /// Return `true` if this is one of the core 5 languages with deep analysis.
    #[must_use]
    pub const fn is_core(&self) -> bool {
        matches!(self, Self::Rust | Self::Python | Self::TypeScript | Self::Go | Self::C)
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
///
/// When `lang-pack` feature is enabled, falls back to
/// `tree-sitter-language-pack` for extension resolution.
#[must_use]
pub fn detect_language(path: &Path) -> Option<Language> {
    // Core languages (always available when their feature is enabled)
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => return Some(Language::Rust),
        Some("py") => return Some(Language::Python),
        Some("ts" | "tsx" | "js" | "jsx") => return Some(Language::TypeScript),
        Some("go") => return Some(Language::Go),
        Some("c" | "h") => return Some(Language::C),
        _ => {}
    }

    // Fall back to tree-sitter-language-pack for other languages
    #[cfg(feature = "lang-pack")]
    {
        detect_language_via_pack(path)
    }

    #[cfg(not(feature = "lang-pack"))]
    {
        None
    }
}

/// Detect language using `tree-sitter-language-pack`'s extension mapping.
#[cfg(feature = "lang-pack")]
fn detect_language_via_pack(path: &Path) -> Option<Language> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    let name = tree_sitter_language_pack::detect_language_from_extension(ext)?;
    // Skip if the pack returns a core language name (already handled above)
    #[expect(clippy::useless_asref)]
    match name.as_ref() {
        "rust" | "python" | "typescript" | "tsx" | "javascript" | "go" | "c" => None,
        other => tree_sitter_language_pack::has_language(other)
            .then(|| Language::Generic(other.to_string())),
    }
}

/// Parse a discovered file into a `ParsedFile`.
///
/// Dispatches to the appropriate language parser based on the detected language.
///
/// # Errors
///
/// Returns an error if the language feature is not enabled or parsing fails.
pub fn parse_file(file: &DiscoveredFile, lang: &Language) -> Result<ParsedFile, AstDocError> {
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
        #[cfg(feature = "lang-pack")]
        Language::Generic(name) => {
            lang::generic_parser::GenericParser::new(name).parse(&file.content, &file.path)
        }
        #[allow(unreachable_patterns)]
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

    #[test]
    fn test_language_display_generic() {
        assert_eq!(Language::Generic("java".to_string()).to_string(), "java");
    }

    #[test]
    fn test_language_is_core() {
        assert!(Language::Rust.is_core());
        assert!(Language::Python.is_core());
        assert!(!Language::Generic("java".to_string()).is_core());
    }

    #[test]
    fn test_language_ts_pack_name() {
        assert_eq!(Language::Rust.ts_pack_name(), "rust");
        assert_eq!(Language::Python.ts_pack_name(), "python");
        assert_eq!(Language::Generic("java".to_string()).ts_pack_name(), "java");
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
        let result = parse_file(&file, &Language::Rust).unwrap();
        assert_eq!(result.language, Language::Rust);
        assert!(result.strategies_data.contains_key(&OutputStrategy::Full));
        assert!(result.strategies_data.contains_key(&OutputStrategy::NoTests));
        assert!(result.strategies_data.contains_key(&OutputStrategy::Summary));
    }
}
