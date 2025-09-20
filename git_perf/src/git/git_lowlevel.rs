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
        GitError::ExecError { command: _, output } if output.stderr.contains("cannot lock ref") => {
            GitError::RefFailedToLock { output }
        }
        GitError::ExecError { command: _, output } if output.stderr.contains("but expected") => {
            GitError::RefConcurrentModification { output }
        }
        GitError::ExecError { command: _, output } if output.stderr.contains("find remote ref") => {
            GitError::NoRemoteMeasurements { output }
        }
        GitError::ExecError { command: _, output } if output.stderr.contains("bad object") => {
            GitError::BadObject { output }
        }
        _ => err,
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

pub(super) fn git_rev_parse_symbolic_ref(reference: &str) -> Option<String> {
    capture_git_output(&["symbolic-ref", "-q", reference], &None)
        .ok()
        .map(|s| s.stdout.trim().to_owned())
}

pub(super) fn is_shallow_repo() -> Result<bool, GitError> {
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

#[cfg(test)]
mod test {
    use super::*;
    use std::env::set_current_dir;

    use tempfile::{tempdir, TempDir};
    use serial_test::serial;

    fn run_git_command(args: &[&str], dir: &Path) {
        assert!(process::Command::new("git")
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

    fn init_repo(dir: &Path) {
        run_git_command(&["init", "--initial-branch", "master"], dir);
        run_git_command(&["commit", "--allow-empty", "-m", "Initial commit"], dir);
    }

    fn dir_with_repo() -> TempDir {
        let tempdir = tempdir().unwrap();
        init_repo(tempdir.path());
        tempdir
    }

    #[test]
    #[serial]
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
}
