use std::{
    env::current_dir,
    path::{Path, PathBuf},
    process::{self},
};

use anyhow::{bail, Context, Result};
use git2::{Index, Repository};
use itertools::Itertools;

fn run_git(args: &[&str], working_dir: &Option<&Path>) -> Result<String> {
    let working_dir = working_dir
        .map(PathBuf::from)
        .unwrap_or(current_dir().context("Failed to retrieve current directory")?);

    let output = process::Command::new("git")
        // TODO(kaihowl) set correct encoding and lang?
        .env("LANG", "")
        .env("LC_ALL", "C")
        .current_dir(working_dir)
        .args(args)
        .output()
        .context("Failed to spawn git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Git command failed to run: {}", stderr);
    }

    Ok(String::from_utf8(output.stdout)?)
}

pub fn add_note_line_to_head(line: &str) -> Result<()> {
    run_git(
        &[
            "notes",
            "--ref",
            "refs/notes/perf",
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

/// Resolve conflicts between two measurement runs on the same commit by
/// sorting and deduplicating lines.
/// This emulates the cat_sort_uniq merge strategy for git notes.
fn resolve_conflicts(ours: impl AsRef<str>, theirs: impl AsRef<str>) -> String {
    ours.as_ref()
        .lines()
        .chain(theirs.as_ref().lines())
        .sorted()
        .dedup()
        .join("\n")
}

pub fn fetch(work_dir: Option<&Path>) -> Result<()> {
    // Use git directly to avoid having to implement ssh-agent and/or extraHeader handling
    run_git(&["fetch", "origin", "refs/notes/perf"], &work_dir)
        .context("Failed to fetch performance measurements.")?;

    Ok(())
}

pub fn reconcile() -> Result<()> {
    let _ = run_git(
        &[
            "notes",
            "--ref",
            "refs/notes/perf",
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

pub fn raw_push(work_dir: Option<&Path>) -> Result<()> {
    // TODO(kaihowl) configure remote?
    // TODO(kaihowl) factor into constants
    // TODO(kaihowl) capture output
    run_git(
        &["push", "origin", "refs/notes/perf:refs/notes/perf"],
        &work_dir,
    )
    .context("Failed to push performance measurements.")?;

    Ok(())
}

// TODO(kaihowl) what happens with a git dir supplied with -C?
pub fn prune() -> Result<()> {
    if is_shallow_repo().context("Could not determine if shallow clone.")? {
        // TODO(kaihowl) is this not already checked by git itself?
        bail!("Refusing to prune on a shallow repo")
    }

    run_git(&["notes", "--ref", "refs/notes/perf", "prune"], &None).context("Failed to prune.")?;

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
            "--notes=refs/notes/perf",
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
    let mut retries = 3;

    // TODO(kaihowl) do actual, random backoff
    // TODO(kaihowl) check transient/permanent error
    while retries > 0 {
        if raw_push(work_dir).is_ok() {
            return Ok(());
        }

        retries -= 1;
        pull(work_dir)?;
    }

    bail!("Retries exceeded.")
}
#[cfg(test)]
mod test {
    use super::*;
    use std::{env::set_current_dir, fs::read_to_string};

    use git2::{Repository, Signature};
    use httptest::{
        http::{header::AUTHORIZATION, Uri},
        matchers::{self, request},
        responders::status_code,
        Expectation, Server,
    };
    use tempfile::{tempdir, TempDir};

    fn init_repo(dir: &Path) -> Repository {
        let repo = git2::Repository::init(dir).expect("Failed to create repo");
        {
            let tree_oid = repo
                .treebuilder(None)
                .expect("Failed to create tree")
                .write()
                .expect("Failed to write tree");
            let tree = &repo
                .find_tree(tree_oid)
                .expect("Could not find written tree");
            let signature = Signature::now("fake", "fake@example.com").expect("No signature");
            repo.commit(
                Some("refs/notes/perf"),
                &signature,
                &signature,
                "Initial commit",
                tree,
                &[],
            )
            .expect("Failed to create first commit");
        }

        repo
    }

    fn dir_with_repo_and_customheader(origin_url: Uri, extra_header: &str) -> TempDir {
        let tempdir = tempdir().unwrap();
        dbg!(&tempdir);
        dbg!(&extra_header);
        dbg!(&origin_url);

        let url = origin_url.to_string();

        let repo = init_repo(tempdir.path());

        repo.remote("origin", &url).expect("Failed to add remote");

        let mut config = repo.config().expect("Failed to get config");
        let config_key = format!("http.{}.extraHeader", url);
        config
            .set_str(&config_key, extra_header)
            .expect("Failed to set config value");

        let stuff = read_to_string(tempdir.path().join(".git/config")).expect("No config");
        eprintln!("config:\n{}", stuff);

        tempdir
    }

    #[test]
    fn test_customheader_push() {
        let test_server = Server::run();
        let repo_dir =
            dir_with_repo_and_customheader(test_server.url(""), "AUTHORIZATION: sometoken");

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

        push(Some(repo_dir.path()))
            .expect_err("We have no valid git http sever setup -> should fail");
    }

    #[test]
    fn test_get_head_revision() {
        let revision = get_head_revision().unwrap();
        assert!(
            &revision.chars().all(|c| c.is_ascii_alphanumeric()),
            "'{}' contained non alphanumeric or non ASCII characters",
            &revision
        )
    }

    #[test]
    fn test_resolve_conflicts() {
        let a = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234567.0 42.0\n";
        let b = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234890.0 22.0\n";

        let resolved = resolve_conflicts(a, b);
        assert!(resolved.contains("mymeasurement 1234567.0 23.0 key=value"));
        assert!(resolved.contains("myothermeasurement 1234567.0 42.0"));
        assert!(resolved.contains("myothermeasurement 1234890.0 22.0"));
        assert_eq!(3, resolved.lines().count());
    }

    #[test]
    fn test_resolve_conflicts_no_trailing_newline() {
        let a = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234567.0 42.0";
        let b = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234890.0 22.0";

        let resolved = resolve_conflicts(a, b);
        assert!(resolved.contains("mymeasurement 1234567.0 23.0 key=value"));
        assert!(resolved.contains("myothermeasurement 1234567.0 42.0"));
        assert!(resolved.contains("myothermeasurement 1234890.0 22.0"));
        assert_eq!(3, resolved.lines().count());
    }
}
