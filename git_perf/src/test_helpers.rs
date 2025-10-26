//! Centralized test helpers for git-perf
//!
//! This module provides common test utilities used across unit tests and benchmarks,
//! including hermetic git environment setup and repository initialization helpers.

use std::env;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::{tempdir, TempDir};

/// Sets up a hermetic git environment by configuring environment variables
/// to isolate git operations from the user's global git configuration.
///
/// This function sets:
/// - `GIT_CONFIG_NOSYSTEM`: Disables system-wide git config
/// - `GIT_CONFIG_GLOBAL`: Points to /dev/null to ignore global config
/// - `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`: Test user identity
/// - `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL`: Test committer identity
pub fn hermetic_git_env() {
    env::set_var("GIT_CONFIG_NOSYSTEM", "true");
    env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
    env::set_var("GIT_AUTHOR_NAME", "testuser");
    env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
    env::set_var("GIT_COMMITTER_NAME", "testuser");
    env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");
}

/// Runs a git command in a hermetic environment with the specified directory.
///
/// # Arguments
/// * `args` - Git command arguments
/// * `dir` - Directory to run the command in
///
/// # Panics
/// Panics if the git command fails or returns a non-zero exit status.
pub fn run_git_command(args: &[&str], dir: &Path) {
    assert!(Command::new("git")
        .args(args)
        .envs([
            ("GIT_CONFIG_NOSYSTEM", "true"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_AUTHOR_NAME", "testuser"),
            ("GIT_AUTHOR_EMAIL", "testuser@example.com"),
            ("GIT_COMMITTER_NAME", "testuser"),
            ("GIT_COMMITTER_EMAIL", "testuser@example.com"),
        ])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to spawn git command")
        .success());
}

/// Initializes a git repository in the specified directory with an initial empty commit.
///
/// # Arguments
/// * `dir` - Directory to initialize the repository in
///
/// # Panics
/// Panics if git initialization or the initial commit fails.
pub fn init_repo(dir: &Path) {
    run_git_command(&["init", "--initial-branch", "master"], dir);
    run_git_command(&["commit", "--allow-empty", "-m", "Initial commit"], dir);
}

/// Creates a temporary directory with an initialized git repository.
///
/// The repository will have:
/// - A master branch as the initial branch
/// - An initial empty commit
///
/// # Returns
/// A `TempDir` that will be automatically cleaned up when dropped.
///
/// # Panics
/// Panics if the temporary directory cannot be created or git initialization fails.
pub fn dir_with_repo() -> TempDir {
    let tempdir = tempdir().unwrap();
    init_repo(tempdir.path());
    tempdir
}

/// Initializes a git repository in the current directory.
/// This is a simplified version for use in benchmarks that don't need a TempDir.
///
/// # Panics
/// Panics if git initialization fails.
pub fn init_repo_simple() {
    assert!(Command::new("git")
        .arg("init")
        .output()
        .expect("Failed to init git repo")
        .status
        .success());
}

/// Creates an empty commit in the current directory.
/// This is a simplified version for use in benchmarks.
///
/// # Panics
/// Panics if the commit fails.
pub fn empty_commit() {
    assert!(Command::new("git")
        .args(["commit", "--allow-empty", "-m", "test commit"])
        .output()
        .expect("Failed to create empty commit")
        .status
        .success());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::set_current_dir;

    #[test]
    fn test_hermetic_git_env() {
        hermetic_git_env();
        assert_eq!(env::var("GIT_CONFIG_NOSYSTEM").unwrap(), "true");
        assert_eq!(env::var("GIT_CONFIG_GLOBAL").unwrap(), "/dev/null");
        assert_eq!(env::var("GIT_AUTHOR_NAME").unwrap(), "testuser");
        assert_eq!(
            env::var("GIT_AUTHOR_EMAIL").unwrap(),
            "testuser@example.com"
        );
    }

    #[test]
    fn test_dir_with_repo() {
        let repo_dir = dir_with_repo();
        set_current_dir(repo_dir.path()).expect("Failed to change dir");

        // Verify the repository was initialized
        let output = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .expect("Failed to run git command");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "true");
    }

    #[test]
    fn test_init_repo() {
        let tempdir = tempdir().unwrap();
        init_repo(tempdir.path());

        set_current_dir(tempdir.path()).expect("Failed to change dir");

        // Verify the repository has at least one commit
        let output = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .output()
            .expect("Failed to run git command");

        assert!(output.status.success());
        let count = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<i32>()
            .unwrap();
        assert_eq!(count, 1);
    }
}
