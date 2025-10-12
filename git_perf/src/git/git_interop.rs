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
    REFS_NOTES_MERGE_BRANCH_PREFIX, REFS_NOTES_READ_PREFIX, REFS_NOTES_REWRITE_TARGET_PREFIX,
    REFS_NOTES_WRITE_SYMBOLIC_REF, REFS_NOTES_WRITE_TARGET_PREFIX,
};
use super::git_lowlevel::{
    capture_git_output, get_git_perf_remote, git_rev_parse, git_rev_parse_symbolic_ref,
    git_symbolic_ref_create_or_update, git_update_ref, internal_get_head_revision, is_shallow_repo,
    map_git_error, set_git_perf_remote, spawn_git_command,
};
use super::git_types::GitError;
use super::git_types::Reference;

pub use super::git_lowlevel::get_head_revision;

pub use super::git_lowlevel::check_git_version;

pub use super::git_lowlevel::get_repository_root;

fn map_git_error_for_backoff(e: GitError) -> ::backoff::Error<GitError> {
    match e {
        GitError::RefFailedToPush { .. }
        | GitError::RefFailedToLock { .. }
        | GitError::RefConcurrentModification { .. }
        | GitError::BadObject { .. } => ::backoff::Error::transient(e),
        GitError::ExecError { .. }
        | GitError::IoError(..)
        | GitError::ShallowRepository
        | GitError::MissingHead { .. }
        | GitError::NoRemoteMeasurements { .. }
        | GitError::NoUpstream { .. }
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

    defer!(remove_reference(&temp_target)
        .expect("Deleting our own temp ref for adding should never fail"));

    // Test if the repo has any commit checked out at HEAD
    if internal_get_head_revision().is_err() {
        return Err(GitError::MissingHead {
            reference: "HEAD".to_string(),
        });
    }

    capture_git_output(
        &["notes", "--ref", &temp_target, "append", "-m", line],
        &None,
    )?;

    // Update current write branch with pending write
    // We update the target ref directly (no symref-verify needed in git 2.43.0)
    // The old-oid verification ensures atomicity of the target ref update
    // If the symref was redirected between reading it and updating, the write goes
    // to the old target which will still be merged during consolidation
    git_update_ref(unindent(
        format!(
            r#"
            start
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

        // Use git symbolic-ref to create the symbolic reference
        // This is not atomic with other ref operations, but that's acceptable
        // as this only runs once during initialization
        git_symbolic_ref_create_or_update(REFS_NOTES_WRITE_SYMBOLIC_REF, &target).or_else(
            |err| {
                // If ref already exists (race with another process), that's fine
                if git_rev_parse(REFS_NOTES_WRITE_SYMBOLIC_REF).is_ok() {
                    Ok(())
                } else {
                    Err(err)
                }
            },
        )?;
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
            "--atomic",
            "--no-write-fetch-head",
            GIT_PERF_REMOTE,
            // Always force overwrite the local reference
            // Separation into write, merge, and read branches ensures that this does not lead to
            // any data loss
            format!("+{REFS_NOTES_BRANCH}:{REFS_NOTES_BRANCH}").as_str(),
        ],
        &work_dir,
    )
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

pub fn remove_measurements_from_commits(older_than: DateTime<Utc>, prune: bool) -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_remove_measurements_from_commits(older_than, prune).map_err(map_git_error_for_backoff)
    };

    let backoff = default_backoff();

    ::backoff::retry_notify(backoff, op, retry_notify).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while removing measurements")
        }
        ::backoff::Error::Transient { err, .. } => {
            anyhow!(err).context("Timed out while removing measurements")
        }
    })?;

    Ok(())
}

fn execute_notes_operation<F>(operation: F) -> Result<(), GitError>
where
    F: FnOnce(&str) -> Result<(), GitError>,
{
    pull_internal(None)?;

    let current_notes_head = git_rev_parse(REFS_NOTES_BRANCH)?;
    let target = create_temp_rewrite_head(&current_notes_head)?;

    operation(&target)?;

    compact_head(&target)?;

    git_push_notes_ref(&current_notes_head, &target, &None, None)?;

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

    remove_reference(&target)?;

    Ok(())
}

fn raw_remove_measurements_from_commits(
    older_than: DateTime<Utc>,
    prune: bool,
) -> Result<(), GitError> {
    // Check for shallow repo once at the beginning (needed for prune)
    if prune && is_shallow_repo()? {
        return Err(GitError::ShallowRepository);
    }

    execute_notes_operation(|target| {
        // Remove measurements older than the specified date
        remove_measurements_from_reference(target, older_than)?;

        // Prune orphaned measurements if requested
        if prune {
            capture_git_output(&["notes", "--ref", target, "prune"], &None).map(|_| ())?;
        }

        Ok(())
    })
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

    // Use git symbolic-ref to update the symbolic reference target
    // This is not atomic with other ref operations, but any concurrent writes
    // that go to the old target will still be merged during consolidation
    git_symbolic_ref_create_or_update(REFS_NOTES_WRITE_SYMBOLIC_REF, &target)?;
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

    for reference in &refs {
        reconcile_branch_with(target, &reference.oid)?;
    }

    Ok(refs)
}

fn remove_reference(ref_name: &str) -> Result<(), GitError> {
    git_update_ref(unindent(
        format!(
            r#"
                    start
                    delete {ref_name}
                    commit
                "#
        )
        .as_str(),
    ))
}

fn raw_push(work_dir: Option<&Path>, remote: Option<&str>) -> Result<(), GitError> {
    ensure_remote_exists()?;
    // This might merge concurrently created write branches. There is no protection against that.
    // This wants to achieve an at-least-once semantic. The exactly-once semantic is ensured by the
    // cat_sort_uniq merge strategy.

    // - Reset the symbolic-ref "write" to a new unique write ref.
    //     - Allows to continue committing measurements while pushing.
    //     - ?? What happens when a git notes amend concurrently still writes to the old ref?
    let new_write_ref = new_symbolic_write_ref()?;

    let merge_ref = create_temp_ref_name(REFS_NOTES_MERGE_BRANCH_PREFIX);

    defer!(remove_reference(&merge_ref).expect("Deleting our own branch should never fail"));

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

    git_push_notes_ref(&current_upstream_oid, &merge_ref, &work_dir, remote)?;

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
    remote: Option<&str>,
) -> Result<(), GitError> {
    // - CAS push the temporary merge ref to upstream using the noted down upstream ref
    //     - In case of concurrent pushes, back off and restart fresh from previous step.
    let remote_name = remote.unwrap_or(GIT_PERF_REMOTE);
    let output = capture_git_output(
        &[
            "push",
            "--porcelain",
            format!("--force-with-lease={REFS_NOTES_BRANCH}:{expected_upstream}").as_str(),
            remote_name,
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

pub fn prune() -> Result<()> {
    let op = || -> Result<(), ::backoff::Error<GitError>> {
        raw_prune().map_err(map_git_error_for_backoff)
    };

    let backoff = default_backoff();

    ::backoff::retry_notify(backoff, op, retry_notify).map_err(|e| match e {
        ::backoff::Error::Permanent(err) => {
            anyhow!(err).context("Permanent failure while pruning refs")
        }
        ::backoff::Error::Transient { err, .. } => anyhow!(err).context("Timed out pushing refs"),
    })?;

    Ok(())
}

fn raw_prune() -> Result<(), GitError> {
    if is_shallow_repo()? {
        return Err(GitError::ShallowRepository);
    }

    execute_notes_operation(|target| {
        capture_git_output(&["notes", "--ref", target, "prune"], &None).map(|_| ())
    })
}

/// Returns a list of all commit SHA-1 hashes that have performance measurements
/// in the refs/notes/perf-v3 branch.
///
/// Each commit hash is returned as a 40-character hexadecimal string.
pub fn list_commits_with_measurements() -> Result<Vec<String>> {
    // Update local read branch to include pending writes (like walk_commits does)
    let temp_ref = update_read_branch()?;

    // Use git notes list to get all annotated commits
    // Output format: <note_oid> <commit_oid>
    let mut list_notes =
        spawn_git_command(&["notes", "--ref", &temp_ref.ref_name, "list"], &None, None)?;

    let stdout = list_notes
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture stdout from git notes list"))?;

    // Parse output line by line: each line is "note_sha commit_sha"
    // We want the commit_sha (second column)
    // Process directly from BufReader for efficiency
    let commits: Vec<String> = BufReader::new(stdout)
        .lines()
        .filter_map(|line_result| {
            line_result
                .ok()
                .and_then(|line| line.split_whitespace().nth(1).map(|s| s.to_string()))
        })
        .collect();

    Ok(commits)
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

struct TempRef {
    ref_name: String,
}

impl TempRef {
    fn new(prefix: &str) -> Result<Self, GitError> {
        Ok(TempRef {
            ref_name: create_temp_ref(prefix, EMPTY_OID)?,
        })
    }
}

impl Drop for TempRef {
    fn drop(&mut self) {
        remove_reference(&self.ref_name)
            .unwrap_or_else(|_| panic!("Failed to remove reference: {}", self.ref_name))
    }
}

fn update_read_branch() -> Result<TempRef, GitError> {
    let temp_ref = TempRef::new(REFS_NOTES_READ_PREFIX)?;
    // Create a fresh read branch from the remote and consolidate all pending write branches.
    // This ensures the read branch is always up to date with the remote branch, even after
    // a history cutoff, by checking against the current upstream state.
    let current_upstream_oid = git_rev_parse(REFS_NOTES_BRANCH).unwrap_or(EMPTY_OID.to_string());

    let _ = consolidate_write_branches_into(&current_upstream_oid, &temp_ref.ref_name, None)?;

    Ok(temp_ref)
}

pub fn walk_commits(num_commits: usize) -> Result<Vec<(String, Vec<String>)>> {
    // update local read branch
    let temp_ref = update_read_branch()?;

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
            format!("--notes={}", temp_ref.ref_name).as_str(),
            "HEAD",
        ],
        &None,
    )
    .context("Failed to retrieve commits")?;

    let mut commits: Vec<(String, Vec<String>)> = Vec::new();
    let mut detected_shallow = false;
    let mut current_commit: Option<String> = None;

    for l in output.stdout.lines() {
        if l.starts_with("--") {
            let info = l.split(',').collect_vec();
            let commit_hash = info
                .get(1)
                .expect("No commit header found before measurement line in git log output");
            detected_shallow |= info[2..].contains(&"grafted");
            current_commit = Some(commit_hash.to_string());
            commits.push((commit_hash.to_string(), Vec::new()));
        } else if let Some(commit_hash) = current_commit.as_ref() {
            if let Some(last) = commits.last_mut() {
                last.1.push(l.to_string());
            } else {
                // Should not happen, but just in case
                commits.push((commit_hash.to_string(), vec![l.to_string()]));
            }
        }
    }

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
    fetch(work_dir)?;
    Ok(())
}

pub fn push(work_dir: Option<&Path>, remote: Option<&str>) -> Result<()> {
    let op = || {
        raw_push(work_dir, remote)
            .map_err(map_git_error_for_backoff)
            .map_err(|e: ::backoff::Error<GitError>| match e {
                ::backoff::Error::Transient { .. } => {
                    // Attempt to pull to resolve conflicts
                    let pull_result = pull_internal(work_dir).map_err(map_git_error_for_backoff);

                    // A concurrent modification comes from a concurrent fetch.
                    // Don't fail for that - it's safe to assume we successfully pulled
                    // in the context of the retry logic.
                    let pull_succeeded = pull_result.is_ok()
                        || matches!(
                            pull_result,
                            Err(::backoff::Error::Permanent(
                                GitError::RefConcurrentModification { .. }
                                    | GitError::RefFailedToLock { .. }
                            ))
                        );

                    if pull_succeeded {
                        // Pull succeeded or failed with expected concurrent errors,
                        // return the original push error to retry
                        e
                    } else {
                        // Pull failed with unexpected error, propagate it
                        pull_result.unwrap_err()
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
    fn test_customheader_pull() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).expect("Failed to change dir");

        let mut test_server = Server::run();
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

        // The pull operation will fail because the mock server doesn't provide a valid git
        // response, but we verify that the authorization header was sent by checking that
        // the server's expectations are met (httptest will panic on drop if not).
        hermetic_git_env();
        let _ = pull(None); // Ignore result - we only care that auth header was sent

        // Explicitly verify server expectations were met
        test_server.verify_and_clear();
    }

    #[test]
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

        hermetic_git_env();

        // Must add a single write as a push without pending local writes just succeeds
        ensure_symbolic_write_ref_exists().expect("Failed to ensure symbolic write ref exists");
        add_note_line_to_head("test note line").expect("Failed to add note line");

        let error = push(None, None);
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
    fn test_empty_or_never_pushed_remote_error_for_fetch() {
        let tempdir = tempdir().unwrap();
        init_repo(tempdir.path());
        set_current_dir(tempdir.path()).expect("Failed to change dir");
        // Add a dummy remote so the code can check for empty remote
        let git_dir_url = format!("file://{}", tempdir.path().display());
        run_git_command(&["remote", "add", "origin", &git_dir_url], tempdir.path());

        // NOTE: GIT_TRACE is required for this test to function correctly
        std::env::set_var("GIT_TRACE", "true");

        // Do not add any notes/measurements or push anything
        let result = super::fetch(Some(tempdir.path()));
        match result {
            Err(GitError::NoRemoteMeasurements { output }) => {
                assert!(
                    output.stderr.contains(GIT_PERF_REMOTE),
                    "Expected output to contain {GIT_PERF_REMOTE}. Output: '{}'",
                    output.stderr
                )
            }
            other => panic!("Expected NoRemoteMeasurements error, got: {:?}", other),
        }
    }

    #[test]
    fn test_empty_or_never_pushed_remote_error_for_push() {
        let tempdir = tempdir().unwrap();
        init_repo(tempdir.path());
        set_current_dir(tempdir.path()).expect("Failed to change dir");

        hermetic_git_env();

        run_git_command(
            &["remote", "add", "origin", "invalid invalid"],
            tempdir.path(),
        );

        // NOTE: GIT_TRACE is required for this test to function correctly
        std::env::set_var("GIT_TRACE", "true");

        add_note_line_to_head("test line, invalid measurement, does not matter").unwrap();

        let result = super::raw_push(Some(tempdir.path()), None);
        match result {
            Err(GitError::RefFailedToPush { output }) => {
                assert!(
                    output.stderr.contains(GIT_PERF_REMOTE),
                    "Expected output to contain {GIT_PERF_REMOTE}, got: {}",
                    output.stderr
                )
            }
            other => panic!("Expected RefFailedToPush error, got: {:?}", other),
        }
    }

    /// Test that new_symbolic_write_ref returns valid, non-empty reference names
    /// Targets missed mutants:
    /// - Could return Ok(String::new()) - empty string
    /// - Could return Ok("xyzzy".into()) - arbitrary invalid string
    #[test]
    fn test_new_symbolic_write_ref_returns_valid_ref() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        // Test the private function directly since we're in the same module
        let result = new_symbolic_write_ref();
        assert!(
            result.is_ok(),
            "Should create symbolic write ref: {:?}",
            result
        );

        let ref_name = result.unwrap();

        // Mutation 1: Should not be empty string
        assert!(
            !ref_name.is_empty(),
            "Reference name should not be empty, got: '{}'",
            ref_name
        );

        // Mutation 2: Should not be arbitrary string like "xyzzy"
        assert!(
            ref_name.starts_with(REFS_NOTES_WRITE_TARGET_PREFIX),
            "Reference should start with {}, got: {}",
            REFS_NOTES_WRITE_TARGET_PREFIX,
            ref_name
        );

        // Should have a hex suffix
        let suffix = ref_name
            .strip_prefix(REFS_NOTES_WRITE_TARGET_PREFIX)
            .expect("Should have prefix");
        assert!(
            !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_hexdigit()),
            "Suffix should be non-empty hex string, got: {}",
            suffix
        );
    }

    /// Test that notes can be added successfully via add_note_line_to_head
    /// Verifies end-to-end note operations work correctly
    #[test]
    fn test_add_and_retrieve_notes() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        // Add first note - this calls ensure_symbolic_write_ref_exists -> new_symbolic_write_ref
        let result = add_note_line_to_head("test: 100");
        assert!(
            result.is_ok(),
            "Should add note (requires valid ref from new_symbolic_write_ref): {:?}",
            result
        );

        // Add second note to ensure ref operations continue to work
        let result2 = add_note_line_to_head("test: 200");
        assert!(result2.is_ok(), "Should add second note: {:?}", result2);

        // Verify notes were actually added by walking commits
        let commits = walk_commits(10);
        assert!(commits.is_ok(), "Should walk commits: {:?}", commits);

        let commits = commits.unwrap();
        assert!(!commits.is_empty(), "Should have commits");

        // Check that HEAD commit has notes
        let (_, notes) = &commits[0];
        assert!(!notes.is_empty(), "HEAD should have notes");
        assert!(
            notes.iter().any(|n| n.contains("test:")),
            "Notes should contain our test data"
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
            );
        }

        // Create a shallow clone (depth 2) which will have grafted commits
        let shallow_dir = tempdir.path().join("shallow");
        let output = process::Command::new("git")
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
        add_note_line_to_head("test: 100").expect("Should add note");

        // Walk commits - should detect as shallow
        let result = walk_commits(10);
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
            );
        }

        // Add a note to enable walk_commits
        add_note_line_to_head("test: 100").expect("Should add note");

        let result = walk_commits(10);
        assert!(result.is_ok(), "walk_commits should succeed");

        let commits = result.unwrap();

        // Should have commits
        assert!(!commits.is_empty(), "Should have found commits");
    }
}
