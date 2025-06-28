use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::Stdio,
    thread,
    time::Duration,
};

use defer::defer;
use log::{debug, warn};
use unindent::unindent;

use anyhow::{anyhow, bail, Context, Result};
use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};
use itertools::Itertools;

use chrono::prelude::*;
use rand::{thread_rng, Rng};

use crate::config;

use super::git_definitions::{
    GIT_ORIGIN, GIT_PERF_REMOTE, REFS_NOTES_ADD_TARGET_PREFIX, REFS_NOTES_BRANCH,
    REFS_NOTES_MERGE_BRANCH_PREFIX, REFS_NOTES_READ_BRANCH, REFS_NOTES_REWRITE_TARGET_PREFIX,
    REFS_NOTES_WRITE_SYMBOLIC_REF, REFS_NOTES_WRITE_TARGET_PREFIX,
};
use super::git_lowlevel::{
    capture_git_output, get_git_perf_remote, git_rev_parse, git_rev_parse_symbolic_ref,
    git_update_ref, internal_get_head_revision, is_shallow_repo, map_git_error,
    set_git_perf_remote, spawn_git_command,
};
use super::git_types::GitError;
use super::git_types::Reference;

pub use super::git_lowlevel::get_head_revision;

pub use super::git_lowlevel::check_git_version;

// TODO(kaihowl) separate into git low and high level logic

fn map_git_error_for_backoff(e: GitError) -> ::backoff::Error<GitError> {
    match e {
        GitError::RefFailedToPush { .. }
        | GitError::RefFailedToLock { .. }
        | GitError::RefConcurrentModification { .. } => ::backoff::Error::transient(e),
        GitError::ExecError { .. }
        | GitError::IoError(..)
        | GitError::ShallowRepository
        | GitError::MissingHead { .. }
        | GitError::NoRemoteMeasurements { .. }
        | GitError::NoUpstream { .. }
        | GitError::EmptyOrNeverPushedRemote { .. }
        | GitError::MissingMeasurements => ::backoff::Error::permanent(e),
    }
}

/// Central place to configure backoff policy for git-perf operations.
fn default_backoff() -> ExponentialBackoff {
    let max_elapsed = config::backoff_max_elapsed_seconds();
    ExponentialBackoffBuilder::default()
        .with_max_elapsed_time(Some(Duration::from_secs(max_elapsed)))
        .build()
}

pub fn add_note_line_to_head(line: &str) -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_add_note_line_to_head(line).map_err(map_git_error_for_backoff)
    };

    let backoff = default_backoff();

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
    if internal_get_head_revision().is_err() {
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

fn ensure_remote_exists() -> Result<(), GitError> {
    if get_git_perf_remote(GIT_PERF_REMOTE).is_some() {
        return Ok(());
    }

    if let Some(x) = get_git_perf_remote(GIT_ORIGIN) {
        return set_git_perf_remote(GIT_PERF_REMOTE, &x);
    }

    Err(GitError::NoUpstream {})
}

/// Creates a temporary reference name by combining a prefix with a random suffix.
fn create_temp_ref_name(prefix: &str) -> String {
    let suffix = random_suffix();
    format!("{prefix}{suffix}")
}

fn ensure_symbolic_write_ref_exists() -> Result<(), GitError> {
    if git_rev_parse(REFS_NOTES_WRITE_SYMBOLIC_REF).is_err() {
        let target = create_temp_ref_name(REFS_NOTES_WRITE_TARGET_PREFIX);

        git_update_ref(unindent(
            format!(
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
                Ok(())
            } else {
                Err(err)
            }
        })?;
    }
    Ok(())
}

fn random_suffix() -> String {
    let suffix: u32 = thread_rng().gen();
    format!("{suffix:08x}")
}

fn fetch(work_dir: Option<&Path>) -> Result<(), GitError> {
    ensure_remote_exists()?;

    let ref_before = git_rev_parse(REFS_NOTES_BRANCH).ok();
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
    .map(|output| print!("{}", output.stderr))
    .map_err(map_git_error)?;

    let ref_after = git_rev_parse(REFS_NOTES_BRANCH).ok();

    if ref_before == ref_after {
        println!("Already up to date");
    }

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

fn create_temp_ref(prefix: &str, current_head: &str) -> Result<String, GitError> {
    let target = create_temp_ref_name(prefix);
    if current_head != EMPTY_OID {
        git_update_ref(unindent(
            format!(
                r#"
            start
            create {target} {current_head}
            commit
            "#
            )
            .as_str(),
        ))?;
    }
    Ok(target)
}

fn create_temp_rewrite_head(current_notes_head: &str) -> Result<String, GitError> {
    create_temp_ref(REFS_NOTES_REWRITE_TARGET_PREFIX, current_notes_head)
}

fn create_temp_add_head(current_notes_head: &str) -> Result<String, GitError> {
    create_temp_ref(REFS_NOTES_ADD_TARGET_PREFIX, current_notes_head)
}

fn compact_head(target: &str) -> Result<(), GitError> {
    let new_removal_head = git_rev_parse(format!("{target}^{{tree}}").as_str())?;

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

fn retry_notify(err: GitError, dur: Duration) {
    debug!("Error happened at {dur:?}: {err}");
    warn!("Retrying...");
}

pub fn remove_measurements_from_commits(older_than: DateTime<Utc>) -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_remove_measurements_from_commits(older_than).map_err(map_git_error_for_backoff)
    };

    let backoff = default_backoff();

    ::backoff::retry_notify(backoff, op, retry_notify).map_err(|e| match e {
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
    // 1. pull
    // 2. remove measurements
    // 3. compact
    // 4. try to push
    fetch(None)?;

    let current_notes_head = match git_rev_parse(REFS_NOTES_BRANCH) {
        Ok(head) => head,
        Err(GitError::MissingHead { .. }) => {
            return Err(GitError::EmptyOrNeverPushedRemote {});
        }
        Err(e) => return Err(e),
    };

    let target = create_temp_rewrite_head(&current_notes_head)?;

    remove_measurements_from_reference(&target, older_than)?;

    compact_head(&target)?;

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
                        writeln!(writer, "{commit}").expect("Could not write to stream");
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
            .for_each(|l| println!("{l}"))
    });

    {
        let reader = BufReader::new(notes_out);
        let mut writer = BufWriter::new(dates_in);

        reader.lines().map_while(Result::ok).for_each(|line| {
            if let Some(line) = line.split_whitespace().nth(1) {
                writeln!(writer, "{line}").expect("Failed to write to pipe");
            }
        });
    }

    removal_handler.join().expect("Failed to join");
    debugging_handler.join().expect("Failed to join");

    list_notes.wait()?;
    get_commit_dates.wait()?;
    remove_measurements.wait()?;

    Ok(())
}

fn new_symbolic_write_ref() -> Result<String, GitError> {
    let target = create_temp_ref_name(REFS_NOTES_WRITE_TARGET_PREFIX);

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
        reconcile_branch_with(target, &reference.oid)?;
    }

    Ok(refs)
}

//TODO(kaihowl) clean up pub methods
fn raw_push(work_dir: Option<&Path>) -> Result<(), GitError> {
    ensure_remote_exists()?;
    // This might merge concurrently created write branches. There is no protection against that.
    // This wants to achieve an at-least-once semantic. The exactly-once semantic is ensured by the
    // cat_sort_uniq merge strategy.

    // - Reset the symbolic-ref "write" to a new unique write ref.
    //     - Allows to continue committing measurements while pushing.
    //     - ?? What happens when a git notes amend concurrently still writes to the old ref?
    let new_write_ref = new_symbolic_write_ref()?;

    let merge_ref = create_temp_ref_name(REFS_NOTES_MERGE_BRANCH_PREFIX);

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

    // It is acceptable to fetch here independent of the push. Only one concurrent push will succeed.
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
}

fn git_push_notes_ref(
    expected_upstream: &str,
    push_ref: &str,
    working_dir: &Option<&Path>,
) -> Result<(), GitError> {
    // TODO(kaihowl) configure remote?
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
        working_dir,
    );

    // - Clean your own temporary merge ref and all others with a merge commit older than x days.
    //     - In case of crashes before clean up, old merge refs are eliminated eventually.

    match output {
        Ok(output) => {
            print!("{}", &output.stdout);
            Ok(())
        }
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
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_prune().map_err(map_git_error_for_backoff)
    };

    let backoff = default_backoff();

    ::backoff::retry_notify(backoff, op, retry_notify).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while pushing refs")
        }
        ::backoff::Error::Transient { err, .. } => anyhow!(err).context("Timed out pushing refs"),
    })?;

    Ok(())
}

fn raw_prune() -> Result<(), GitError> {
    if is_shallow_repo()? {
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
                    .expect("No commit header found before measurement line in git log output")
                    .to_owned(),
            );

            detected_shallow |= info[2..].contains(&"grafted");

            None
        } else {
            // TODO(kaihowl) lot's of string copies...
            Some((
                current_commit
                    .as_ref()
                    .expect("No commit header found before measurement line in git log output")
                    .to_owned(),
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

    let backoff = default_backoff();

    ::backoff::retry_notify(backoff, op, retry_notify).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while pushing refs")
        }
        ::backoff::Error::Transient { err, .. } => anyhow!(err).context("Timed out pushing refs"),
    })?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env::{self, set_current_dir};
    use std::process;

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
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
        ensure_symbolic_write_ref_exists().expect("Failed to ensure symbolic write ref exists");
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

    #[test]
    #[serial]
    fn test_empty_or_never_pushed_remote_error() {
        use chrono::Utc;
        let tempdir = tempdir().unwrap();
        init_repo(tempdir.path());
        set_current_dir(tempdir.path()).expect("Failed to change dir");
        // Add a dummy remote so the code can check for empty remote
        run_git_command(
            &["remote", "add", "origin", "https://example.com/empty.git"],
            tempdir.path(),
        );
        // Do not add any notes/measurements or push anything
        let result = super::raw_remove_measurements_from_commits(Utc::now());
        match result {
            Err(GitError::EmptyOrNeverPushedRemote { .. }) => {}
            other => panic!("Expected EmptyOrNeverPushedRemote error, got: {:?}", other),
        }
    }
}
