use std::{
    env::current_dir,
    io,
    path::{Path, PathBuf},
    process::{self},
};

use anyhow::{bail, Context, Result};
use backoff::{Error, ExponentialBackoff};
use itertools::Itertools;
use thiserror::Error;

#[derive(Debug, Error)]
enum GitError {
    #[error("Git failed to execute, stdout:\n{stdout}\nstderr:\n{stderr}")]
    ExecError { stdout: String, stderr: String },

    #[error("Failed to execute git command")]
    IoError(#[from] io::Error),
}

fn run_git(args: &[&str], working_dir: &Option<&Path>) -> Result<String, GitError> {
    let working_dir = working_dir.map(PathBuf::from).unwrap_or(current_dir()?);

    let output = process::Command::new("git")
        // TODO(kaihowl) set correct encoding and lang?
        .env("LANG", "")
        .env("LC_ALL", "C")
        .current_dir(working_dir)
        .args(args)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(GitError::ExecError { stdout, stderr });
    }

    Ok(stdout)
}

const REFS_NOTES_BRANCH: &str = "refs/notes/perf";

pub fn add_note_line_to_head(line: &str) -> Result<()> {
    run_git(
        &[
            "notes",
            "--ref",
            REFS_NOTES_BRANCH,
            "append",
            "--no-separator",
            "-m",
            line,
        ],
        &None,
    )
    .context("Failed to add new measurement")?;

    Ok(())
}

pub fn get_head_revision() -> Result<String> {
    let head = run_git(&["rev-parse", "HEAD"], &None).context("Failed to parse HEAD.")?;

    Ok(head.trim().to_owned())
}
pub fn fetch(work_dir: Option<&Path>) -> Result<()> {
    // Use git directly to avoid having to implement ssh-agent and/or extraHeader handling
    run_git(&["fetch", "origin", REFS_NOTES_BRANCH], &work_dir)
        .context("Failed to fetch performance measurements.")?;

    Ok(())
}

pub fn reconcile() -> Result<()> {
    let _ = run_git(
        &[
            "notes",
            "--ref",
            REFS_NOTES_BRANCH,
            "merge",
            "-s",
            "cat_sort_uniq",
            "FETCH_HEAD",
        ],
        &None,
    )
    .context("Failed to merge measurements with upstream")?;
    Ok(())
}

#[derive(Debug, Error)]
enum PushError {
    #[error("A ref failed to be pushed:\n{stdout}\n{stderr}")]
    RefFailedToPush { stdout: String, stderr: String },
}

pub fn raw_push(work_dir: Option<&Path>) -> Result<()> {
    // TODO(kaihowl) configure remote?
    // TODO(kaihowl) factor into constants
    // TODO(kaihowl) capture output
    let output = run_git(
        &[
            "push",
            "--porcelain",
            "origin",
            format!("{REFS_NOTES_BRANCH}:{REFS_NOTES_BRANCH}").as_str(),
        ],
        &work_dir,
    );

    dbg!(&output);

    match output {
        Ok(_) => Ok(()),
        Err(GitError::ExecError { stdout, stderr }) => {
            for line in stdout.lines() {
                if !line.contains(format!("{REFS_NOTES_BRANCH}:").as_str()) {
                    continue;
                }
                if !line.starts_with('!') {
                    return Ok(());
                }
            }
            bail!(PushError::RefFailedToPush { stdout, stderr })
        }
        Err(e) => bail!(e),
    }
}

// TODO(kaihowl) what happens with a git dir supplied with -C?
pub fn prune() -> Result<()> {
    if is_shallow_repo().context("Could not determine if shallow clone.")? {
        // TODO(kaihowl) is this not already checked by git itself?
        bail!("Refusing to prune on a shallow repo")
    }

    run_git(&["notes", "--ref", REFS_NOTES_BRANCH, "prune"], &None).context("Failed to prune.")?;

    Ok(())
}

fn is_shallow_repo() -> Result<bool> {
    let output = run_git(&["rev-parse", "--is-shallow-repository"], &None)
        .context("Failed to determine if repo is a shallow clone.")?;

    Ok(output.starts_with("true"))
}

// TODO(kaihowl) return a nested iterator / generator instead?
pub fn walk_commits(num_commits: usize) -> Result<Vec<(String, Vec<String>)>> {
    let output = run_git(
        &[
            "--no-pager",
            "log",
            "--no-color",
            "--ignore-missing",
            "-n",
            num_commits.to_string().as_str(),
            "--first-parent",
            "--pretty=--,%H,%D%n%N",
            "--decorate=full",
            format!("--notes={REFS_NOTES_BRANCH}").as_str(),
            "HEAD",
        ],
        &None,
    )
    .context("Failed to retrieve commits")?;

    let mut current_commit = None;
    let mut detected_shallow = false;

    // TODO(kaihowl) iterator or generator instead / how to propagate exit code?
    let it = output.lines().filter_map(|l| {
        if l.starts_with("--") {
            let info = l.split(',').collect_vec();

            current_commit = Some(
                info.get(1)
                    .expect("Could not read commit header.")
                    .to_owned(),
            );

            detected_shallow |= info[2..].iter().any(|s| *s == "grafted");

            None
        } else {
            // TODO(kaihowl) lot's of string copies...
            Some((
                current_commit.as_ref().expect("TODO(kaihowl)").to_owned(),
                l,
            ))
        }
    });

    let commits: Vec<_> = it
        .group_by(|it| it.0.to_owned())
        .into_iter()
        .map(|(k, v)| {
            (
                k.to_owned(),
                // TODO(kaihowl) joining what was split above already
                // TODO(kaihowl) lot's of string copies...
                v.map(|(_, v)| v.to_owned()).collect::<Vec<_>>(),
            )
        })
        .collect();

    if detected_shallow && commits.len() < num_commits {
        bail!("Refusing to continue as commit log depth was limited by shallow clone");
    }

    Ok(commits)
}

pub fn pull(work_dir: Option<&Path>) -> Result<()> {
    fetch(work_dir)?;
    reconcile()
}

pub fn push(work_dir: Option<&Path>) -> Result<()> {
    // TODO(kaihowl) configure
    let backoff = ExponentialBackoff::default();

    // TODO(kaihowl) check transient/permanent error
    let op = || -> Result<(), backoff::Error<anyhow::Error>> {
        raw_push(work_dir).map_err(|e| match e.downcast_ref::<PushError>() {
            Some(PushError::RefFailedToPush { .. }) => match pull(work_dir) {
                Err(pull_error) => Error::permanent(pull_error),
                Ok(_) => Error::transient(e),
            },
            None => Error::Permanent(e),
        })
    };

    backoff::retry(backoff, op).map_err(|e| match e {
        Error::Permanent(e) => e.context("Permanent failure while pushing refs"),
        Error::Transient { err, .. } => err.context("Timed out pushing refs"),
    })?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env::{self, set_current_dir};

    use httptest::{
        http::{header::AUTHORIZATION, Uri},
        matchers::{self, request},
        responders::status_code,
        Expectation, Server,
    };
    use tempfile::{tempdir, TempDir};

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

    fn dir_with_repo_and_customheader(origin_url: Uri, extra_header: &str) -> TempDir {
        let tempdir = dir_with_repo();

        let url = origin_url.to_string();

        run_git_command(&["remote", "add", "origin", &url], tempdir.path());
        run_git_command(
            &[
                "config",
                "--add",
                format!("http.{}.extraHeader", url).as_str(),
                extra_header,
            ],
            tempdir.path(),
        );

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

    #[test]
    fn test_customheader_push() {
        let test_server = Server::run();
        let repo_dir =
            dir_with_repo_and_customheader(test_server.url(""), "AUTHORIZATION: sometoken");
        set_current_dir(repo_dir.path()).expect("Failed to change dir");

        test_server.expect(
            Expectation::matching(request::headers(matchers::contains((
                AUTHORIZATION.as_str(),
                "sometoken",
            ))))
            .times(1..)
            .respond_with(status_code(200)),
        );

        // TODO(kaihowl) not so great test as this fails with/without authorization
        // We only want to verify that a call on the server with the authorization header was
        // received.
        hermetic_git_env();
        pull(Some(repo_dir.path()))
            .expect_err("We have no valid git http server setup -> should fail");
    }

    #[test]
    fn test_customheader_pull() {
        let test_server = Server::run();
        let repo_dir =
            dir_with_repo_and_customheader(test_server.url(""), "AUTHORIZATION: someothertoken");
        set_current_dir(&repo_dir).expect("Failed to change dir");

        test_server.expect(
            Expectation::matching(request::headers(matchers::contains((
                AUTHORIZATION.as_str(),
                "someothertoken",
            ))))
            .times(1..)
            .respond_with(status_code(200)),
        );

        // TODO(kaihowl) duplication, leaks out of this test
        hermetic_git_env();
        let error = push(Some(repo_dir.path()));
        dbg!(&error);
        let error = error.unwrap_err().root_cause().to_string();
        dbg!(&error);
        assert!(error.contains("couldn't find remote ref"));
    }

    #[test]
    fn test_get_head_revision() {
        let repo_dir = dir_with_repo();
        set_current_dir(repo_dir.path()).expect("Failed to change dir");
        let revision = get_head_revision().unwrap();
        assert!(
            &revision.chars().all(|c| c.is_ascii_alphanumeric()),
            "'{}' contained non alphanumeric or non ASCII characters",
            &revision
        )
    }
}
