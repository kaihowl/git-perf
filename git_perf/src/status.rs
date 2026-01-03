use crate::git::git_interop::{
    create_consolidated_pending_read_branch, get_commit_details, get_commits_with_notes,
    get_notes_for_commit,
};
use crate::serialization::deserialize;
use anyhow::Result;
use std::collections::HashSet;

/// Information about pending measurements
#[derive(Debug)]
pub struct PendingStatus {
    /// Total number of commits with pending measurements
    pub commit_count: usize,

    /// Total number of measurements across all commits
    pub measurement_count: usize,

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
pub fn gather_pending_status(detailed: bool) -> Result<PendingStatus> {
    // Create a consolidated read branch that includes ONLY pending writes
    // (not the remote branch). After a successful push, the write refs are deleted,
    // so this branch only contains measurements that haven't been pushed yet.
    let pending_guard = create_consolidated_pending_read_branch()?;

    // Get the temporary ref name from the guard
    let pending_ref = pending_guard.ref_name();

    // Efficiently get commits that have notes in the pending branch
    // These are all commits with pending (unpushed) measurements
    let pending_commits = get_commits_with_notes(pending_ref)?;

    let mut commit_count = 0;
    let mut measurement_count = 0;
    let mut all_measurement_names = HashSet::new();
    let mut per_commit = if detailed { Some(Vec::new()) } else { None };

    for commit_sha in &pending_commits {
        let note_lines = get_notes_for_commit(pending_ref, commit_sha)?;
        if note_lines.is_empty() {
            continue;
        }

        // Deserialize measurements from note
        let note_text = note_lines.join("\n");
        let measurements = deserialize(&note_text);

        if measurements.is_empty() {
            continue;
        }

        commit_count += 1;
        measurement_count += measurements.len();

        // Collect measurement names
        let measurement_names: Vec<String> = measurements.iter().map(|m| m.name.clone()).collect();

        for name in &measurement_names {
            all_measurement_names.insert(name.clone());
        }

        // Store per-commit details if requested
        if let Some(ref mut per_commit_vec) = per_commit {
            // Get commit details (title, author)
            let commit_details = get_commit_details(std::slice::from_ref(commit_sha))?;
            if let Some(commit_info) = commit_details.first() {
                per_commit_vec.push(CommitMeasurements {
                    commit: commit_sha.clone(),
                    title: commit_info.title.clone(),
                    measurement_names: measurement_names.clone(),
                    count: measurements.len(),
                });
            }
        }
    }

    Ok(PendingStatus {
        commit_count,
        measurement_count,
        measurement_names: all_measurement_names,
        per_commit,
    })
}

/// Display status information to stdout
fn display_status(status: &PendingStatus, detailed: bool) -> Result<()> {
    if status.commit_count == 0 {
        println!("No pending measurements.");
        println!("(use \"git perf add\" or \"git perf measure\" to add measurements)");
        return Ok(());
    }

    println!("Pending measurements:");
    let commit_word = if status.commit_count == 1 {
        "commit"
    } else {
        "commits"
    };
    println!(
        "  {} {} with measurements",
        status.commit_count, commit_word
    );
    let measurement_word = if status.measurement_names.len() == 1 {
        "measurement"
    } else {
        "measurements"
    };
    println!(
        "  {} unique {}",
        status.measurement_names.len(),
        measurement_word
    );
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
                let meas_word = if commit_info.count == 1 {
                    "measurement"
                } else {
                    "measurements"
                };
                println!(
                    "  {} ({} {}) - {}",
                    short_sha, commit_info.count, meas_word, commit_info.title
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
