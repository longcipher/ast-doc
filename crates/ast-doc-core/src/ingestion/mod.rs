//! Phase 1: File discovery and ingestion.
//!
//! Walks the project directory, applies .gitignore/.astdocignore rules,
//! captures git metadata, and produces a directory tree.

pub mod git;
pub mod walker;

use std::path::{Path, PathBuf};

use git::extract_git_context;
use tracing::{debug, info, warn};
use walker::{build_globset, walk_directory};

use crate::{config::AstDocConfig, error::AstDocError, parser::Language};

/// A discovered source file with its content and metadata.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    /// Relative path from the project root.
    pub path: PathBuf,
    /// Full file content.
    pub content: String,
    /// Detected language (None if unsupported).
    pub language: Option<Language>,
    /// Raw token count of the original content.
    pub raw_token_count: usize,
}

/// Git context captured from the repository.
#[derive(Debug, Clone)]
pub struct GitContext {
    /// Current branch name.
    pub branch: String,
    /// Latest commit summary.
    pub latest_commit: String,
    /// Uncommitted changes diff (may be truncated).
    pub diff: Option<String>,
}

/// Result of the ingestion phase.
#[derive(Debug)]
pub struct IngestionResult {
    /// All discovered source files.
    pub files: Vec<DiscoveredFile>,
    /// Directory tree string (with annotations).
    pub directory_tree: String,
    /// Git context (None if `--no-git` or not a git repo).
    pub git_context: Option<GitContext>,
}

/// Run the ingestion phase.
///
/// # Errors
///
/// Returns an error if directory walking or git operations fail.
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn run_ingestion(config: &AstDocConfig) -> Result<IngestionResult, AstDocError> {
    let root = config
        .path
        .canonicalize()
        .map_err(|e| AstDocError::FileRead { path: config.path.clone(), source: e })?;
    info!(path = %root.display(), "starting ingestion");

    // Build glob sets for include/exclude filtering
    let include = build_globset(&config.include_patterns)?;
    let exclude = build_globset(&config.exclude_patterns)?;

    // Walk the directory to discover files
    let file_paths = walk_directory(&root, &include, &exclude, config)?;

    // Read file contents and detect languages
    let mut files = Vec::with_capacity(file_paths.len());
    for rel_path in &file_paths {
        let abs_path = root.join(rel_path);
        match std::fs::read_to_string(&abs_path) {
            Ok(content) => {
                let language = crate::parser::detect_language(rel_path);
                let token_count = count_tokens(&content);
                debug!(
                    path = %rel_path.display(),
                    language = ?language,
                    tokens = token_count,
                    "discovered file"
                );
                files.push(DiscoveredFile {
                    path: rel_path.clone(),
                    content,
                    language,
                    raw_token_count: token_count,
                });
            }
            Err(e) => {
                warn!(
                    path = %rel_path.display(),
                    error = %e,
                    "failed to read file, skipping"
                );
            }
        }
    }

    // Build directory tree
    let directory_tree =
        if config.no_tree { String::new() } else { build_directory_tree(&root, &file_paths) };

    // Capture git context
    let git_context = if config.no_git {
        None
    } else {
        match extract_git_context(&root) {
            Ok(Some(ctx)) => Some(ctx),
            Ok(None) => None,
            Err(e) => {
                warn!(error = %e, "failed to extract git context");
                None
            }
        }
    };

    info!(files = files.len(), git = git_context.is_some(), "ingestion complete");

    Ok(IngestionResult { files, directory_tree, git_context })
}

/// Count tokens in a string using `tiktoken-rs`.
///
/// Uses a cached BPE instance to avoid repeated initialization.
fn count_tokens(text: &str) -> usize {
    use std::sync::LazyLock;
    static BPE: LazyLock<Option<tiktoken_rs::CoreBPE>> =
        LazyLock::new(|| tiktoken_rs::cl100k_base().ok());

    BPE.as_ref().map_or(0, |bpe| bpe.encode_with_special_tokens(text).len())
}

/// Build a directory tree string from discovered file paths.
///
/// Uses `termtree` to render a tree with annotations for detected languages.
fn build_directory_tree(root: &Path, files: &[PathBuf]) -> String {
    use termtree::Tree;

    let parent_name = root.file_name().unwrap_or_default().to_string_lossy().to_string();

    let mut tree = Tree::new(parent_name);

    for file_path in files {
        let mut current = &mut tree;
        let components: Vec<_> =
            file_path.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();

        for (i, component) in components.iter().enumerate() {
            if i == components.len() - 1 {
                // Leaf node - file with language annotation
                let lang = crate::parser::detect_language(file_path)
                    .map(|l| format!(" [{l}]"))
                    .unwrap_or_default();
                current.push(Tree::new(format!("{component}{lang}")));
            } else {
                // Directory node - find or create
                let idx = current.leaves.iter().position(|child| child.root == component.as_str());
                if let Some(pos) = idx {
                    current = &mut current.leaves[pos];
                } else {
                    current.push(Tree::new(component.clone()));
                    let last = current.leaves.len() - 1;
                    current = &mut current.leaves[last];
                }
            }
        }
    }

    tree.to_string()
}

/// Detect the language of a file from its extension.
///
/// Re-exports `parser::detect_language` for convenience.
#[must_use]
pub fn detect_language(path: &Path) -> Option<Language> {
    crate::parser::detect_language(path)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn make_config(root: &Path) -> AstDocConfig {
        AstDocConfig {
            path: root.to_path_buf(),
            output: None,
            max_tokens: 10_000,
            core_patterns: vec![],
            default_strategy: crate::config::OutputStrategy::Full,
            include_patterns: vec![],
            exclude_patterns: vec![],
            no_git: true,
            no_tree: false,
            copy: false,
            verbose: false,
        }
    }

    fn setup_rust_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        fs::write(base.join("src/lib.rs"), "/// Library docs\npub fn lib() -> i32 {\n    42\n}\n")
            .unwrap();
        fs::write(base.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        dir
    }

    #[test]
    fn test_run_ingestion_discovers_files() {
        let dir = setup_rust_project();
        let config = make_config(dir.path());
        let result = run_ingestion(&config).unwrap();

        assert!(!result.files.is_empty());
        assert!(result.files.iter().any(|f| f.path.ends_with("src/main.rs")));
        assert!(result.files.iter().any(|f| f.path.ends_with("src/lib.rs")));
    }

    #[test]
    fn test_run_ingestion_detects_languages() {
        let dir = setup_rust_project();
        let config = make_config(dir.path());
        let result = run_ingestion(&config).unwrap();

        let main_file = result.files.iter().find(|f| f.path.ends_with("src/main.rs")).unwrap();
        assert_eq!(main_file.language, Some(Language::Rust));
    }

    #[test]
    fn test_run_ingestion_counts_tokens() {
        let dir = setup_rust_project();
        let config = make_config(dir.path());
        let result = run_ingestion(&config).unwrap();

        for file in &result.files {
            assert!(file.raw_token_count > 0, "token count should be > 0");
        }
    }

    #[test]
    fn test_run_ingestion_with_include_patterns() {
        let dir = setup_rust_project();
        let mut config = make_config(dir.path());
        config.include_patterns = vec!["*.rs".to_string()];

        let result = run_ingestion(&config).unwrap();
        assert!(result.files.iter().all(|f| f.path.extension().is_some_and(|e| e == "rs")));
    }

    #[test]
    fn test_run_ingestion_with_exclude_patterns() {
        let dir = setup_rust_project();
        let mut config = make_config(dir.path());
        config.exclude_patterns = vec!["*.toml".to_string()];

        let result = run_ingestion(&config).unwrap();
        assert!(!result.files.iter().any(|f| f.path.ends_with("Cargo.toml")));
    }

    #[test]
    fn test_run_ingestion_no_tree() {
        let dir = setup_rust_project();
        let mut config = make_config(dir.path());
        config.no_tree = true;

        let result = run_ingestion(&config).unwrap();
        assert!(result.directory_tree.is_empty());
    }

    #[test]
    fn test_run_ingestion_generates_tree() {
        let dir = setup_rust_project();
        let config = make_config(dir.path());

        let result = run_ingestion(&config).unwrap();
        assert!(!result.directory_tree.is_empty());
        // Tree should contain the directory name and file names
        let tree = &result.directory_tree;
        assert!(tree.contains("src"), "tree should contain 'src' directory");
        assert!(tree.contains("main.rs"), "tree should contain 'main.rs'");
    }

    #[test]
    fn test_run_ingestion_no_git_flag() {
        let dir = setup_rust_project();
        let mut config = make_config(dir.path());
        config.no_git = true;

        let result = run_ingestion(&config).unwrap();
        assert!(result.git_context.is_none());
    }

    #[test]
    fn test_run_ingestion_reads_file_contents() {
        let dir = setup_rust_project();
        let config = make_config(dir.path());
        let result = run_ingestion(&config).unwrap();

        let main_file = result.files.iter().find(|f| f.path.ends_with("src/main.rs")).unwrap();
        assert!(main_file.content.contains("main"));
    }

    #[test]
    fn test_run_ingestion_with_python_files() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("app.py"), "def main():\n    pass\n").unwrap();
        fs::write(base.join("main.rs"), "fn main() {}\n").unwrap();

        let config = make_config(base);
        let result = run_ingestion(&config).unwrap();

        let py_file = result.files.iter().find(|f| f.path.ends_with("app.py")).unwrap();
        assert_eq!(py_file.language, Some(Language::Python));

        let rs_file = result.files.iter().find(|f| f.path.ends_with("main.rs")).unwrap();
        assert_eq!(rs_file.language, Some(Language::Rust));
    }

    #[test]
    fn test_run_ingestion_empty_directory() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        let result = run_ingestion(&config).unwrap();
        assert!(result.files.is_empty());
        assert!(result.git_context.is_none());
    }

    #[test]
    fn test_build_directory_tree_basic() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("README.md"),
        ];

        let tree = build_directory_tree(base, &files);
        assert!(tree.contains("src"));
        assert!(tree.contains("main.rs"));
        assert!(tree.contains("lib.rs"));
        assert!(tree.contains("README.md"));
    }

    #[test]
    fn test_run_ingestion_nested_directories() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src/utils/helpers")).unwrap();
        fs::write(base.join("src/utils/helpers/math.rs"), "pub fn add() {}").unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();

        let config = make_config(base);
        let result = run_ingestion(&config).unwrap();

        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.path.ends_with("src/utils/helpers/math.rs")));

        let tree = &result.directory_tree;
        assert!(tree.contains("utils"), "tree should contain 'utils'");
        assert!(tree.contains("helpers"), "tree should contain 'helpers'");
        assert!(tree.contains("math.rs"), "tree should contain 'math.rs'");
    }
}
