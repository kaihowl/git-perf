use std::{
    env::current_dir,
    io::{self, BufRead},
    path::Path,
    process::{self, Command},
};

use anyhow::{bail, Result};
use git2::{Index, Repository};
use itertools::Itertools;
use thiserror::Error;

// TODO(kaihowl) this copies the entire content everytime.
// replace by git invocation...
pub fn add_note_line_to_head(line: &str) -> Result<()> {
    let repo = Repository::open(".")?;
    let author = repo.signature()?;
    let head = repo.head()?;
    let head = head.peel_to_commit()?;

    let body;

    if let Ok(existing_note) = repo.find_note(Some("refs/notes/perf"), head.id()) {
        // TODO(kaihowl) check empty / not-utf8
        let existing_measurements = existing_note.message().expect("Message is not utf-8");
        // TODO(kaihowl) what about missing trailing new lines?
        // TODO(kaihowl) is there a more memory efficient way?
        body = format!("{}{}", existing_measurements, line);
    } else {
        body = line.to_string();
    }

    repo.note(
        &author,
        &author,
        Some("refs/notes/perf"),
        head.id(),
        &body,
        true,
    )
    .expect("TODO(kaihowl) note failed");

    Ok(())
}

pub fn add_note_line_to_head2(line: &str) -> Result<()> {
    let status = process::Command::new("git")
        .args([
            "notes",
            "--ref",
            "refs/notes/perf",
            "append",
            "--no-separator",
            "-m",
            line,
        ])
        .status()?;

    if !status.success() {
        bail!("Failed to add new measurement");
    }
    Ok(())
}

pub fn get_head_revision() -> String {
    let proc = process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("failed to parse head");

    // TODO(kaihowl) check status
    String::from_utf8(proc.stdout)
        .expect("oh no")
        .trim()
        .to_string()
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
    let work_dir = match work_dir {
        Some(dir) => dir.to_path_buf(),
        None => current_dir().expect("Could not determine current working directory"),
    };

    // Use git directly to avoid having to implement ssh-agent and/or extraHeader handling
    let status = process::Command::new("git")
        .args(["fetch", "origin", "refs/notes/perf"])
        .current_dir(work_dir)
        .status()?;

    if !status.success() {
        return Err(PushPullError::RawGitError.into());
    }

    Ok(())
}

pub fn reconcile() -> Result<()> {
    let repo = Repository::open(".")?;
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_head = fetch_head.peel_to_commit()?;

    let notes = match repo.find_reference("refs/notes/perf") {
        Ok(reference) => reference,
        Err(_) => {
            repo.reference(
                // TODO(kaihowl) pull into constant / configuration
                "refs/notes/perf",
                fetch_head.id(),
                false, /* this should never fail */
                "init perf notes",
            )?;
            return Ok(());
        }
    };

    let notes = notes.peel_to_commit()?;
    let index = repo.merge_commits(&notes, &fetch_head, None)?;

    let mut out_index = Index::new()?;
    let mut conflict_entries = Vec::new();

    if let Ok(conflicts) = index.conflicts() {
        conflict_entries = conflicts.try_collect()?;
    }

    for entry in index.iter() {
        if conflict_entries.iter().any(|c| {
            // TODO(kaihowl) think harder about this
            let conflict_entry = if let Some(our) = &c.our {
                our
            } else {
                c.their.as_ref().expect("Both our and their unset")
            };

            conflict_entry.path == entry.path
        }) {
            continue;
        }
        out_index.add(&entry).expect("failing entry in new index");
    }
    for conflict in conflict_entries {
        // TODO(kaihowl) no support for deleted / pruned measurements
        let our = conflict.our.unwrap();
        let our_oid = our.id;
        let our_content = String::from_utf8(repo.find_blob(our_oid)?.content().to_vec())
            .expect("UTF-8 error for our content");
        let their_oid = conflict.their.unwrap().id;
        let their_content = String::from_utf8(repo.find_blob(their_oid)?.content().to_vec())
            .expect("UTF-8 error for their content");
        let resolved_content = resolve_conflicts(&our_content, &their_content);
        // TODO(kaihowl) what should this be set to instead of copied from?
        let blob = repo.blob(resolved_content.as_bytes())?;
        let mut entry = our;
        // Missing bindings for resolving conflict in libgit2-rs. Therefore, manually overwrite.
        entry.flags = 0;
        entry.flags_extended = 0;
        entry.id = blob;

        out_index.add(&entry).expect("Could not add");
    }
    let out_index_paths = out_index
        .iter()
        .map(|i| String::from_utf8(i.path).unwrap())
        .collect_vec();

    dbg!(&out_index_paths);
    dbg!(out_index.has_conflicts());
    dbg!(out_index.len());
    let merged_tree = repo.find_tree(out_index.write_tree_to(&repo)?)?;

    // TODO(kaihowl) make this conditional on the conflicts.
    let signature = repo.signature()?;
    repo.commit(
        Some("refs/notes/perf"),
        &signature,
        &signature,
        "Merge it",
        &merged_tree,
        &[&notes, &fetch_head],
    )?;
    // repo.merge
    Ok(())
}

pub fn raw_push(work_dir: Option<&Path>) -> Result<()> {
    let work_dir = match work_dir {
        Some(dir) => dir.to_path_buf(),
        None => current_dir().expect("Could not determine current working directory"),
    };
    // TODO(kaihowl) configure remote?
    // TODO(kaihowl) factor into constants
    // TODO(kaihowl) capture output
    let status = Command::new("git")
        .args(["push", "origin", "refs/notes/perf:refs/notes/perf"])
        .current_dir(work_dir)
        .status()?;

    match status.code() {
        Some(0) => Ok(()),
        _ => Err(PushPullError::RawGitError.into()),
    }
}

#[derive(Debug, Error)]
pub enum PruneError {
    #[error("shallow repo")]
    ShallowRepo,

    #[error("git execution error")]
    RawGitError(#[from] io::Error),

    #[error("git error")]
    GitError,
}

// TODO(kaihowl) what happens with a git dir supplied with -C?
pub fn prune() -> Result<()> {
    match is_shallow_repo() {
        Some(true) => return Err(PruneError::ShallowRepo.into()),
        None => return Err(PruneError::GitError.into()),
        _ => {}
    }

    let status = process::Command::new("git")
        .args(["notes", "--ref", "refs/notes/perf", "prune"])
        .status()?;

    if !status.success() {
        return Err(PruneError::GitError.into());
    }

    Ok(())
}

fn is_shallow_repo() -> Option<bool> {
    match process::Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .output()
    {
        Ok(out) if out.status.success() => match std::str::from_utf8(&out.stdout) {
            Ok(out) => Some(out.starts_with("true")),
            Err(_) => None,
        },
        _ => None,
    }
}

// TODO(kaihowl) return a nested iterator / generator instead?
pub fn walk_commits(num_commits: usize) -> Result<Vec<(String, Vec<String>)>> {
    let output = process::Command::new("git")
        .args([
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
        ])
        .output()?;

    if !output.status.success() {
        eprintln!(
            "git failed: {}",
            String::from_utf8(output.stderr).expect("Oh god")
        );
        bail!("TODO(kaihowl) git error");
    }

    let mut current_commit = None;

    let lines: Vec<String> = output
        .stdout
        .lines()
        .map(|f| f.expect("stuff").to_string())
        .collect();

    // TODO(kaihowl) iterator or generator instead / how to propagate exit code?
    let it = lines.into_iter().filter_map(|l| {
        if l.starts_with("--") {
            current_commit = Some(
                l.split(",")
                    .skip(1)
                    .next()
                    .expect("TODO(kaihowl)")
                    .to_owned(),
            );
            None
        } else {
            // TODO(kaihowl) lot's of string copies...
            Some((
                current_commit.as_ref().expect("TODO(kaihowl)").to_owned(),
                l,
            ))
        }
    });
    let it: Vec<_> = it
        .group_by(|it| it.0.to_owned())
        .into_iter()
        .map(|(k, v)| {
            (
                k.to_owned(),
                // TODO(kaihowl) joining what was split above already
                v.map(|(_, v)| v).collect::<Vec<_>>(),
            )
        })
        .collect();
    Ok(it)
}

#[derive(Debug, Error)]
pub enum PushPullError {
    #[error("libgit2 error")]
    Git(#[from] git2::Error),

    #[error("git error")]
    RawGitError,

    #[error("git execution error")]
    GitExecError(#[from] io::Error),

    #[error("retries exceeded")]
    RetriesExceeded,
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
        match raw_push(work_dir) {
            Ok(_) => return Ok(()),
            Err(_) => {
                retries -= 1;
                pull(work_dir)?;
            }
        }
    }

    Err(PushPullError::RetriesExceeded.into())
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
        let revision = get_head_revision();
        assert!(
            revision.chars().all(|c| c.is_ascii_alphanumeric()),
            "'{}' contained non alphanumeric or non ASCII characters",
            revision
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
