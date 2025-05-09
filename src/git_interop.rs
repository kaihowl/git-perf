use std::{
    env::current_dir,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{self, Child, Stdio},
    thread,
    time::Duration,
};

use defer::defer;
use log::{debug, trace};
use unindent::unindent;

use ::backoff::ExponentialBackoffBuilder;
use anyhow::{anyhow, bail, Context, Result};
use itertools::Itertools;

use chrono::prelude::*;
use rand::{thread_rng, Rng};

#[derive(Debug)]
struct GitOutput {
    stdout: String,
    stderr: String,
}

#[derive(Debug, thiserror::Error)]
enum GitError {
    #[error("A ref failed to be pushed:\n{0}\n{1}", output.stdout, output.stderr)]
    RefFailedToPush { output: GitOutput },

    #[error("Missing HEAD for {reference}")]
    MissingHead { reference: String },

    #[error("A ref failed to be locked:\n{0}\n{1}", output.stdout, output.stderr)]
    RefFailedToLock { output: GitOutput },

    #[error("Shallow repository. Refusing operation.")]
    ShallowRepository,

    #[error("This repo does not have any measurements.")]
    MissingMeasurements,

    #[error("A concurrent change to the ref occurred:\n{0}\n{1}", output.stdout, output.stderr)]
    RefConcurrentModification { output: GitOutput },

    #[error("Git failed to execute. {output:?}")]
    ExecError { command: String, output: GitOutput },

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
    debug!("execute: git {}", args.join(" "));
    process::Command::new("git")
        // TODO(kaihowl) set correct encoding and lang?
        .env("LANG", "")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LC_ALL", "C")
        .current_dir(working_dir)
        .args(args)
        .spawn()
}

fn capture_git_output(args: &[&str], working_dir: &Option<&Path>) -> Result<GitOutput, GitError> {
    feed_git_command(args, working_dir, None)
}

fn feed_git_command(
    args: &[&str],
    working_dir: &Option<&Path>,
    input: Option<&str>,
) -> Result<GitOutput, GitError> {
    let stdin = input.and_then(|_s| Some(Stdio::piped()));

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
    trace!("stdout: {}", stdout);

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    trace!("stderr: {}", stderr);

    let git_output = GitOutput { stdout, stderr };

    if output.status.success() {
        trace!("exec succeeded");
    } else {
        trace!("exec failed");
        return Err(GitError::ExecError {
            command: args.join(" "),
            output: git_output,
        });
    }

    Ok(git_output)
}

// TODO(kaihowl) missing docs
const REFS_NOTES_BRANCH: &str = "refs/notes/perf-v3";
const REFS_NOTES_WRITE_SYMBOLIC_REF: &str = "refs/notes/perf-v3-write";
const REFS_NOTES_WRITE_TARGET_PREFIX: &str = "refs/notes/perf-v3-write-";
const REFS_NOTES_ADD_TARGET_PREFIX: &str = "refs/notes/perf-v3-add-";
const REFS_NOTES_REWRITE_TARGET_PREFIX: &str = "refs/notes/perf-v3-rewrite-";
const REFS_NOTES_MERGE_BRANCH_PREFIX: &str = "refs/notes/perf-v3-merge-";
const REFS_NOTES_READ_BRANCH: &str = "refs/notes/perf-v3-read";

fn map_git_error_for_backoff(e: GitError) -> ::backoff::Error<GitError> {
    match e {
        GitError::RefFailedToPush { .. }
        | GitError::RefFailedToLock { .. }
        | GitError::RefConcurrentModification { .. } => ::backoff::Error::transient(e),
        GitError::ExecError { .. }
        | GitError::IoError(..)
        | GitError::ShallowRepository
        | GitError::MissingHead { .. }
        | GitError::MissingMeasurements => ::backoff::Error::permanent(e),
    }
}

pub fn add_note_line_to_head(line: &str) -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_add_note_line_to_head(line).map_err(map_git_error_for_backoff)
    };

    // TODO(kaihowl) configure
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    ::backoff::retry(backoff, op).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while adding note line to head")
        }
        ::backoff::Error::Transient { err, .. } => {
            anyhow!(err).context("Timed out while adding note line to head")
        }
    })?;

    Ok(())
}

fn raw_add_note_line_to_head(line: &str) -> Result<(), GitError> {
    ensure_symbolic_write_ref_exists()?;

    // `git notes append` is not safe to use concurrently.
    // We create a new type of temporary reference: Cannot reuse the normal write references as
    // they only get merged upon push. This can take arbitrarily long.
    let current_note_head =
        git_rev_parse(REFS_NOTES_WRITE_SYMBOLIC_REF).unwrap_or(EMPTY_OID.to_string());
    let current_symbolic_ref_target = git_rev_parse_symbolic_ref(REFS_NOTES_WRITE_SYMBOLIC_REF)
        .expect("Missing symbolic-ref for target");
    let temp_target = create_temp_add_head(&current_note_head)?;

    defer!(git_update_ref(unindent(
        format!(
            r#"
            start
            delete {temp_target}
            commit
            "#
        )
        .as_str(),
    ))
    .expect("Deleting our own temp ref for adding should never fail"));

    // Test if the repo has any commit checked out at HEAD
    if let Err(_) = internal_get_head_revision() {
        return Err(GitError::MissingHead {
            reference: "HEAD".to_string(),
        });
    }

    capture_git_output(
        &[
            "notes",
            "--ref",
            &temp_target,
            "append",
            // TODO(kaihowl) disabled until #96 is solved
            // "--no-separator",
            "-m",
            line,
        ],
        &None,
    )?;

    // Update current write branch with pending write
    // TODO(kaihowl) duplication
    git_update_ref(unindent(
        format!(
            r#"
            start
            symref-verify {REFS_NOTES_WRITE_SYMBOLIC_REF} {current_symbolic_ref_target}
            update {current_symbolic_ref_target} {temp_target} {current_note_head}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(())
}

fn ensure_symbolic_write_ref_exists() -> Result<(), GitError> {
    if let Err(GitError::MissingHead { .. }) = git_rev_parse(REFS_NOTES_WRITE_SYMBOLIC_REF) {
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
        ))
        .or_else(|err| {
            if let GitError::RefFailedToLock { .. } = err {
                return Ok(());
            } else {
                return Err(err);
            }
        })?;
    }
    Ok(())
}

fn random_suffix() -> String {
    let suffix: u32 = thread_rng().gen();
    format!("{:08x}", suffix)
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
    .map_err(map_git_error)
    .map(|_| ())
}

pub fn get_head_revision() -> Result<String> {
    Ok(internal_get_head_revision()?)
}

fn internal_get_head_revision() -> Result<String, GitError> {
    git_rev_parse("HEAD")
}

fn map_git_error(err: GitError) -> GitError {
    match err {
        GitError::ExecError { command: _, output } if output.stderr.contains("cannot lock ref") => {
            GitError::RefFailedToLock { output }
        }
        GitError::ExecError { command: _, output } if output.stderr.contains("but expected") => {
            GitError::RefConcurrentModification { output }
        }
        _ => err,
    }
}

fn fetch(work_dir: Option<&Path>) -> Result<(), GitError> {
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
    .map_err(map_git_error)?;

    Ok(())
}

fn reconcile_branch_with(target: &str, branch: &str) -> Result<(), GitError> {
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

// TODO(kaihowl) duplication
fn create_temp_rewrite_head(current_notes_head: &str) -> Result<String, GitError> {
    let suffix = random_suffix();
    let target = format!("{REFS_NOTES_REWRITE_TARGET_PREFIX}{suffix}");

    // Clone reference
    git_update_ref(unindent(
        format!(
            r#"
            start
            create {target} {current_notes_head}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(target)
}

fn create_temp_add_head(current_notes_head: &str) -> Result<String, GitError> {
    let suffix = random_suffix();
    let target = format!("{REFS_NOTES_ADD_TARGET_PREFIX}{suffix}");

    // TODO(kaihowl) humpty dumpty
    if current_notes_head != EMPTY_OID {
        // Clone reference
        git_update_ref(unindent(
            format!(
                r#"
            start
            create {target} {current_notes_head}
            commit
            "#
            )
            .as_str(),
        ))?;
    }

    Ok(target)
}

fn compact_head(target: &str) -> Result<(), GitError> {
    let new_removal_head = git_rev_parse(&format!("{target}^{{tree}}").as_str())?;

    // Orphan compaction commit
    let compaction_head = capture_git_output(
        &["commit-tree", "-m", "cutoff history", &new_removal_head],
        &None,
    )?
    .stdout;

    let compaction_head = compaction_head.trim();

    git_update_ref(unindent(
        format!(
            r#"
            start
            update {target} {compaction_head}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(())
}

pub fn remove_measurements_from_commits(older_than: DateTime<Utc>) -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_remove_measurements_from_commits(older_than).map_err(map_git_error_for_backoff)
    };

    // TODO(kaihowl) configure
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    ::backoff::retry(backoff, op).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while adding note line to head")
        }
        ::backoff::Error::Transient { err, .. } => {
            anyhow!(err).context("Timed out while adding note line to head")
        }
    })?;

    Ok(())
}

fn raw_remove_measurements_from_commits(older_than: DateTime<Utc>) -> Result<(), GitError> {
    // TODO(kaihowl) flow
    // 1. pull
    // 2. remove measurements
    // 3. compact
    // 4. try to push
    // TODO(kaihowl) repeat with back off
    // TODO(kaihowl) clean up branches

    // TODO(kaihowl) better error message for remote empty / never pushed
    fetch(None)?;

    let current_notes_head = git_rev_parse(REFS_NOTES_BRANCH)?;

    let target = create_temp_rewrite_head(&current_notes_head)?;

    remove_measurements_from_reference(&target, older_than)?;

    compact_head(&target)?;

    // TODO(kaihowl) actual push needed
    git_push_notes_ref(&current_notes_head, &target, &None)?;

    git_update_ref(unindent(
        format!(
            r#"
            start
            update {REFS_NOTES_BRANCH} {target}
            commit
            "#
        )
        .as_str(),
    ))?;

    // Delete target
    git_update_ref(unindent(
        format!(
            r#"
            start
            delete {target}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(())
}

// Remove notes pertaining to git commits whose commit date is older than specified.
fn remove_measurements_from_reference(
    reference: &str,
    older_than: DateTime<Utc>,
) -> Result<(), GitError> {
    let oldest_timestamp = older_than.timestamp();
    // Outputs line-by-line <note_oid> <annotated_oid>
    let mut list_notes = spawn_git_command(&["notes", "--ref", reference, "list"], &None, None)?;
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
            reference,
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
                    if timestamp <= oldest_timestamp {
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

fn new_symbolic_write_ref() -> Result<String, GitError> {
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

fn git_rev_parse(reference: &str) -> Result<String, GitError> {
    capture_git_output(&["rev-parse", "--verify", "-q", reference], &None)
        .map_err(|_e| GitError::MissingHead {
            reference: reference.into(),
        })
        .map(|s| s.stdout.trim().to_owned())
}

fn git_rev_parse_symbolic_ref(reference: &str) -> Option<String> {
    capture_git_output(&["symbolic-ref", "-q", reference], &None)
        .ok()
        .map(|s| s.stdout.trim().to_owned())
}

fn consolidate_write_branches_into(
    current_upstream_oid: &str,
    target: &str,
    except_ref: Option<&str>,
) -> Result<Vec<Reference>, GitError> {
    // - Reset the merge ref to the upstream perf ref iff it still matches the captured OID
    //   - otherwise concurrent pull occurred.
    git_update_ref(unindent(
        format!(
            r#"
                start
                verify {REFS_NOTES_BRANCH} {current_upstream_oid}
                update {target} {current_upstream_oid} {EMPTY_OID}
                commit
            "#
        )
        .as_str(),
    ))?;

    // - merge in all existing write refs, except for the newly created one from first step
    //     - Same step (except for filtering of the new ref) happens on local read as well.)
    //     - Relies on unrelated histories, cat_sort_uniq merge strategy
    //     - Allows to cut off the history on upstream periodically
    let additional_args = vec![format!("{REFS_NOTES_WRITE_TARGET_PREFIX}*")];
    let refs = get_refs(additional_args)?
        .into_iter()
        .filter(|r| r.refname != except_ref.unwrap_or_default())
        .collect_vec();

    // TODO(kaihowl) explicit test needed, currently only fails in concurrency test
    // when push is called before the first add.
    if refs.is_empty() {
        return Ok([].into());
    }

    for reference in &refs {
        reconcile_branch_with(&target, &reference.oid)?;
    }

    Ok(refs)
}

//TODO(kaihowl) clean up pub methods
fn raw_push(work_dir: Option<&Path>) -> Result<(), GitError> {
    // This might merge concurrently created write branches. There is no protection against that.
    // This wants to achieve an at-least-once semantic. The exactly-once semantic is ensured by the
    // cat_sort_uniq merge strategy.

    // - Reset the symbolic-ref “write” to a new unique write ref.
    //     - Allows to continue committing measurements while pushing.
    //     - ?? What happens when a git notes amend concurrently still writes to the old ref?
    let new_write_ref = new_symbolic_write_ref()?;

    // TODO(kaihowl) catch all dupes with this pattern
    let suffix = random_suffix();
    let merge_ref = format!("{REFS_NOTES_MERGE_BRANCH_PREFIX}{suffix}");

    defer!(git_update_ref(unindent(
        format!(
            r#"
                    start
                    delete {merge_ref}
                    commit
                "#
        )
        .as_str()
    ))
    .expect("Deleting our own branch should never fail"));

    // - Create a temporary merge ref, set to the upstream perf ref, merge in all existing write refs except the newly created one from the previous step.
    //     - Same step (except for filtering of the new ref) happens on local read as well.)
    //     - Relies on unrelated histories, cat_sort_uniq merge strategy
    //     - Allows to cut off the history on upstream periodically
    // NEW
    // - Note down the current upstream perf ref oid
    let current_upstream_oid = git_rev_parse(REFS_NOTES_BRANCH).unwrap_or(EMPTY_OID.to_string());
    let refs =
        consolidate_write_branches_into(&current_upstream_oid, &merge_ref, Some(&new_write_ref))?;

    if refs.is_empty() && current_upstream_oid == EMPTY_OID {
        return Err(GitError::MissingMeasurements);
    }

    git_push_notes_ref(&current_upstream_oid, &merge_ref, &work_dir)?;

    // TODO(kaihowl) can git push immediately update the local ref as well?
    // This might be necessary for a concurrent push in between the last push from here and the now
    // following fetch. Otherwise, the `verify` will fail in the update-ref call later.
    fetch(None)?;

    // Delete merged-in write references
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

fn git_push_notes_ref(
    expected_upstream: &str,
    push_ref: &str,
    working_dir: &Option<&Path>,
) -> Result<(), GitError> {
    // TODO(kaihowl) configure remote?
    // TODO(kaihowl) factor into constants
    // TODO(kaihowl) capture output
    // - CAS push the temporary merge ref to upstream using the noted down upstream ref
    //     - In case of concurrent pushes, back off and restart fresh from previous step.
    let output = capture_git_output(
        &[
            "push",
            "--porcelain",
            format!("--force-with-lease={REFS_NOTES_BRANCH}:{expected_upstream}").as_str(),
            "origin",
            format!("{push_ref}:{REFS_NOTES_BRANCH}").as_str(),
        ],
        &working_dir,
    );

    // - Clean your own temporary merge ref and all others with a merge commit older than x days.
    //     - In case of crashes before clean up, old merge refs are eliminated eventually.

    match output {
        Ok(_) => Ok(()),
        Err(GitError::ExecError { command: _, output }) => {
            let successful_push = output.stdout.lines().any(|l| {
                l.contains(format!("{REFS_NOTES_BRANCH}:").as_str()) && !l.starts_with('!')
            });
            if successful_push {
                Ok(())
            } else {
                Err(GitError::RefFailedToPush { output })
            }
        }
        Err(e) => Err(e),
    }?;

    Ok(())
}

// TODO(kaihowl) what happens with a git dir supplied with -C?
pub fn prune() -> Result<()> {
    // TODO(kaihowl) put the transient / permanent error in its own function, reuse
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_prune().map_err(map_git_error_for_backoff)
    };

    // TODO(kaihowl) configure
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    ::backoff::retry(backoff, op).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while pushing refs")
        }
        ::backoff::Error::Transient { err, .. } => anyhow!(err).context("Timed out pushing refs"),
    })?;

    Ok(())
}

fn raw_prune() -> Result<(), GitError> {
    // TODO(kaihowl) missing raw + retry
    if is_shallow_repo()? {
        // TODO(kaihowl) is this not already checked by git itself?
        return Err(GitError::ShallowRepository);
    }

    // TODO(kaihowl) code duplication with remove_measurements_from_commits

    // - update local upstream from remote
    pull_internal(None)?;

    // - create temp branch for pruning and set to current upstream
    let current_notes_head = git_rev_parse(REFS_NOTES_BRANCH)?;
    let target = create_temp_rewrite_head(&current_notes_head)?;

    // - invoke prune
    capture_git_output(&["notes", "--ref", &target, "prune"], &None)?;

    // - compact the new head
    compact_head(&target)?;

    // TODO(kaihowl) add additional test coverage checking that the head has been compacted
    // / elements are dropped

    // - CAS remote upstream
    git_push_notes_ref(&current_notes_head, &target, &None)?;
    git_update_ref(unindent(
        format!(
            r#"
            start
            update {REFS_NOTES_BRANCH} {target}
            commit
            "#
        )
        .as_str(),
    ))?;

    // - clean up temp branch
    // TODO(kaihowl) clean up old temp branches
    git_update_ref(unindent(
        format!(
            r#"
            start
            delete {target}
            commit
            "#
        )
        .as_str(),
    ))?;

    Ok(())
}

fn is_shallow_repo() -> Result<bool, GitError> {
    let output = capture_git_output(&["rev-parse", "--is-shallow-repository"], &None)?;

    Ok(output.stdout.starts_with("true"))
}

#[derive(Debug, PartialEq)]
struct Reference {
    refname: String,
    oid: String,
}

fn get_refs(additional_args: Vec<String>) -> Result<Vec<Reference>, GitError> {
    let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
    args.extend(additional_args.iter().map(|s| s.as_str()));

    let output = capture_git_output(&args, &None)?;
    Ok(output
        .stdout
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

fn update_read_branch() -> Result<()> {
    // TODO(kaihowl) use temp branches and return RAII object
    git_update_ref(unindent(
        format!(
            r#"
            start
            delete {REFS_NOTES_READ_BRANCH}
            commit
            "#
        )
        .as_str(),
    ))?;

    // - With the upstream refs/notes/perf-v3
    //     - If not merged into refs/notes/perf-v3-read: set refs/notes/perf-v3-read to refs/notes/perf-v3
    //     - Protect against concurrent invocations by checking that the refs/notes/perf-v3-read has
    //     not changed between invocations!
    //
    // TODO(kaihowl) add test for bug:
    //   read branch might not be up to date with the remote branch after a history cut off.
    //   Then the _old_ read branch might have all writes already merged in.
    //   But the upstream does not. But we check the pending write branches against the old read
    //   branch......
    //   Better to just create the read branch fresh from the remote and add in all pending write
    //   branches and not optimize. This should be the same as creating the merge branch. Can the
    //   code be ..merged..?

    let current_upstream_oid = git_rev_parse(REFS_NOTES_BRANCH).unwrap_or(EMPTY_OID.to_string());
    // TODO(kaihowl) protect against concurrent writes with temp read branch?
    let _ = consolidate_write_branches_into(&current_upstream_oid, REFS_NOTES_READ_BRANCH, None)?;

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
    let it = output.stdout.lines().filter_map(|l| {
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
    pull_internal(work_dir)?;
    Ok(())
}

fn pull_internal(work_dir: Option<&Path>) -> Result<(), GitError> {
    fetch(work_dir).or_else(|err| match err {
        // A concurrent modification comes from a concurrent fetch.
        // Don't fail for that.
        // TODO(kaihowl) must potentially be moved into the retry logic from the push backoff as it
        // only is there safe to assume that we successfully pulled.
        GitError::RefConcurrentModification { .. } | GitError::RefFailedToLock { .. } => Ok(()),
        _ => Err(err),
    })?;

    Ok(())
}

pub fn push(work_dir: Option<&Path>) -> Result<()> {
    // TODO(kaihowl) check transient/permanent error
    let op = || {
        raw_push(work_dir)
            .map_err(map_git_error_for_backoff)
            .map_err(|e: ::backoff::Error<GitError>| match e {
                ::backoff::Error::Transient { .. } => {
                    match pull_internal(work_dir).map_err(map_git_error_for_backoff) {
                        Ok(_) => e,
                        Err(e) => e,
                    }
                }
                ::backoff::Error::Permanent { .. } => e,
            })
    };

    // TODO(kaihowl) configure
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        .build();

    ::backoff::retry(backoff, op).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while pushing refs")
        }
        ::backoff::Error::Transient { err, .. } => anyhow!(err).context("Timed out pushing refs"),
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

        // Must add a single write as a push without pending local writes just succeeds
        add_note_line_to_head("test note line").expect("Failed to add note line");

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
    fn test_random_suffix() {
        for _ in 1..1000 {
            let first = random_suffix();
            dbg!(&first);
            let second = random_suffix();
            dbg!(&second);

            let all_hex = |s: &String| s.chars().all(|c| c.is_ascii_hexdigit());

            assert_ne!(first, second);
            assert_eq!(first.len(), 8);
            assert_eq!(second.len(), 8);
            assert!(all_hex(&first));
            assert!(all_hex(&second));
        }
    }
}
