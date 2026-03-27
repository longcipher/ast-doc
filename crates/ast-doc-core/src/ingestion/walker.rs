//! Directory walker with .gitignore and .astdocignore support.
//!
//! Uses `ignore::WalkBuilder` for directory traversal, respecting
//! .gitignore and .astdocignore rules. Applies glob-based include/exclude
//! filtering via `globset`.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use tracing::{debug, info};

use crate::{config::AstDocConfig, error::AstDocError};

/// Build a `GlobSet` from a list of glob patterns.
///
/// # Errors
///
/// Returns an error if any pattern is invalid.
pub fn build_globset(patterns: &[String]) -> Result<GlobSet, AstDocError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

/// Walk the project directory and return discovered file paths.
///
/// Respects .gitignore and .astdocignore. Applies include/exclude glob filters.
/// The `root` parameter is the canonicalized project root path.
///
/// # Errors
///
/// Returns an error if the directory cannot be walked.
pub fn walk_directory(
    root: &Path,
    include: &GlobSet,
    exclude: &GlobSet,
    _config: &AstDocConfig,
) -> Result<Vec<PathBuf>, AstDocError> {
    info!(path = %root.display(), "walking directory");

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .add_custom_ignore_filename(".astdocignore")
        .follow_links(false)
        .build();

    let mut files = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                debug!(error = %err, "skipping entry due to error");
                continue;
            }
        };

        // Skip directories
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            continue;
        }

        let path = entry.path();

        // Compute relative path from root
        let relative = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => path,
        };

        if should_include_file(relative, include, exclude) {
            debug!(path = %relative.display(), "including file");
            files.push(relative.to_path_buf());
        }
    }

    files.sort();
    info!(count = files.len(), "discovered files");
    Ok(files)
}

/// Determine whether a file should be included based on include/exclude globs.
///
/// - If include is empty, all files pass the include check.
/// - Exclude wins over include.
#[must_use]
pub fn should_include_file(path: &Path, include: &GlobSet, exclude: &GlobSet) -> bool {
    let included = include.is_empty() || include.is_match(path);
    let excluded = exclude.is_match(path);
    included && !excluded
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

    fn setup_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("src/lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(base.join("README.md"), "# Project").unwrap();
        fs::write(base.join("notes.txt"), "notes").unwrap();
        dir
    }

    #[test]
    fn test_walk_discovers_source_files() {
        let dir = setup_project();
        let config = make_config(dir.path());
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(dir.path(), &include, &exclude, &config).unwrap();
        assert!(files.iter().any(|p| p.ends_with("src/main.rs")));
        assert!(files.iter().any(|p| p.ends_with("src/lib.rs")));
        assert!(files.iter().any(|p| p.ends_with("README.md")));
        assert!(files.iter().any(|p| p.ends_with("notes.txt")));
    }

    #[test]
    fn test_walk_with_include_filter() {
        let dir = setup_project();
        let config = make_config(dir.path());
        let include = build_globset(&["*.rs".to_string()]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(dir.path(), &include, &exclude, &config).unwrap();
        assert!(files.iter().all(|p| p.extension().is_some_and(|e| e == "rs")));
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_walk_with_exclude_filter() {
        let dir = setup_project();
        let config = make_config(dir.path());
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&["*.txt".to_string()]).unwrap();

        let files = walk_directory(dir.path(), &include, &exclude, &config).unwrap();
        assert!(!files.iter().any(|p| p.ends_with("notes.txt")));
        assert!(files.iter().any(|p| p.ends_with("src/main.rs")));
    }

    #[test]
    fn test_walk_include_and_exclude_combined() {
        let dir = setup_project();
        let config = make_config(dir.path());
        let include = build_globset(&["*.rs".to_string(), "*.md".to_string()]).unwrap();
        let exclude = build_globset(&["README*".to_string()]).unwrap();

        let files = walk_directory(dir.path(), &include, &exclude, &config).unwrap();
        assert!(files.iter().all(|p| p.extension().is_some_and(|e| e == "rs")));
        assert!(!files.iter().any(|p| p.ends_with("README.md")));
    }

    #[test]
    fn test_should_include_file_include_empty() {
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&["*.txt".to_string()]).unwrap();

        assert!(should_include_file(Path::new("foo.rs"), &include, &exclude));
        assert!(!should_include_file(Path::new("foo.txt"), &include, &exclude));
    }

    #[test]
    fn test_should_include_file_exclude_wins() {
        let include = build_globset(&["*.rs".to_string()]).unwrap();
        let exclude = build_globset(&["*.rs".to_string()]).unwrap();

        // exclude wins over include
        assert!(!should_include_file(Path::new("foo.rs"), &include, &exclude));
    }

    #[test]
    fn test_should_include_file_non_matching_include() {
        let include = build_globset(&["*.py".to_string()]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        assert!(!should_include_file(Path::new("foo.rs"), &include, &exclude));
        assert!(should_include_file(Path::new("foo.py"), &include, &exclude));
    }

    #[test]
    fn test_build_globset_invalid_pattern() {
        let result = build_globset(&["[".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_walk_respects_astdocignore() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("src/generated.rs"), "// generated").unwrap();
        fs::write(base.join(".astdocignore"), "src/generated.rs\n").unwrap();

        let config = make_config(base);
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(base, &include, &exclude, &config).unwrap();
        assert!(files.iter().any(|p| p.ends_with("src/main.rs")));
        assert!(!files.iter().any(|p| p.ends_with("src/generated.rs")));
    }

    #[test]
    fn test_walk_respects_gitignore() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("src/secret.rs"), "// secret").unwrap();
        fs::write(base.join(".gitignore"), "src/secret.rs\n").unwrap();

        // Initialize a git repo so ignore crate picks up .gitignore
        let _repo = git2::Repository::init(base).unwrap();

        let config = make_config(base);
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(base, &include, &exclude, &config).unwrap();
        assert!(files.iter().any(|p| p.ends_with("src/main.rs")));
        assert!(!files.iter().any(|p| p.ends_with("src/secret.rs")));
    }

    #[test]
    fn test_walk_hidden_directories_excluded() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join(".hidden")).unwrap();
        fs::write(base.join(".hidden/file.rs"), "fn x() {}").unwrap();
        fs::write(base.join("visible.rs"), "fn y() {}").unwrap();

        let config = make_config(base);
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(base, &include, &exclude, &config).unwrap();
        assert!(files.iter().any(|p| p.ends_with("visible.rs")));
        assert!(!files.iter().any(|p| p.to_string_lossy().contains(".hidden")));
    }

    #[test]
    fn test_walk_output_is_sorted() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/zz.rs"), "fn z() {}").unwrap();
        fs::write(base.join("src/aa.rs"), "fn a() {}").unwrap();
        fs::write(base.join("src/mm.rs"), "fn m() {}").unwrap();

        let config = make_config(base);
        let include = build_globset(&[]).unwrap();
        let exclude = build_globset(&[]).unwrap();

        let files = walk_directory(base, &include, &exclude, &config).unwrap();
        let paths: Vec<_> = files.iter().map(|p| p.to_string_lossy().to_string()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }
}
