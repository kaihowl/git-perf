use super::{
    git_definitions::EXPECTED_VERSION,
    git_types::{GitError, GitOutput},
};

use std::{
    env::current_dir,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    process::{self, Child, Stdio},
};

use log::{debug, trace};

use anyhow::{anyhow, bail, Context, Result};
use itertools::Itertools;

pub(super) fn spawn_git_command(
    args: &[&str],
    working_dir: &Option<&Path>,
    stdin: Option<Stdio>,
) -> Result<Child, io::Error> {
    let working_dir = working_dir.map(PathBuf::from).unwrap_or(current_dir()?);
    // Disable Git's automatic maintenance to prevent interference with concurrent operations
    let default_pre_args = [
        "-c",
        "gc.auto=0",
        "-c",
        "maintenance.auto=0",
        "-c",
        "fetch.fsckObjects=false",
    ];
    let stdin = stdin.unwrap_or(Stdio::null());
    let all_args: Vec<_> = default_pre_args.iter().chain(args.iter()).collect();
    debug!("execute: git {}", all_args.iter().join(" "));
    process::Command::new("git")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("LANGUAGE", "C.UTF-8")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(working_dir)
        .args(all_args)
        .spawn()
}

pub(super) fn capture_git_output(
    args: &[&str],
    working_dir: &Option<&Path>,
) -> Result<GitOutput, GitError> {
    feed_git_command(args, working_dir, None)
}

pub(super) fn feed_git_command(
    args: &[&str],
    working_dir: &Option<&Path>,
    input: Option<&str>,
) -> Result<GitOutput, GitError> {
    let stdin = input.map(|_| Stdio::piped());

    let child = spawn_git_command(args, working_dir, stdin)?;

    debug!("input: {}", input.unwrap_or(""));

    let output = match child.stdin {
        Some(ref stdin) => {
            let mut writer = BufWriter::new(stdin);
            writer.write_all(input.unwrap().as_bytes())?;
            drop(writer);
            child.wait_with_output()
        }
        None => child.wait_with_output(),
    }?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    trace!("stdout: {stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    trace!("stderr: {stderr}");

    let git_output = GitOutput { stdout, stderr };

    if output.status.success() {
        trace!("exec succeeded");
        Ok(git_output)
    } else {
        trace!("exec failed");
        Err(GitError::ExecError {
            command: args.join(" "),
            output: git_output,
        })
    }
}

pub(super) fn map_git_error(err: GitError) -> GitError {
    // Parsing error messasges is not a very good idea, but(!) there are no consistent + documented error code for these cases.
    // This is tested by the git compatibility check and we add an explicit LANG to the git invocation.
    match err {
        GitError::ExecError { output, .. } if output.stderr.contains("cannot lock ref") => {
            GitError::RefFailedToLock { output }
        }
        GitError::ExecError { output, .. } if output.stderr.contains("but expected") => {
            GitError::RefConcurrentModification { output }
        }
        GitError::ExecError { output, .. } if output.stderr.contains("find remote ref") => {
            GitError::NoRemoteMeasurements { output }
        }
        GitError::ExecError { output, .. } if output.stderr.contains("bad object") => {
            GitError::BadObject { output }
        }
        GitError::ExecError { .. }
        | GitError::RefFailedToPush { .. }
        | GitError::MissingHead { .. }
        | GitError::RefFailedToLock { .. }
        | GitError::ShallowRepository
        | GitError::MissingMeasurements
        | GitError::RefConcurrentModification { .. }
        | GitError::NoRemoteMeasurements { .. }
        | GitError::NoUpstream {}
        | GitError::BadObject { .. }
        | GitError::IoError(_) => err,
    }
}

pub(super) fn get_git_perf_remote(remote: &str) -> Option<String> {
    capture_git_output(&["remote", "get-url", remote], &None)
        .ok()
        .map(|s| s.stdout.trim().to_owned())
}

pub(super) fn set_git_perf_remote(remote: &str, url: &str) -> Result<(), GitError> {
    capture_git_output(&["remote", "add", remote, url], &None).map(|_| ())
}

pub(super) fn git_update_ref(commands: impl AsRef<str>) -> Result<(), GitError> {
    feed_git_command(
        &[
            "update-ref",
            // When updating existing symlinks, we want to update the source symlink and not its target
            "--no-deref",
            "--stdin",
        ],
        &None,
        Some(commands.as_ref()),
    )
    .map_err(map_git_error)
    .map(|_| ())
}

pub fn get_head_revision() -> Result<String> {
    Ok(internal_get_head_revision()?)
}

pub(super) fn internal_get_head_revision() -> Result<String, GitError> {
    git_rev_parse("HEAD")
}

pub(super) fn git_rev_parse(reference: &str) -> Result<String, GitError> {
    capture_git_output(&["rev-parse", "--verify", "-q", reference], &None)
        .map_err(|_e| GitError::MissingHead {
            reference: reference.into(),
        })
        .map(|s| s.stdout.trim().to_owned())
}

/// Resolve a committish (commit, branch, tag, HEAD~3, etc.) to a full SHA-1 hash
pub fn resolve_committish(committish: &str) -> Result<String> {
    git_rev_parse(committish).map_err(|e| anyhow!(e))
}

pub(super) fn git_rev_parse_symbolic_ref(reference: &str) -> Option<String> {
    capture_git_output(&["symbolic-ref", "-q", reference], &None)
        .ok()
        .map(|s| s.stdout.trim().to_owned())
}

pub(super) fn git_symbolic_ref_create_or_update(
    reference: &str,
    target: &str,
) -> Result<(), GitError> {
    capture_git_output(&["symbolic-ref", reference, target], &None)
        .map_err(map_git_error)
        .map(|_| ())
}

pub fn is_shallow_repo() -> Result<bool, GitError> {
    let output = capture_git_output(&["rev-parse", "--is-shallow-repository"], &None)?;

    Ok(output.stdout.starts_with("true"))
}

pub(super) fn parse_git_version(version: &str) -> Result<(i32, i32, i32)> {
    let version = version
        .split_whitespace()
        .nth(2)
        .ok_or(anyhow!("Could not find git version in string {version}"))?;
    match version.split('.').collect_vec()[..] {
        [major, minor, patch] => Ok((major.parse()?, minor.parse()?, patch.parse()?)),
        _ => Err(anyhow!("Failed determine semantic version from {version}")),
    }
}

fn get_git_version() -> Result<(i32, i32, i32)> {
    let version = capture_git_output(&["--version"], &None)
        .context("Determine git version")?
        .stdout;
    parse_git_version(&version)
}

fn concat_version(version_tuple: (i32, i32, i32)) -> String {
    format!(
        "{}.{}.{}",
        version_tuple.0, version_tuple.1, version_tuple.2
    )
}

pub fn check_git_version() -> Result<()> {
    let version_tuple = get_git_version().context("Determining compatible git version")?;
    if version_tuple < EXPECTED_VERSION {
        bail!(
            "Version {} is smaller than {}",
            concat_version(version_tuple),
            concat_version(EXPECTED_VERSION)
        )
    }
    Ok(())
}

/// Get the repository root directory using git
pub fn get_repository_root() -> Result<String, String> {
    let output = capture_git_output(&["rev-parse", "--show-toplevel"], &None)
        .map_err(|e| format!("Failed to get repository root: {}", e))?;
    Ok(output.stdout.trim().to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::dir_with_repo;
    use std::env::set_current_dir;

    #[test]
    fn test_get_head_revision() {
        let repo_dir = dir_with_repo();
        set_current_dir(repo_dir.path()).expect("Failed to change dir");
        let revision = internal_get_head_revision().unwrap();
        assert!(
            &revision.chars().all(|c| c.is_ascii_alphanumeric()),
            "'{}' contained non alphanumeric or non ASCII characters",
            &revision
        )
    }

    #[test]
    fn test_parse_git_version() {
        let version = parse_git_version("git version 2.52.0");
        assert_eq!(version.unwrap(), (2, 52, 0));

        let version = parse_git_version("git version 2.52.0\n");
        assert_eq!(version.unwrap(), (2, 52, 0));
    }

    #[test]
    fn test_map_git_error_ref_failed_to_lock() {
        let output = GitOutput {
            stdout: String::new(),
            stderr: "fatal: cannot lock ref 'refs/heads/main': Unable to create lock".to_string(),
        };
        let error = GitError::ExecError {
            command: "update-ref".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        assert!(matches!(mapped, GitError::RefFailedToLock { .. }));
    }

    #[test]
    fn test_map_git_error_ref_concurrent_modification() {
        let output = GitOutput {
            stdout: String::new(),
            stderr: "fatal: ref updates forbidden, but expected commit abc123".to_string(),
        };
        let error = GitError::ExecError {
            command: "update-ref".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        assert!(matches!(mapped, GitError::RefConcurrentModification { .. }));
    }

    #[test]
    fn test_map_git_error_no_remote_measurements() {
        let output = GitOutput {
            stdout: String::new(),
            stderr: "fatal: couldn't find remote ref refs/notes/measurements".to_string(),
        };
        let error = GitError::ExecError {
            command: "fetch".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        assert!(matches!(mapped, GitError::NoRemoteMeasurements { .. }));
    }

    #[test]
    fn test_map_git_error_bad_object() {
        let output = GitOutput {
            stdout: String::new(),
            stderr: "error: bad object abc123def456".to_string(),
        };
        let error = GitError::ExecError {
            command: "cat-file".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        assert!(matches!(mapped, GitError::BadObject { .. }));
    }

    #[test]
    fn test_map_git_error_unmapped() {
        let output = GitOutput {
            stdout: String::new(),
            stderr: "fatal: some other error".to_string(),
        };
        let error = GitError::ExecError {
            command: "status".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        // Should remain as ExecError for unrecognized patterns
        assert!(matches!(mapped, GitError::ExecError { .. }));
    }

    #[test]
    fn test_map_git_error_false_positive_avoidance() {
        // Test that partial matches don't trigger false positives
        let output = GitOutput {
            stdout: String::new(),
            stderr: "this message mentions 'lock' without the full pattern".to_string(),
        };
        let error = GitError::ExecError {
            command: "test".to_string(),
            output,
        };

        let mapped = map_git_error(error);
        // Should NOT be mapped to RefFailedToLock
        assert!(matches!(mapped, GitError::ExecError { .. }));
    }

    #[test]
    fn test_map_git_error_cannot_lock_ref_pattern_must_match() {
        // Test that "cannot lock ref" must be present (not just "lock")
        let test_cases = vec![
            ("fatal: cannot lock ref 'refs/heads/main'", true),
            ("error: cannot lock ref update", true),
            ("fatal: failed to lock something", false),
            ("error: lock failed", false),
        ];

        for (stderr_msg, should_map) in test_cases {
            let output = GitOutput {
                stdout: String::new(),
                stderr: stderr_msg.to_string(),
            };
            let error = GitError::ExecError {
                command: "test".to_string(),
                output,
            };

            let mapped = map_git_error(error);
            if should_map {
                assert!(
                    matches!(mapped, GitError::RefFailedToLock { .. }),
                    "Expected RefFailedToLock for: {}",
                    stderr_msg
                );
            } else {
                assert!(
                    matches!(mapped, GitError::ExecError { .. }),
                    "Expected ExecError for: {}",
                    stderr_msg
                );
            }
        }
    }

    #[test]
    fn test_map_git_error_but_expected_pattern_must_match() {
        // Test that "but expected" must be present
        let test_cases = vec![
            ("fatal: but expected commit abc123", true),
            ("error: ref update failed but expected something", true),
            ("fatal: expected something", false),
            ("error: only mentioned the word but", false),
        ];

        for (stderr_msg, should_map) in test_cases {
            let output = GitOutput {
                stdout: String::new(),
                stderr: stderr_msg.to_string(),
            };
            let error = GitError::ExecError {
                command: "test".to_string(),
                output,
            };

            let mapped = map_git_error(error);
            if should_map {
                assert!(
                    matches!(mapped, GitError::RefConcurrentModification { .. }),
                    "Expected RefConcurrentModification for: {}",
                    stderr_msg
                );
            } else {
                assert!(
                    matches!(mapped, GitError::ExecError { .. }),
                    "Expected ExecError for: {}",
                    stderr_msg
                );
            }
        }
    }
}
