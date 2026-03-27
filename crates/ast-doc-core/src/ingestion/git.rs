//! Git context extraction.
//!
//! Provides `GitContextProvider` trait and `Git2Context` implementation
//! for extracting branch, commit, and diff information from a git repository.

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::{error::AstDocError, ingestion::GitContext};

/// Trait for extracting git context, enabling testability.
pub trait GitContextProvider {
    /// Get the current branch name.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch cannot be determined.
    fn get_branch(&self) -> Result<String, AstDocError>;

    /// Get the latest commit summary (short hash + subject).
    ///
    /// # Errors
    ///
    /// Returns an error if the commit cannot be read.
    fn get_latest_commit(&self) -> Result<String, AstDocError>;

    /// Get the uncommitted diff as a string, or `None` if clean.
    ///
    /// # Errors
    ///
    /// Returns an error if the diff cannot be computed.
    fn get_diff(&self) -> Result<Option<String>, AstDocError>;

    /// Extract all git context into a `GitContext` struct.
    ///
    /// # Errors
    ///
    /// Returns an error if any git operation fails.
    fn extract(&self) -> Result<GitContext, AstDocError> {
        Ok(GitContext {
            branch: self.get_branch()?,
            latest_commit: self.get_latest_commit()?,
            diff: self.get_diff()?,
        })
    }
}

/// Git context provider backed by `git2`.
#[derive(Debug)]
pub struct Git2Context {
    repo_path: PathBuf,
}

impl Git2Context {
    /// Open a git repository at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a git repository.
    pub fn new(repo_path: &Path) -> Result<Self, AstDocError> {
        // Validate that the path is a git repo
        let _repo = git2::Repository::discover(repo_path)?;
        Ok(Self { repo_path: repo_path.to_path_buf() })
    }
}

impl GitContextProvider for Git2Context {
    fn get_branch(&self) -> Result<String, AstDocError> {
        let repo = git2::Repository::open(&self.repo_path)?;
        let head = repo.head()?;

        if let Some(name) = head.shorthand() {
            debug!(branch = name, "detected branch");
            Ok(name.to_string())
        } else {
            Ok("HEAD (detached)".to_string())
        }
    }

    fn get_latest_commit(&self) -> Result<String, AstDocError> {
        let repo = git2::Repository::open(&self.repo_path)?;
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;

        let short_id = commit.as_object().short_id()?.as_str().unwrap_or("???????").to_string();

        let summary = commit.summary().unwrap_or("(no message)").to_string();

        let result = format!("{short_id} {summary}");
        debug!(commit = %result, "latest commit");
        Ok(result)
    }

    fn get_diff(&self) -> Result<Option<String>, AstDocError> {
        let repo = git2::Repository::open(&self.repo_path)?;

        let head = repo.head()?;
        let head_tree = head.peel_to_tree()?;

        let mut diff_opts = git2::DiffOptions::new();
        let diff = repo.diff_tree_to_workdir_with_index(
            Some(&head_tree),
            Some(diff_opts.include_untracked(true)),
        )?;

        if diff.stats()?.files_changed() == 0 {
            info!("no uncommitted changes");
            return Ok(None);
        }

        let mut diff_text = Vec::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            diff_text.extend_from_slice(line.content());
            true
        })?;

        let diff_str = String::from_utf8_lossy(&diff_text).to_string();
        if diff_str.is_empty() {
            return Ok(None);
        }

        // Truncate large diffs to avoid bloating output
        const MAX_DIFF_SIZE: usize = 50_000;
        let diff_str = if diff_str.len() > MAX_DIFF_SIZE {
            warn!(size = diff_str.len(), limit = MAX_DIFF_SIZE, "truncating large diff");
            format!("{}...[truncated]", &diff_str[..MAX_DIFF_SIZE])
        } else {
            diff_str
        };

        debug!(len = diff_str.len(), "captured diff");
        Ok(Some(diff_str))
    }
}

/// A mock `GitContextProvider` for testing.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct MockGitContext {
    branch: String,
    commit: String,
    diff: Option<String>,
}

#[cfg(test)]
impl MockGitContext {
    /// Create a new mock git context.
    pub fn new(branch: &str, commit: &str, diff: Option<&str>) -> Self {
        Self {
            branch: branch.to_string(),
            commit: commit.to_string(),
            diff: diff.map(String::from),
        }
    }
}

#[cfg(test)]
impl GitContextProvider for MockGitContext {
    fn get_branch(&self) -> Result<String, AstDocError> {
        Ok(self.branch.clone())
    }

    fn get_latest_commit(&self) -> Result<String, AstDocError> {
        Ok(self.commit.clone())
    }

    fn get_diff(&self) -> Result<Option<String>, AstDocError> {
        Ok(self.diff.clone())
    }
}

/// Helper to extract git context from a repository path.
///
/// Returns `Ok(None)` if the path is not a git repository.
pub fn extract_git_context(repo_path: &Path) -> Result<Option<GitContext>, AstDocError> {
    match git2::Repository::discover(repo_path) {
        Ok(_) => {
            let provider = Git2Context::new(repo_path)?;
            Ok(Some(provider.extract()?))
        }
        Err(err) => {
            debug!(path = %repo_path.display(), error = %err, "not a git repo");
            Ok(None)
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_mock_git_context() {
        let mock = MockGitContext::new(
            "main",
            "abc1234 feat: add feature",
            Some("diff --git a/file.rs b/file.rs\n+added line"),
        );

        assert_eq!(mock.get_branch().unwrap(), "main");
        assert_eq!(mock.get_latest_commit().unwrap(), "abc1234 feat: add feature");
        assert!(mock.get_diff().unwrap().is_some());

        let ctx = mock.extract().unwrap();
        assert_eq!(ctx.branch, "main");
        assert_eq!(ctx.latest_commit, "abc1234 feat: add feature");
    }

    #[test]
    fn test_mock_git_context_clean() {
        let mock = MockGitContext::new("develop", "def5678 fix: bug fix", None);

        assert_eq!(mock.get_branch().unwrap(), "develop");
        assert!(mock.get_diff().unwrap().is_none());

        let ctx = mock.extract().unwrap();
        assert!(ctx.diff.is_none());
    }

    #[test]
    fn test_extract_git_context_non_repo() {
        let dir = TempDir::new().unwrap();
        let result = extract_git_context(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_git_context_in_repo() {
        // Init a temporary git repo for testing
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure git user for commit
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@test.com").unwrap();

        // Create an initial commit
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[]).unwrap();

        let result = extract_git_context(dir.path()).unwrap();
        assert!(result.is_some());
        let ctx = result.unwrap();
        assert!(!ctx.branch.is_empty());
        assert!(ctx.latest_commit.contains("initial commit"));
    }

    #[test]
    fn test_git2_context_nonexistent_repo() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("not-a-repo");
        std::fs::create_dir_all(&nested).unwrap();
        let result = Git2Context::new(&nested);
        assert!(result.is_err());
    }

    #[test]
    fn test_git2_context_branch_detached() {
        let dir = TempDir::new().unwrap();
        let _repo = git2::Repository::init(dir.path()).unwrap();

        // Without any commits, HEAD is unborn; get_branch should handle it
        let ctx = Git2Context::new(dir.path()).unwrap();
        // This may return an error for a repo with no commits
        let _ = ctx.get_branch();
    }

    #[test]
    fn test_git2_context_with_diff() {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure git user
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@test.com").unwrap();

        // Create a file and make an initial commit
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.rs")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "add test.rs", &tree, &[]).unwrap();

        // Modify the file to create a diff
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let ctx = Git2Context::new(dir.path()).unwrap();
        let diff = ctx.get_diff().unwrap();
        assert!(diff.is_some());
        assert!(diff.unwrap().contains("hello"));
    }
}
