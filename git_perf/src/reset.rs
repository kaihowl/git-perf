use crate::git::git_interop::{
    create_consolidated_pending_read_branch, create_new_write_ref, delete_reference,
    get_write_refs, walk_commits,
};
use crate::serialization::deserialize;
use anyhow::{Context, Result};
use std::io::{self, Write};

/// Information about what will be reset
#[derive(Debug)]
pub struct ResetPlan {
    /// References that will be deleted
    pub refs_to_delete: Vec<String>,

    /// Number of measurements that will be removed
    pub measurement_count: usize,

    /// Number of commits affected
    pub commit_count: usize,
}

/// Reset (discard) pending measurements
pub fn reset_measurements(dry_run: bool, force: bool) -> Result<()> {
    // CRITICAL: Create a fresh write ref FIRST, before gathering refs to delete.
    // This ensures that any concurrent measurements added during the reset operation
    // will go to the new write ref and won't be accidentally deleted.
    let new_write_ref = create_new_write_ref().context("Failed to create fresh write ref")?;

    // Now gather the refs to delete (this will NOT include the new write ref we just created)
    let plan = plan_reset(&new_write_ref)?;

    // Check if there's anything to reset
    if plan.refs_to_delete.is_empty() {
        println!("No pending measurements to reset.");
        return Ok(());
    }

    // Display plan
    display_reset_plan(&plan)?;

    // Get confirmation unless force or dry-run
    if !dry_run && !force && !confirm_reset()? {
        println!("Reset cancelled.");
        return Ok(());
    }

    // Execute reset (unless dry-run)
    if dry_run {
        println!();
        println!("Dry run - no changes made.");
    } else {
        execute_reset(&plan)?;
        println!();
        let ref_word = if plan.refs_to_delete.len() == 1 {
            "ref"
        } else {
            "refs"
        };
        println!(
            "Reset complete. {} write {} deleted.",
            plan.refs_to_delete.len(),
            ref_word
        );
    }

    Ok(())
}

/// Plan what will be reset
///
/// The new_write_ref parameter is the ref we just created, which should NOT be deleted.
fn plan_reset(new_write_ref: &str) -> Result<ResetPlan> {
    // Get all write refs
    let refs = get_write_refs()?;

    // Filter out the new write ref we just created
    let refs_to_delete: Vec<String> = refs
        .into_iter()
        .map(|(refname, _)| refname)
        .filter(|refname| refname != new_write_ref)
        .collect();

    if refs_to_delete.is_empty() {
        return Ok(ResetPlan {
            refs_to_delete: vec![],
            measurement_count: 0,
            commit_count: 0,
        });
    }

    // Count measurements for display
    let (measurement_count, commit_count) = count_all_pending_measurements()?;

    Ok(ResetPlan {
        refs_to_delete,
        measurement_count,
        commit_count,
    })
}

/// Count all pending measurements
fn count_all_pending_measurements() -> Result<(usize, usize)> {
    // Create consolidated pending branch using git module function
    let _guard = create_consolidated_pending_read_branch()?;
    // Use a large but reasonable number instead of usize::MAX to avoid overflow issues
    let commits = walk_commits(1_000_000)?;

    let mut total_measurements = 0;
    let mut commit_count = 0;

    for commit_with_notes in commits {
        if commit_with_notes.note_lines.is_empty() {
            continue;
        }

        let note_text = commit_with_notes.note_lines.join("\n");
        let measurements = deserialize(&note_text);
        if !measurements.is_empty() {
            total_measurements += measurements.len();
            commit_count += 1;
        }
    }

    Ok((total_measurements, commit_count))
}

/// Execute the reset plan
fn execute_reset(plan: &ResetPlan) -> Result<()> {
    // Delete all the old write refs
    // The new write ref was already created before planning, so it won't be in this list
    for ref_name in &plan.refs_to_delete {
        delete_reference(ref_name)
            .with_context(|| format!("Failed to delete reference: {}", ref_name))?;
    }

    Ok(())
}

/// Display what will be reset
fn display_reset_plan(plan: &ResetPlan) -> Result<()> {
    println!("Will reset:");
    let ref_word = if plan.refs_to_delete.len() == 1 {
        "ref"
    } else {
        "refs"
    };
    println!("  {} write {}", plan.refs_to_delete.len(), ref_word);
    let measurement_word = if plan.measurement_count == 1 {
        "measurement"
    } else {
        "measurements"
    };
    println!("  {} {}", plan.measurement_count, measurement_word);
    let commit_word = if plan.commit_count == 1 {
        "commit"
    } else {
        "commits"
    };
    println!("  {} {} with measurements", plan.commit_count, commit_word);

    Ok(())
}

/// Prompt user for confirmation
fn confirm_reset() -> Result<bool> {
    print!("Are you sure you want to discard these pending measurements? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}
