use crate::git::git_interop::{
    create_temp_ref_name, delete_reference, get_write_refs, walk_commits,
};
use crate::serialization::deserialize;
use anyhow::Result;
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// Information about pending measurements
#[derive(Debug)]
pub struct PendingStatus {
    /// Total number of commits with pending measurements
    pub commit_count: usize,

    /// Unique measurement names found in pending writes
    pub measurement_names: HashSet<String>,

    /// Per-commit breakdown (if detailed)
    pub per_commit: Option<Vec<CommitMeasurements>>,
}

/// Measurements for a specific commit
#[derive(Debug)]
pub struct CommitMeasurements {
    /// Commit SHA
    pub commit: String,

    /// Commit title
    pub title: String,

    /// Measurement names in this commit
    pub measurement_names: Vec<String>,

    /// Number of measurements in this commit
    pub count: usize,
}

/// Display pending measurement status
pub fn show_status(detailed: bool) -> Result<()> {
    // 1. Check if there are any pending measurements
    let status = gather_pending_status(detailed)?;

    // 2. Display results
    display_status(&status, detailed)?;

    Ok(())
}

/// Gather information about pending measurements
fn gather_pending_status(detailed: bool) -> Result<PendingStatus> {
    // Create a consolidated read branch that includes pending writes
    // but not the remote branch
    let _pending_guard = create_consolidated_pending_read_branch()?;

    // Walk all commits to find measurements
    let commits = walk_commits(usize::MAX)?;

    let mut commit_count = 0;
    let mut all_measurement_names = HashSet::new();
    let mut per_commit = if detailed { Some(Vec::new()) } else { None };

    for commit_with_notes in commits {
        if commit_with_notes.note_lines.is_empty() {
            continue;
        }

        // Deserialize measurements from note
        let note_text = commit_with_notes.note_lines.join("\n");
        let measurements = deserialize(&note_text);

        if measurements.is_empty() {
            continue;
        }

        commit_count += 1;

        // Collect unique measurement names
        let commit_measurement_names: Vec<String> =
            measurements.iter().map(|m| m.name.clone()).collect();

        for name in &commit_measurement_names {
            all_measurement_names.insert(name.clone());
        }

        // Store per-commit details if requested
        if let Some(ref mut per_commit_vec) = per_commit {
            per_commit_vec.push(CommitMeasurements {
                commit: commit_with_notes.sha.clone(),
                title: commit_with_notes.title.clone(),
                measurement_names: commit_measurement_names,
                count: measurements.len(),
            });
        }
    }

    Ok(PendingStatus {
        commit_count,
        measurement_names: all_measurement_names,
        per_commit,
    })
}

/// Create a read branch with ONLY pending writes (exclude remote)
fn create_consolidated_pending_read_branch() -> Result<PendingReadBranchGuard> {
    use crate::git::git_definitions::REFS_NOTES_READ_PREFIX;

    let temp_ref = TempRef::new(REFS_NOTES_READ_PREFIX)?;

    // Consolidate only write branches (not remote)
    let refs = get_write_refs()?;

    // Start with an empty tree
    const EMPTY_OID: &str = "0000000000000000000000000000000000000000";

    // Create or update the ref to point to empty
    let status = Command::new("git")
        .args(&["update-ref", &temp_ref.ref_name, EMPTY_OID])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to create temporary ref");
    }

    // Merge in all write refs
    for (_, oid) in &refs {
        reconcile_branch_with(&temp_ref.ref_name, oid)?;
    }

    Ok(PendingReadBranchGuard { temp_ref })
}

/// Temporary reference for reading pending measurements
struct TempRef {
    ref_name: String,
}

impl TempRef {
    fn new(prefix: &str) -> Result<Self> {
        let ref_name = create_temp_ref_name(prefix);
        Ok(TempRef { ref_name })
    }
}

impl Drop for TempRef {
    fn drop(&mut self) {
        let _ = delete_reference(&self.ref_name);
    }
}

/// Guard for the pending read branch
struct PendingReadBranchGuard {
    #[allow(dead_code)]
    temp_ref: TempRef,
}

/// Reconcile a branch with another ref using git notes merge
fn reconcile_branch_with(target: &str, oid: &str) -> Result<()> {
    let status = Command::new("git")
        .args(&[
            "notes",
            "--ref",
            target,
            "merge",
            "--strategy=cat_sort_uniq",
            oid,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to merge notes");
    }

    Ok(())
}

/// Display status information to stdout
fn display_status(status: &PendingStatus, detailed: bool) -> Result<()> {
    if status.commit_count == 0 {
        println!("No pending measurements.");
        println!("(use \"git perf add\" or \"git perf measure\" to add measurements)");
        return Ok(());
    }

    println!("Pending measurements:");
    println!("  {} commit(s) with measurements", status.commit_count);
    println!("  {} unique measurement(s)", status.measurement_names.len());
    println!();

    if !status.measurement_names.is_empty() {
        println!("Measurement names:");
        let mut sorted_names: Vec<_> = status.measurement_names.iter().collect();
        sorted_names.sort();
        for name in sorted_names {
            println!("  - {}", name);
        }
        println!();
    }

    if detailed {
        if let Some(ref per_commit) = status.per_commit {
            println!("Per-commit breakdown:");
            for commit_info in per_commit {
                let short_sha = if commit_info.commit.len() >= 12 {
                    &commit_info.commit[..12]
                } else {
                    &commit_info.commit
                };
                println!(
                    "  {} ({} measurement(s)) - {}",
                    short_sha, commit_info.count, commit_info.title
                );
                for name in &commit_info.measurement_names {
                    println!("    - {}", name);
                }
            }
            println!();
        }
    }

    println!("(use \"git perf reset\" to discard pending measurements)");
    println!("(use \"git perf push\" to publish measurements)");

    Ok(())
}
