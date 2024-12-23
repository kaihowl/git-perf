use std::{
    env::current_dir,
    io::{self, stdout, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{self, Child, Stdio},
    thread,
    time::Duration,
};

use unindent::unindent;

use anyhow::{anyhow, bail, Context, Result};
use backoff;
use backoff::ExponentialBackoffBuilder;
use itertools::Itertools;

use chrono::prelude::*;
use rand::{thread_rng, Rng};

#[derive(Debug, thiserror::Error)]
enum GitError {
    #[error("Git failed to execute.\n\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    ExecError {
        command: String,
        stdout: String,
        stderr: String,
    },

    #[error("Failed to execute git command")]
    IoError(#[from] io::Error),
}

fn spawn_git_command(
    args: &[&str],
    working_dir: &Option<&Path>,
    stdin: Option<Stdio>,
) -> Result<Child, io::Error> {
    let working_dir = working_dir.map(PathBuf::from).unwrap_or(current_dir()?);
    let stdin = stdin.unwrap_or(Stdio::null());
    process::Command::new("git")
        // TODO(kaihowl) set correct encoding and lang?
        .env("LANG", "")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .env("LC_ALL", "C")
        .current_dir(working_dir)
        .args(args)
        .spawn()
}

fn capture_git_output(args: &[&str], working_dir: &Option<&Path>) -> Result<String, GitError> {
    feed_git_command(args, working_dir, None)
}

fn feed_git_command(
    args: &[&str],
    working_dir: &Option<&Path>,
    input: Option<&str>,
) -> Result<std::string::String, GitError> {
    let stdin = input.and_then(|_s| Some(Stdio::piped()));

    let child = spawn_git_command(args, working_dir, stdin)?;

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

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(GitError::ExecError {
            command: args.join(" "),
            stdout,
            stderr,
        });
    }

    Ok(stdout)
}

const REFS_NOTES_BRANCH: &str = "refs/notes/perf-v3";
const REFS_NOTES_WRITE_SYMBOLIC_REF: &str = "refs/notes/perf-v3-write";
const REFS_NOTES_WRITE_TARGET_PREFIX: &str = "refs/notes/perf-v3-write-";
const REFS_NOTES_MERGE_BRANCH: &str = "refs/notes/perf-v3-merge";
const REFS_NOTES_READ_BRANCH: &str = "refs/notes/perf-v3-read";

pub fn add_note_line_to_head(line: &str) -> Result<()> {
    ensure_symbolic_write_ref_exists()?;
    capture_git_output(
        &[
            "notes",
            "--ref",
            REFS_NOTES_WRITE_SYMBOLIC_REF,
            "append",
            // TODO(kaihowl) disabled until #96 is solved
            // "--no-separator",
            "-m",
            line,
        ],
        &None,
    )
    .context("Failed to add new measurement")?;

    Ok(())
}

fn ensure_symbolic_write_ref_exists() -> Result<()> {
    if git_rev_parse(REFS_NOTES_WRITE_SYMBOLIC_REF).is_none() {
        let suffix = random_suffix();
        let target = format!("{REFS_NOTES_WRITE_TARGET_PREFIX}{suffix}");

        git_update_ref(unindent(
            format!(
                // Commit only if not yet created
                r#"
                start
                symref-create {REFS_NOTES_WRITE_SYMBOLIC_REF} {target}
                commit
                "#
            )
            .as_str(),
        ))?
    }
    Ok(())
}

fn random_suffix() -> String {
    let suffix: u32 = thread_rng().gen();
    format!("{:x}", suffix)
}

fn git_update_ref(commands: impl AsRef<str>) -> Result<(), GitError> {
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
    .map(|_s| ())
}

pub fn get_head_revision() -> Result<String> {
    git_rev_parse("HEAD").ok_or(anyhow!("missing head"))
}

pub fn fetch(work_dir: Option<&Path>) -> Result<()> {
    // Use git directly to avoid having to implement ssh-agent and/or extraHeader handling
    capture_git_output(
        &[
            "fetch",
            "--no-write-fetch-head",
            "origin",
            // Always force overwrite the local reference
            // Separation into write, merge, and read branches ensures that this does not lead to
            // any data loss
            format!("+{REFS_NOTES_BRANCH}:{REFS_NOTES_BRANCH}").as_str(),
        ],
        &work_dir,
    )
    .context("Failed to fetch performance measurements.")?;

    Ok(())
}

fn reconcile_branch_with(target: &str, branch: &str) -> Result<()> {
    _ = capture_git_output(
        &[
            "notes",
            "--ref",
            target,
            "merge",
            "-s",
            "cat_sort_uniq",
            branch,
        ],
        &None,
    )?;
    Ok(())
}

pub fn remove_measurements_from_commits(older_than: DateTime<Utc>) -> Result<()> {
    let oldest_timestamp = older_than.timestamp();
    // Outputs line-by-line <note_oid> <annotated_oid>
    let mut list_notes =
        spawn_git_command(&["notes", "--ref", REFS_NOTES_BRANCH, "list"], &None, None)?;
    let notes_out = list_notes.stdout.take().unwrap();

    let mut get_commit_dates = spawn_git_command(
        &[
            "log",
            "--ignore-missing",
            "--no-walk",
            "--pretty=format:%H %ct",
            "--stdin",
        ],
        &None,
        Some(Stdio::piped()),
    )?;
    let dates_in = get_commit_dates.stdin.take().unwrap();
    let dates_out = get_commit_dates.stdout.take().unwrap();

    let mut remove_measurements = spawn_git_command(
        &[
            "notes",
            "--ref",
            REFS_NOTES_BRANCH,
            "remove",
            "--stdin",
            "--ignore-missing",
        ],
        &None,
        Some(Stdio::piped()),
    )?;
    let removal_in = remove_measurements.stdin.take().unwrap();
    let removal_out = remove_measurements.stdout.take().unwrap();

    let removal_handler = thread::spawn(move || {
        let reader = BufReader::new(dates_out);
        let mut writer = BufWriter::new(removal_in);
        for line in reader.lines().map_while(Result::ok) {
            if let Some((commit, timestamp)) = line.split_whitespace().take(2).collect_tuple() {
                if let Ok(timestamp) = timestamp.parse::<i64>() {
                    if timestamp < oldest_timestamp {
                        writeln!(writer, "{}", commit).expect("Could not write to stream");
                    }
                }
            }
        }
    });

    let debugging_handler = thread::spawn(move || {
        let reader = BufReader::new(removal_out);
        reader
            .lines()
            .map_while(Result::ok)
            .for_each(|l| println!("{}", l))
    });

    {
        let reader = BufReader::new(notes_out);
        let mut writer = BufWriter::new(dates_in);

        reader.lines().map_while(Result::ok).for_each(|line| {
            if let Some(line) = line.split_whitespace().nth(1) {
                writeln!(writer, "{}", line).expect("Failed to write to pipe");
            }
        });

        // TODO(kaihowl) necessary?
        drop(writer);
    }

    removal_handler.join().expect("Failed to join");
    debugging_handler.join().expect("Failed to join");

    list_notes.wait()?;
    get_commit_dates.wait()?;
    remove_measurements.wait()?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum PushError {
    #[error("A ref failed to be pushed:\n{stdout}\n{stderr}")]
    RefFailedToPush { stdout: String, stderr: String },
}

fn new_symbolic_write_ref() -> Result<String> {
    let suffix = random_suffix();
    let target = format!("{REFS_NOTES_WRITE_TARGET_PREFIX}{suffix}");

    // TODO(kaihowl) can this actually return a failure upon abort?
    // TODO(kaihowl) does this actually run atomically as it claims?
    // See https://github.com/libgit2/libgit2/issues/5918 for a counter example
    // Also source code for the refs/files-backend.c does not look up to the task?
    // Do we need packed references after all? Or the new reftable format?
    git_update_ref(unindent(
        format!(
            r#"
            start
            symref-update {REFS_NOTES_WRITE_SYMBOLIC_REF} {target}
            commit
            "#
        )
        .as_str(),
    ))?;
    Ok(target)
}

const EMPTY_OID: &str = "0000000000000000000000000000000000000000";

fn git_rev_parse(reference: &str) -> Option<String> {
    capture_git_output(&["rev-parse", "--verify", "-q", reference], &None)
        .ok()
        .map(|s| s.trim().to_owned())
}

//TODO(kaihowl) clean up pub methods
pub fn raw_push(work_dir: Option<&Path>) -> Result<()> {
    // - Reset the symbolic-ref “write” to a new unique write ref.
    //     - Allows to continue committing measurements while pushing.
    //     - ?? What happens when a git notes amend concurrently still writes to the old ref?
    let new_write_ref = new_symbolic_write_ref()?;

    // OLD
    // - Create a temporary merge ref, set to the upstream perf ref, merge in all existing write refs except the newly created one from the previous step.
    //     - Same step (except for filtering of the new ref) happens on local read as well.)
    //     - Relies on unrelated histories, cat_uniq_sort merge strategy
    //     - Allows to cut off the history on upstream periodically
    // NEW
    // - Note down the current upstream perf ref oid
    let current_upstream_oid =
        git_rev_parse(REFS_NOTES_BRANCH).unwrap_or_else(|| EMPTY_OID.to_string());

    // - Reset the merge ref to the upstream perf ref iff it still matches the captured OID
    //   - otherwise concurrent pull occurred.
    git_update_ref(unindent(
        format!(
            r#"
                start
                verify {REFS_NOTES_BRANCH} {current_upstream_oid}
                update {REFS_NOTES_MERGE_BRANCH} {current_upstream_oid}
                commit
            "#
        )
        .as_str(),
    ))?;

    // - merge in all existing write refs, except for the newly created one from first step
    //     - Same step (except for filtering of the new ref) happens on local read as well.)
    //     - Relies on unrelated histories, cat_uniq_sort merge strategy
    //     - Allows to cut off the history on upstream periodically
    let additional_args = vec![format!("{REFS_NOTES_WRITE_TARGET_PREFIX}*")];
    let refs = get_refs(additional_args)?
        .into_iter()
        .filter(|r| r.refname != new_write_ref)
        .collect_vec();

    for reference in &refs {
        reconcile_branch_with(REFS_NOTES_MERGE_BRANCH, &reference.oid)?;
    }

    // TODO(kaihowl) configure remote?
    // TODO(kaihowl) factor into constants
    // TODO(kaihowl) capture output
    // - CAS push the temporary merge ref to upstream using the noted down upstream ref
    //     - In case of concurrent pushes, back off and restart fresh from previous step.
    let output = capture_git_output(
        &[
            "push",
            "--porcelain",
            format!("--force-with-lease={REFS_NOTES_BRANCH}:{current_upstream_oid}").as_str(),
            "origin",
            format!("{REFS_NOTES_MERGE_BRANCH}:{REFS_NOTES_BRANCH}").as_str(),
        ],
        &work_dir,
    );

    // - Clean your own temporary merge ref and all others with a merge commit older than x days.
    //     - In case of crashes before clean up, old merge refs are eliminated eventually.

    match output {
        Ok(_) => Ok(()),
        Err(GitError::ExecError {
            command: _,
            stdout,
            stderr,
        }) => {
            let successful_push = stdout.lines().any(|l| {
                l.contains(format!("{REFS_NOTES_BRANCH}:").as_str()) && !l.starts_with('!')
            });
            if successful_push {
                Ok(())
            } else {
                Err(anyhow!(PushError::RefFailedToPush { stdout, stderr }))
            }
        }
        Err(e) => Err(anyhow!(e)),
    }?;

    // TODO(kaihowl) can git push immediately update the local ref as well?
    // This might be necessary for a concurrent push in between the last push from here and the now
    // following fetch. Otherwise, the `verify` will fail in the update-ref call later.
    fetch(None)?;

    // Delete merged in write references
    let mut commands = Vec::new();
    commands.push(String::from("start"));
    for Reference { refname, oid } in &refs {
        commands.push(format!("delete {refname} {oid}"));
    }
    commands.push(String::from("commit"));
    // empty line
    commands.push(String::new());
    let commands = commands.join("\n");
    git_update_ref(commands)?;

    Ok(())

    // TODO(kaihowl) - Clean up all local write refs that have been merged into the upstream branch.
}

// TODO(kaihowl) what happens with a git dir supplied with -C?
pub fn prune() -> Result<()> {
    if is_shallow_repo().context("Could not determine if shallow clone.")? {
        // TODO(kaihowl) is this not already checked by git itself?
        bail!("Refusing to prune on a shallow repo")
    }

    // TODO(kaihowl) make this push to the remote with force-with-lease
    capture_git_output(&["notes", "--ref", REFS_NOTES_BRANCH, "prune"], &None)
        .context("Failed to prune.")?;

    Ok(())
}

fn is_shallow_repo() -> Result<bool> {
    let output = capture_git_output(&["rev-parse", "--is-shallow-repository"], &None)
        .context("Failed to determine if repo is a shallow clone.")?;

    Ok(output.starts_with("true"))
}

#[derive(Debug, PartialEq)]
struct Reference {
    refname: String,
    oid: String,
}

fn get_refs(additional_args: Vec<String>) -> Result<Vec<Reference>> {
    let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
    args.extend(additional_args.iter().map(|s| s.as_str()));

    let output = capture_git_output(&args, &None)?;
    Ok(output
        .lines()
        .map(|s| {
            let items = s.split('\0').take(2).collect_vec();
            assert!(items.len() == 2);
            Reference {
                refname: items[0].to_string(),
                oid: items[1].to_string(),
            }
        })
        .collect_vec())
}

fn get_non_merged_refs() -> Result<Vec<Reference>> {
    let additional_args = vec![
        format!("--no-merged={REFS_NOTES_READ_BRANCH}"),
        format!("{REFS_NOTES_BRANCH}"),
        format!("{REFS_NOTES_WRITE_TARGET_PREFIX}*"),
    ];
    get_refs(additional_args)
}

fn ensure_branch_exists(branch: &str) -> Result<()> {
    if git_rev_parse(branch).is_some() {
        return Ok(());
    }

    let empty_tree_oid = capture_git_output(&["mktree"], &None)?;
    let empty_tree_oid = empty_tree_oid.trim();

    let empty_commit = capture_git_output(
        &["commit-tree", "-m", "empty commit", empty_tree_oid],
        &None,
    )?;
    let empty_commit = empty_commit.trim();

    git_update_ref(unindent(
        format!(
            r#"
            start
            create {branch} {empty_commit}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(())
}

fn update_read_branch() -> Result<()> {
    ensure_branch_exists(REFS_NOTES_READ_BRANCH)?;

    let refs = get_non_merged_refs()?;
    let read_ref = refs.iter().find(|f| f.refname == REFS_NOTES_BRANCH);

    // - With the upstream refs/notes/perf-v3
    //     - If not merged into refs/notes/perf-v3-read: set refs/notes/perf-v3-read to refs/notes/perf-v3
    //     - Protect against concurrent invocations by checking that the refs/notes/perf-v3-read has
    //     not changed between invocations!

    if let Some(Reference { refname, oid }) = read_ref {
        // Protect against concurrent pulls
        git_update_ref(unindent(
            format!(
                r#"
                    start
                    verify {refname} {oid}
                    update {REFS_NOTES_READ_BRANCH} {oid}
                    commit
                "#
            )
            .as_str(),
        ))?;
    }

    // - With each local refs/notes/perf-v3-write-XXX
    //     - If not merged into refs/notes/perf-v3-read: merge in with cat_uniq_sort
    for reference in &refs {
        // TODO(kaihowl) unnecessary optimization?
        if Some(reference) == read_ref {
            continue;
        }

        reconcile_branch_with(REFS_NOTES_READ_BRANCH, &reference.oid)?
    }

    Ok(())
}

// TODO(kaihowl) return a nested iterator / generator instead?
pub fn walk_commits(num_commits: usize) -> Result<Vec<(String, Vec<String>)>> {
    // update local read branch
    update_read_branch()?;

    // TODO(kaihowl) update the local read branch
    let output = capture_git_output(
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
            format!("--notes={REFS_NOTES_READ_BRANCH}").as_str(),
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
    fetch(work_dir)
}

pub fn push(work_dir: Option<&Path>) -> Result<()> {
    // TODO(kaihowl) check transient/permanent error
    let op = || -> Result<(), backoff::Error<anyhow::Error>> {
        raw_push(work_dir).map_err(|e| match e.downcast_ref::<PushError>() {
            Some(PushError::RefFailedToPush { .. }) => match pull(work_dir) {
                Err(pull_error) => backoff::Error::permanent(pull_error),
                Ok(_) => backoff::Error::transient(e),
            },
            None => backoff::Error::Permanent(e),
        })
    };

    // TODO(kaihowl) configure
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    backoff::retry(backoff, op).map_err(|e| match e {
        backoff::Error::Permanent(e) => e.context("Permanent failure while pushing refs"),
        backoff::Error::Transient { err, .. } => err.context("Timed out pushing refs"),
    })?;

    Ok(())
}

fn parse_git_version(version: &str) -> Result<(i32, i32, i32)> {
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
    let version = capture_git_output(&["--version"], &None).context("Determine git version")?;
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
    let expected_version = (2, 41, 0);
    if version_tuple < expected_version {
        bail!(
            "Version {} is smaller than {}",
            concat_version(version_tuple),
            concat_version(expected_version)
        )
    }
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
    use serial_test::serial;
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

    fn switch_to_dir_with_repo_and_customheader(origin_url: Uri, extra_header: &str) {}

    fn add_server_remote(origin_url: Uri, extra_header: &str, dir: &Path) {
        let url = origin_url.to_string();

        run_git_command(&["remote", "add", "origin", &url], dir);
        run_git_command(
            &[
                "config",
                "--add",
                format!("http.{}.extraHeader", url).as_str(),
                extra_header,
            ],
            dir,
        );
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
    #[serial]
    fn test_customheader_pull() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).expect("Failed to change dir");

        let test_server = Server::run();
        add_server_remote(
            test_server.url(""),
            "AUTHORIZATION: sometoken",
            tempdir.path(),
        );

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
        pull(None).expect_err("We have no valid git http server setup -> should fail");
    }

    #[test]
    // TODO(kaihowl) properly pass current working directory into commands and remove serial
    // execution again
    #[serial]
    fn test_customheader_push() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).expect("Failed to change dir");

        let test_server = Server::run();
        add_server_remote(
            test_server.url(""),
            "AUTHORIZATION: someothertoken",
            tempdir.path(),
        );

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
        let error = push(None);
        error
            .as_ref()
            .expect_err("We have no valid git http server setup -> should fail");
        dbg!(&error);
    }

    #[test]
    #[serial]
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

    #[test]
    fn test_parse_git_version() {
        let version = parse_git_version("git version 2.52.0");
        assert_eq!(version.unwrap(), (2, 52, 0));

        let version = parse_git_version("git version 2.52.0\n");
        assert_eq!(version.unwrap(), (2, 52, 0));
    }

    #[test]
    fn test_random_suffix() {
        let first = random_suffix();
        let second = random_suffix();

        let all_hex = |s: &String| s.chars().all(|c| c.is_ascii_hexdigit());

        assert_ne!(first, second);
        assert_eq!(first.len(), 8);
        assert_eq!(second.len(), 8);
        assert!(all_hex(&first));
        assert!(all_hex(&second));
    }
}
