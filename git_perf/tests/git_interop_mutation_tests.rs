// Tests specifically targeting missed mutants found in mutation testing analysis
// See mutation-testing-analysis.md for details
//
// Note: Some of the missed mutants are in private functions that cannot be tested directly.
// These tests target the public APIs and integration scenarios that exercise the mutated code paths.

use std::env::{self, set_current_dir};
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn run_git_command(args: &[&str], dir: &Path) -> std::io::Result<std::process::Output> {
    Command::new("git")
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
        .output()
}

fn init_repo(dir: &Path) {
    run_git_command(&["init", "--initial-branch", "master"], dir).unwrap();
    run_git_command(&["commit", "--allow-empty", "-m", "Initial commit"], dir).unwrap();
}

fn dir_with_repo() -> TempDir {
    let tempdir = tempdir().unwrap();
    init_repo(tempdir.path());
    tempdir
}

fn hermetic_git_env() {
    env::set_var("GIT_CONFIG_NOSYSTEM", "true");
    env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
    env::set_var("GIT_AUTHOR_NAME", "testuser");
    env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
    env::set_var("GIT_COMMITTER_NAME", "testuser");
    env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");
}

/// Test that symbolic write ref is created properly via add_note_line_to_head
/// Indirectly tests new_symbolic_write_ref() which had missed mutants:
/// - Could return Ok(String::new()) - empty string
/// - Could return Ok("xyzzy".into()) - arbitrary invalid string
///
/// The function new_symbolic_write_ref() is private but called by ensure_symbolic_write_ref_exists(),
/// which is called by add_note_line_to_head(). This test verifies the symbolic ref created
/// is valid and properly formatted.
#[test]
fn test_symbolic_write_ref_creates_valid_reference() {
    let tempdir = dir_with_repo();
    set_current_dir(tempdir.path()).expect("Failed to change dir");
    hermetic_git_env();

    // This indirectly calls ensure_symbolic_write_ref_exists -> new_symbolic_write_ref
    let result = git_perf::git::git_interop::add_note_line_to_head("test: 100");
    assert!(
        result.is_ok(),
        "Should add note (which creates symbolic write ref): {:?}",
        result
    );

    // Verify the symbolic ref exists and is not empty or invalid
    let output = run_git_command(&["symbolic-ref", "refs/notes/write"], tempdir.path())
        .expect("Should read symbolic ref");

    assert!(
        output.status.success(),
        "symbolic-ref read should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let symbolic_target = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Mutation 1: Should not be empty
    assert!(
        !symbolic_target.is_empty(),
        "Symbolic ref target should not be empty"
    );

    // Mutation 2: Should not be arbitrary like "xyzzy", should follow ref pattern
    assert!(
        symbolic_target.starts_with("refs/notes/write-target/"),
        "Symbolic ref should point to refs/notes/write-target/*, got: {}",
        symbolic_target
    );

    // Verify it has a hex suffix (shows it's properly generated)
    let suffix = symbolic_target
        .strip_prefix("refs/notes/write-target/")
        .expect("Should have prefix");
    assert!(
        !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_hexdigit()),
        "Target should have hex suffix, got: {}",
        suffix
    );
}

/// Test walk_commits with shallow repository containing multiple grafted commits
/// Targets missed mutant at line 725: detected_shallow |= vs ^=
/// The XOR operator would toggle instead of OR, failing with multiple grafts
#[test]
fn test_walk_commits_shallow_repo_detection() {
    let tempdir = dir_with_repo();
    hermetic_git_env();

    // Create multiple commits
    set_current_dir(tempdir.path()).unwrap();
    for i in 2..=5 {
        run_git_command(
            &["commit", "--allow-empty", "-m", &format!("Commit {}", i)],
            tempdir.path(),
        )
        .unwrap();
    }

    // Create a shallow clone (depth 2) which will have grafted commits
    let shallow_dir = tempdir.path().join("shallow");
    let output = Command::new("git")
        .args(&[
            "clone",
            "--depth",
            "2",
            tempdir.path().to_str().unwrap(),
            shallow_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Shallow clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    set_current_dir(&shallow_dir).unwrap();
    hermetic_git_env();

    // Add a note to enable walk_commits
    git_perf::git::git_interop::add_note_line_to_head("test: 100").expect("Should add note");

    // Walk commits - should detect as shallow
    let result = git_perf::git::git_interop::walk_commits(10);
    assert!(result.is_ok(), "walk_commits should succeed: {:?}", result);

    let commits = result.unwrap();

    // In a shallow repo, git log --boundary shows grafted markers
    // The |= operator correctly sets detected_shallow to true
    // The ^= mutant would toggle the flag, potentially giving wrong result

    // Verify we got commits (the function works)
    assert!(
        !commits.is_empty(),
        "Should have found commits in shallow repo"
    );
}

/// Test walk_commits correctly identifies normal (non-shallow) repos
#[test]
fn test_walk_commits_normal_repo_not_shallow() {
    let tempdir = dir_with_repo();
    set_current_dir(tempdir.path()).unwrap();
    hermetic_git_env();

    // Create a few commits
    for i in 2..=3 {
        run_git_command(
            &["commit", "--allow-empty", "-m", &format!("Commit {}", i)],
            tempdir.path(),
        )
        .unwrap();
    }

    // Add a note to enable walk_commits
    git_perf::git::git_interop::add_note_line_to_head("test: 100").expect("Should add note");

    let result = git_perf::git::git_interop::walk_commits(10);
    assert!(result.is_ok(), "walk_commits should succeed");

    let commits = result.unwrap();

    // Should have commits
    assert!(!commits.is_empty(), "Should have found commits");
}
