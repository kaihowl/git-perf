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

/// Returns hermetic git environment variables as an array of tuples.
///
/// This is useful for passing to `Command::envs()` when spawning processes
/// that need isolated git environment.
///
/// # Returns
/// Array of (key, value) tuples for hermetic git environment variables
pub fn hermetic_git_env_vars() -> [(&'static str, &'static str); 6] {
    [
        ("GIT_CONFIG_NOSYSTEM", "true"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_AUTHOR_NAME", "testuser"),
        ("GIT_AUTHOR_EMAIL", "testuser@example.com"),
        ("GIT_COMMITTER_NAME", "testuser"),
        ("GIT_COMMITTER_EMAIL", "testuser@example.com"),
    ]
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

/// Initializes a git repository in the specified directory with a real file commit.
///
/// Creates a test.txt file with "test content" and commits it.
///
/// # Arguments
/// * `dir` - Directory to initialize the repository in
///
/// # Panics
/// Panics if git initialization, file creation, or commit fails.
pub fn init_repo_with_file(dir: &Path) {
    run_git_command(&["init", "--initial-branch", "master"], dir);

    // Create a test file and commit it
    std::fs::write(dir.join("test.txt"), "test content").expect("Failed to create test file");
    run_git_command(&["add", "test.txt"], dir);
    run_git_command(&["commit", "-m", "Initial commit"], dir);
}

/// Creates a temporary directory with an initialized git repository and a file commit.
///
/// The repository will have:
/// - A master branch as the initial branch
/// - A test.txt file committed
///
/// # Returns
/// A `TempDir` that will be automatically cleaned up when dropped.
///
/// # Panics
/// Panics if the temporary directory cannot be created or git initialization fails.
pub fn dir_with_repo_and_file() -> TempDir {
    let tempdir = tempdir().unwrap();
    init_repo_with_file(tempdir.path());
    tempdir
}

/// Sets up an isolated HOME directory for config isolation tests.
///
/// This helper creates a temporary HOME directory and removes XDG_CONFIG_HOME
/// to ensure tests run in complete isolation. The original HOME is restored
/// after the test closure completes.
///
/// # Arguments
/// * `f` - Closure that takes the temporary home path and returns a result
///
/// # Returns
/// The result from the closure
pub fn with_isolated_home<F, R>(f: F) -> R
where
    F: FnOnce(&Path) -> R,
{
    let temp_dir = TempDir::new().unwrap();

    // Save original HOME
    let original_home = env::var("HOME").ok();
    let original_xdg = env::var("XDG_CONFIG_HOME").ok();

    // Set up isolated HOME directory
    env::set_var("HOME", temp_dir.path());
    env::remove_var("XDG_CONFIG_HOME");

    let result = f(temp_dir.path());

    // Restore original environment
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }
    if let Some(xdg) = original_xdg {
        env::set_var("XDG_CONFIG_HOME", xdg);
    }

    result
}

/// Writes a .gitperfconfig file in the specified directory.
///
/// # Arguments
/// * `dir` - Directory where .gitperfconfig should be written
/// * `content` - TOML content to write to the config file
///
/// # Panics
/// Panics if the file cannot be written.
pub fn write_gitperfconfig(dir: &Path, content: &str) {
    let config_path = dir.join(".gitperfconfig");
    std::fs::write(&config_path, content).expect("Failed to write .gitperfconfig");
}

/// Sets up a complete test environment with git repo and config.
///
/// This is a convenience function that:
/// 1. Sets up hermetic git environment variables
/// 2. Creates a temporary directory with an initialized git repository
/// 3. Writes the provided config content to .gitperfconfig
///
/// # Arguments
/// * `config_content` - TOML content for .gitperfconfig
///
/// # Returns
/// A `TempDir` that will be automatically cleaned up when dropped.
///
/// # Panics
/// Panics if any step fails.
pub fn setup_test_env_with_config(config_content: &str) -> TempDir {
    hermetic_git_env();
    let temp_dir = dir_with_repo();
    write_gitperfconfig(temp_dir.path(), config_content);
    temp_dir
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
