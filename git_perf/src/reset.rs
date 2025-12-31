use crate::git::git_definitions::{
    REFS_NOTES_READ_PREFIX, REFS_NOTES_WRITE_SYMBOLIC_REF, REFS_NOTES_WRITE_TARGET_PREFIX,
};
use crate::git::git_interop::{
    create_temp_ref_name, delete_reference, get_write_refs, walk_commits,
};
use crate::serialization::deserialize;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::process::{Command, Stdio};

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
    // 1. Determine what would be reset
    let plan = plan_reset()?;

    // 2. Check if there's anything to reset
    if plan.refs_to_delete.is_empty() {
        println!("No pending measurements to reset.");
        return Ok(());
    }

    // 3. Display plan
    display_reset_plan(&plan)?;

    // 4. Get confirmation unless force or dry-run
    if !dry_run && !force {
        if !confirm_reset()? {
            println!("Reset cancelled.");
            return Ok(());
        }
    }

    // 5. Execute reset (unless dry-run)
    if dry_run {
        println!();
        println!("Dry run - no changes made.");
    } else {
        execute_reset(&plan)?;
        println!();
        println!(
            "Reset complete. {} write ref(s) deleted.",
            plan.refs_to_delete.len()
        );
    }

    Ok(())
}

/// Plan what will be reset
fn plan_reset() -> Result<ResetPlan> {
    // Get all write refs
    let refs = get_write_refs()?;

    if refs.is_empty() {
        return Ok(ResetPlan {
            refs_to_delete: vec![],
            measurement_count: 0,
            commit_count: 0,
        });
    }

    // Count measurements for display
    let (measurement_count, commit_count) = count_all_pending_measurements()?;

    Ok(ResetPlan {
        refs_to_delete: refs.into_iter().map(|(refname, _)| refname).collect(),
        measurement_count,
        commit_count,
    })
}

/// Count all pending measurements
fn count_all_pending_measurements() -> Result<(usize, usize)> {
    // Create consolidated pending branch same way as status does
    let _guard = create_consolidated_pending_read_branch()?;
    let commits = walk_commits(usize::MAX)?;

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

/// Create a consolidated pending read branch (same as in status module)
fn create_consolidated_pending_read_branch() -> Result<PendingReadBranchGuard> {
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

/// Execute the reset plan
fn execute_reset(plan: &ResetPlan) -> Result<()> {
    // According to the amendment in the issue comments, we should:
    // 1. Create a new write ref (representing the reset state)
    // 2. Delete all OTHER write refs

    // Create new write ref
    let new_write_ref = create_new_write_ref()?;

    // Delete all other write refs
    for ref_name in &plan.refs_to_delete {
        if ref_name != &new_write_ref {
            delete_reference(ref_name)
                .with_context(|| format!("Failed to delete reference: {}", ref_name))?;
        }
    }

    Ok(())
}

/// Create a new write ref for the reset operation
fn create_new_write_ref() -> Result<String> {
    let new_ref = create_temp_ref_name(REFS_NOTES_WRITE_TARGET_PREFIX);

    // Update the symbolic ref to point to the new write ref
    let status = Command::new("git")
        .args(&["symbolic-ref", REFS_NOTES_WRITE_SYMBOLIC_REF, &new_ref])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to update symbolic ref");
    }

    Ok(new_ref)
}

/// Display what will be reset
fn display_reset_plan(plan: &ResetPlan) -> Result<()> {
    println!("Will reset:");
    println!("  {} write ref(s)", plan.refs_to_delete.len());
    println!("  {} measurement(s)", plan.measurement_count);
    println!("  {} commit(s) with measurements", plan.commit_count);

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
