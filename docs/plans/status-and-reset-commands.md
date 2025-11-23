# Plan: Add Commands to Manage Pending Local Measurements

**Status:** Planning
**Created:** 2025-11-23
**Issue:** #485

## Overview

Add two new commands to git-perf to manage locally pending measurements (measurements that have been added but not yet pushed): `status` to view pending measurements and `reset` to drop/discard them. This provides a workflow similar to `git status` and `git reset` for measurement management.

## Motivation

Currently, git-perf users who create test measurements locally have no convenient way to:
- See which measurements are pending (not yet pushed to remote)
- Identify which commits have pending measurements
- Selectively or completely discard local measurements before pushing
- Clean up experimental measurements without affecting remote data

This leads to:
- Accidental pushing of test/debug measurements
- Cluttered measurement history with unwanted data
- No visibility into local-only vs. published measurements
- Manual git operations to clean up measurement notes

A `status` command would provide:
- Quick overview of pending measurements similar to `git status`
- Count of commits with pending measurements
- List of measurement names that are pending
- Clear distinction between local and remote state

A `reset` command would provide:
- Safe way to discard local pending measurements
- Options to reset all or specific measurements
- Prevention of data loss by only affecting unpushed data
- Workflow similar to `git reset` for familiarity

## Goals

1. **Status command** - Show pending measurements that haven't been pushed
   - Display count of commits with pending measurements
   - List unique measurement names that are pending
   - Show summary similar to `git status` output
   - Optional detailed view showing per-commit breakdown

2. **Reset command** - Drop locally pending measurements
   - Remove all pending measurements (default behavior)
   - Optionally filter by measurement name
   - Optionally filter by commit/commit range
   - Preserve remote measurements (only affect local pending)
   - Provide confirmation/dry-run options for safety

3. **Safety and usability**
   - Never affect measurements that have been pushed
   - Clear error messages if no pending measurements exist
   - Graceful handling of edge cases (shallow repos, etc.)
   - Consistent with git-perf's existing command patterns

## Non-Goals (Future Work)

- Interactive mode for selective reset (pick which measurements to drop)
- Reset by date/time range
- Undo/recovery of reset measurements
- Stashing measurements (save for later)
- Comparison of local vs remote measurements
- Reset with automatic backup

## Background: Measurement Storage Architecture

Based on analysis of `git_perf/src/git/git_interop.rs`:

### How Measurements are Stored

1. **Write Branches** (`refs/notes/perf-write-*`):
   - Temporary refs created when measurements are added
   - Each process gets a unique write ref with random suffix
   - Symbolic ref `refs/notes/perf-write` points to current write target
   - Multiple write refs can exist concurrently

2. **Read Branch** (`refs/notes/perf-v3`):
   - Canonical branch containing all published measurements
   - Updated during `git perf push` operations
   - Fetched from remote during `git perf pull`
   - Used by all read operations (report, audit, etc.)

3. **Merge Process**:
   - `push` consolidates all write refs into merge branch
   - Merge branch is pushed to remote
   - After successful push, write refs are deleted
   - Uses `cat_sort_uniq` merge strategy for deduplication

### Pending vs Published Measurements

**Pending Measurements**:
- Exist in write refs (`refs/notes/perf-write-*`)
- Have not been pushed to remote
- Can be safely discarded without affecting others
- Created by: `git perf add`, `git perf measure`, `git perf import`

**Published Measurements**:
- Exist in remote `refs/notes/perf-v3`
- Visible to all users after `git perf pull`
- Cannot be discarded with `reset` (require `remove` or `prune`)
- Created by: `git perf push`

### Key Functions to Leverage

From `git_interop.rs`:
- `git_rev_parse()` - Get OID of a reference
- `git_update_ref()` - Atomically update/delete references
- `get_refs()` - List references matching pattern
- `walk_commits()` - Read measurements from commits
- `create_consolidated_read_branch()` - Merge write refs for reading
- `remove_reference()` - Delete a git reference

## Design

### 1. CLI Definitions

**File**: `cli_types/src/lib.rs`

Add to `Commands` enum (after `Prune`, around line 417):

```rust
/// Show pending measurements that haven't been pushed
///
/// Lists local measurements that exist in write branches but haven't been
/// pushed to the remote repository. Similar to `git status` for tracking
/// which changes are pending.
///
/// Pending measurements are those created with `add`, `measure`, or `import`
/// that haven't been published via `push`. These can be safely discarded
/// with the `reset` command.
///
/// Examples:
///   git perf status                    # Show summary of pending measurements
///   git perf status --detailed         # Show per-commit breakdown
///   git perf status --measurement M    # Only show specific measurement
Status {
    /// Show detailed per-commit breakdown
    #[arg(short, long)]
    detailed: bool,

    /// Filter by specific measurement name (can be specified multiple times)
    #[arg(short, long)]
    measurement: Vec<String>,
},

/// Drop locally pending measurements that haven't been pushed
///
/// Removes measurements from local write branches that haven't been pushed
/// to the remote repository. This is useful for discarding test or debug
/// measurements before publishing.
///
/// IMPORTANT: This only affects local pending measurements. Measurements that
/// have been pushed to the remote are not affected. Use `remove` or `prune`
/// for managing published measurements.
///
/// By default, removes ALL pending measurements. Use filters to be selective.
///
/// Examples:
///   git perf reset                        # Remove all pending measurements
///   git perf reset --dry-run              # Preview what would be reset
///   git perf reset --measurement M        # Only reset specific measurement
///   git perf reset --commit HEAD~3..HEAD  # Reset for specific commit range
Reset {
    /// Preview what would be reset without actually resetting
    #[arg(long)]
    dry_run: bool,

    /// Only reset measurements with these names (can be specified multiple times)
    #[arg(short, long)]
    measurement: Vec<String>,

    /// Only reset measurements for specific commit(s) or range
    /// Format: commit-ish (e.g., HEAD, abc123, main~5..main)
    #[arg(short, long)]
    commit: Option<String>,

    /// Skip confirmation prompt (dangerous)
    #[arg(short, long)]
    force: bool,
},
```

**Location**: After `Prune {}` command (around line 416)

### 2. Status Module Implementation

**File**: `git_perf/src/status.rs` (NEW FILE)

#### Core Data Structures

```rust
use crate::git::git_interop::create_consolidated_read_branch;
use crate::serialization::deserialize;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};

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

    /// Measurement names in this commit
    pub measurement_names: Vec<String>,

    /// Number of measurements in this commit
    pub count: usize,
}
```

#### Main Entry Point

```rust
/// Display pending measurement status
pub fn show_status(
    detailed: bool,
    measurement_filter: &[String],
) -> Result<()> {
    // 1. Check if there are any pending measurements
    let status = gather_pending_status(detailed, measurement_filter)?;

    // 2. Display results
    display_status(&status, detailed)?;

    Ok(())
}

/// Gather information about pending measurements
fn gather_pending_status(
    detailed: bool,
    measurement_filter: &[String],
) -> Result<PendingStatus> {
    // Create a consolidated read branch that includes pending writes
    // but not the remote branch
    let pending_guard = create_consolidated_pending_read_branch()?;

    // Walk all commits to find measurements
    let commits = crate::git::git_interop::walk_commits(usize::MAX)?;

    let mut commit_count = 0;
    let mut all_measurement_names = HashSet::new();
    let mut per_commit = if detailed {
        Some(Vec::new())
    } else {
        None
    };

    for (commit_sha, note_lines) in commits {
        if note_lines.is_empty() {
            continue;
        }

        // Deserialize measurements from note
        let measurements = deserialize(&note_lines.join("\n"));

        if measurements.is_empty() {
            continue;
        }

        // Filter by measurement name if requested
        let filtered_measurements: Vec<_> = if measurement_filter.is_empty() {
            measurements
        } else {
            measurements
                .into_iter()
                .filter(|m| measurement_filter.contains(&m.name))
                .collect()
        };

        if filtered_measurements.is_empty() {
            continue;
        }

        commit_count += 1;

        // Collect unique measurement names
        let commit_measurement_names: Vec<String> = filtered_measurements
            .iter()
            .map(|m| m.name.clone())
            .collect();

        for name in &commit_measurement_names {
            all_measurement_names.insert(name.clone());
        }

        // Store per-commit details if requested
        if let Some(ref mut per_commit_vec) = per_commit {
            per_commit_vec.push(CommitMeasurements {
                commit: commit_sha.clone(),
                measurement_names: commit_measurement_names,
                count: filtered_measurements.len(),
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
fn create_consolidated_pending_read_branch() -> Result<crate::git::git_interop::ReadBranchGuard> {
    // This is similar to update_read_branch() but without the remote branch
    // We only want to see what's in write refs, not what's been published
    use crate::git::git_definitions::REFS_NOTES_WRITE_TARGET_PREFIX;
    use crate::git::git_lowlevel::git_rev_parse;
    use crate::git::git_types::TempRef;

    let temp_ref = TempRef::new(REFS_NOTES_READ_PREFIX)?;

    // Consolidate only write branches (not remote)
    let refs = get_refs(vec![format!("{REFS_NOTES_WRITE_TARGET_PREFIX}*")])?;

    for reference in &refs {
        reconcile_branch_with(&temp_ref.ref_name, &reference.oid)?;
    }

    Ok(ReadBranchGuard { temp_ref })
}
```

#### Display Functions

```rust
/// Display status information to stdout
fn display_status(status: &PendingStatus, detailed: bool) -> Result<()> {
    if status.commit_count == 0 {
        println!("No pending measurements.");
        println!("(use \"git perf add\" or \"git perf measure\" to add measurements)");
        return Ok(());
    }

    println!("Pending measurements:");
    println!("  {} commit(s) with measurements", status.commit_count);
    println!(
        "  {} unique measurement(s)",
        status.measurement_names.len()
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
                println!(
                    "  {} ({} measurement(s))",
                    &commit_info.commit[..12], // Short SHA
                    commit_info.count
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
```

### 3. Reset Module Implementation

**File**: `git_perf/src/reset.rs` (NEW FILE)

#### Core Data Structures

```rust
use crate::git::git_definitions::REFS_NOTES_WRITE_TARGET_PREFIX;
use crate::git::git_interop::{create_consolidated_read_branch, walk_commits};
use crate::git::git_lowlevel::{get_refs, remove_reference};
use crate::serialization::{deserialize, serialize};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
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
```

#### Main Entry Point

```rust
/// Reset (discard) pending measurements
pub fn reset_measurements(
    dry_run: bool,
    measurement_filter: &[String],
    commit_filter: Option<&str>,
    force: bool,
) -> Result<()> {
    // 1. Determine what would be reset
    let plan = plan_reset(measurement_filter, commit_filter)?;

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
        println!("Reset complete. {} write ref(s) deleted.", plan.refs_to_delete.len());
    }

    Ok(())
}

/// Plan what will be reset
fn plan_reset(
    measurement_filter: &[String],
    commit_filter: Option<&str>,
) -> Result<ResetPlan> {
    // Get all write refs
    let refs = get_refs(vec![format!("{REFS_NOTES_WRITE_TARGET_PREFIX}*")])?;

    if refs.is_empty() {
        return Ok(ResetPlan {
            refs_to_delete: vec![],
            measurement_count: 0,
            commit_count: 0,
        });
    }

    // If no filters, delete all write refs
    if measurement_filter.is_empty() && commit_filter.is_none() {
        // Count measurements for display
        let (measurement_count, commit_count) = count_all_pending_measurements()?;

        return Ok(ResetPlan {
            refs_to_delete: refs.into_iter().map(|r| r.refname).collect(),
            measurement_count,
            commit_count,
        });
    }

    // With filters, we need selective reset
    // This is more complex - we need to rewrite write refs
    plan_selective_reset(measurement_filter, commit_filter, &refs)
}

/// Count all pending measurements
fn count_all_pending_measurements() -> Result<(usize, usize)> {
    let guard = create_consolidated_read_branch()?;
    let commits = walk_commits(usize::MAX)?;

    let mut total_measurements = 0;
    let mut commit_count = 0;

    for (_commit, note_lines) in commits {
        if note_lines.is_empty() {
            continue;
        }

        let measurements = deserialize(&note_lines.join("\n"));
        if !measurements.is_empty() {
            total_measurements += measurements.len();
            commit_count += 1;
        }
    }

    Ok((total_measurements, commit_count))
}

/// Plan selective reset (with filters)
fn plan_selective_reset(
    measurement_filter: &[String],
    commit_filter: Option<&str>,
    refs: &[Reference],
) -> Result<ResetPlan> {
    // For selective reset, we need to:
    // 1. Read all measurements from write refs
    // 2. Filter out unwanted measurements
    // 3. Rewrite refs with remaining measurements
    // 4. Delete refs that become empty

    // This is complex and error-prone. For v1, we can:
    // - Only support full reset (no filters)
    // - Or: delete all write refs and prompt user to re-add what they want to keep

    // For safety, let's not support selective reset in v1
    if !measurement_filter.is_empty() || commit_filter.is_some() {
        bail!(
            "Selective reset (by measurement or commit) is not yet supported.\n\
             To reset specific measurements, use 'git perf reset' to discard all pending,\n\
             then re-add the measurements you want to keep."
        );
    }

    unreachable!("This code path should not be reached with current filters")
}
```

#### Reset Execution

```rust
/// Execute the reset plan
fn execute_reset(plan: &ResetPlan) -> Result<()> {
    for ref_name in &plan.refs_to_delete {
        remove_reference(ref_name)
            .with_context(|| format!("Failed to delete reference: {}", ref_name))?;
    }

    Ok(())
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
```

### 4. Command Handlers

**File**: `git_perf/src/cli.rs`

Add to match statement (after `Prune`, around line 127):

```rust
Commands::Status {
    detailed,
    measurement,
} => status::show_status(*detailed, measurement),

Commands::Reset {
    dry_run,
    measurement,
    commit,
    force,
} => reset::reset_measurements(
    *dry_run,
    measurement,
    commit.as_deref(),
    *force,
),
```

Add module declarations at top of file:

```rust
mod status;
mod reset;
```

### 5. Module Registration

**File**: `git_perf/src/lib.rs`

Ensure the `status` and `reset` modules are declared.

## Implementation Phases

### Phase 1: Status Command (Basic)

1. Add CLI definition for `Status` command
2. Create `git_perf/src/status.rs` with basic implementation
   - Gather pending measurements from write refs
   - Display summary (commit count, measurement names)
3. Wire up command handler
4. Test manually with local measurements
5. Run `cargo fmt` and `cargo clippy`

### Phase 2: Status Command (Enhanced)

1. Add `--detailed` flag support
   - Show per-commit breakdown
   - Include measurement counts
2. Add measurement filtering
3. Improve output formatting
4. Add helpful hints in output

### Phase 3: Reset Command (Basic)

1. Add CLI definition for `Reset` command
2. Create `git_perf/src/reset.rs` with basic implementation
   - Support full reset only (no filters)
   - Add confirmation prompt
   - Add dry-run support
3. Wire up command handler
4. Test carefully with test data

### Phase 4: Reset Command (Enhanced)

1. Add `--force` flag to skip confirmation
2. Improve error messages and safety checks
3. Add better dry-run output
4. Consider selective reset (stretch goal)

### Phase 5: Documentation

1. Add comprehensive doc comments to CLI definitions
2. Run `./scripts/generate-manpages.sh`
3. Commit regenerated manpages
4. Update README if needed
5. Add examples to documentation

### Phase 6: Testing

1. Create `test/test_status.sh` integration test
2. Create `test/test_reset.sh` integration test
3. Add tests to `test/run_tests.sh`
4. Run full test suite
5. Manual testing with various scenarios

### Phase 7: Code Quality

1. Run `cargo fmt` to format code
2. Run `cargo clippy` and address warnings
3. Review error handling
4. Add unit tests for helper functions
5. Performance testing

## Integration Tests

**File**: `test/test_status.sh` (NEW FILE)

```bash
#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

## Test git perf status functionality

cd "$(mktemp -d)"
git init
create_commit

echo "Test 1: No pending measurements"
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty status"

echo "Test 2: Add measurements and check status"
git perf add -m test-measure 10.0
output=$(git perf status)
assert_output_contains "$output" "1 commit(s)" "Expected 1 commit"
assert_output_contains "$output" "test-measure" "Expected measurement name"

echo "Test 3: Multiple measurements"
git perf add -m another-measure 20.0
output=$(git perf status)
assert_output_contains "$output" "2 unique measurement(s)" "Expected 2 measurements"

echo "Test 4: Detailed output"
create_commit
git perf add -m test-measure 15.0
output=$(git perf status --detailed)
assert_output_contains "$output" "Per-commit breakdown" "Expected detailed view"

echo "Test 5: After push, status should be empty"
# Setup remote
git clone --bare . ../remote.git
git remote add origin ../remote.git
git perf push
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty after push"

echo "All status tests passed!"
```

**File**: `test/test_reset.sh` (NEW FILE)

```bash
#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

## Test git perf reset functionality

cd "$(mktemp -d)"
git init
create_commit

echo "Test 1: Reset with no pending measurements"
output=$(git perf reset --force)
assert_output_contains "$output" "No pending measurements" "Expected empty reset"

echo "Test 2: Dry run"
git perf add -m test-measure 10.0
output=$(git perf reset --dry-run)
assert_output_contains "$output" "Dry run" "Expected dry-run indicator"
# Verify measurements still exist
output=$(git perf status)
assert_output_contains "$output" "test-measure" "Expected measurements to remain"

echo "Test 3: Force reset"
git perf reset --force
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty after reset"

echo "Test 4: Reset doesn't affect pushed measurements"
git perf add -m test-measure 20.0
git clone --bare . ../remote.git
git remote add origin ../remote.git
git perf push

# Add more measurements locally
git perf add -m test-measure 30.0
git perf reset --force

# Check that pushed measurement still accessible
git perf pull
# Verify we can still read the pushed measurement
# (This would require checking the report or audit command)

echo "All reset tests passed!"
```

## Example Usage

### Status Examples

```bash
# Basic status check
$ git perf status
Pending measurements:
  3 commit(s) with measurements
  2 unique measurement(s)

Measurement names:
  - benchmark_parse
  - benchmark_render

(use "git perf reset" to discard pending measurements)
(use "git perf push" to publish measurements)
```

```bash
# Detailed status
$ git perf status --detailed
Pending measurements:
  3 commit(s) with measurements
  2 unique measurement(s)

Measurement names:
  - benchmark_parse
  - benchmark_render

Per-commit breakdown:
  a1b2c3d4e5f6 (3 measurement(s))
    - benchmark_render
    - benchmark_parse
  f6e5d4c3b2a1 (2 measurement(s))
    - benchmark_render
  9876543210ab (1 measurement(s))
    - benchmark_parse

(use "git perf reset" to discard pending measurements)
(use "git perf push" to publish measurements)
```

```bash
# Filter by measurement name
$ git perf status --measurement benchmark_render
Pending measurements:
  2 commit(s) with measurements
  1 unique measurement(s)

Measurement names:
  - benchmark_render

(use "git perf reset" to discard pending measurements)
(use "git perf push" to publish measurements)
```

### Reset Examples

```bash
# Dry run to preview
$ git perf reset --dry-run
Will reset:
  3 write ref(s)
  15 measurement(s)
  5 commit(s) with measurements

Dry run - no changes made.
```

```bash
# Reset with confirmation
$ git perf reset
Will reset:
  3 write ref(s)
  15 measurement(s)
  5 commit(s) with measurements

Are you sure you want to discard these pending measurements? [y/N] y

Reset complete. 3 write ref(s) deleted.
```

```bash
# Force reset (skip confirmation)
$ git perf reset --force
Will reset:
  3 write ref(s)
  15 measurement(s)
  5 commit(s) with measurements

Reset complete. 3 write ref(s) deleted.
```

## Technical Considerations

### Architecture Decisions

1. **Status reads from write refs only**:
   - Don't include remote/published measurements
   - Create consolidated read branch from write refs only
   - This requires new helper function (can't reuse existing)

2. **Reset deletes write refs**:
   - Simple and atomic operation
   - Doesn't modify remote branch
   - Safe because only affects unpushed data
   - Uses existing `remove_reference()` function

3. **Selective reset complexity**:
   - V1: Only support full reset (all pending)
   - Future: Selective reset requires rewriting git notes
   - Rewriting is complex and error-prone
   - Better UX: reset all, then re-add what you want

### Safety Considerations

1. **Never affect published measurements**:
   - Only operate on write refs
   - Don't touch remote branch
   - Don't affect refs/notes/perf-v3

2. **Confirmation prompts**:
   - Default behavior asks for confirmation
   - `--force` flag to skip (for scripts)
   - `--dry-run` to preview safely

3. **Clear error messages**:
   - Explain what will happen
   - Show counts before deletion
   - Provide helpful next steps

### Performance Considerations

1. **Status command**:
   - Needs to walk all commits with pending measurements
   - Could be slow with many commits
   - Acceptable for typical use (< 100 commits)

2. **Reset command**:
   - Just deletes references (very fast)
   - No commit walking required
   - O(n) where n = number of write refs

### Edge Cases

1. **Shallow repository**:
   - Status may fail if walk_commits hits depth limit
   - Document limitation
   - Consider checking for shallow repo

2. **No pending measurements**:
   - Both commands should handle gracefully
   - Clear messages about empty state

3. **Concurrent operations**:
   - Another process might be adding measurements
   - Reset should be atomic (deletes refs in transaction)
   - Status shows point-in-time snapshot

## Compatibility

### Git Version Requirements

- No special Git version requirements
- Uses existing git-perf infrastructure
- Compatible with all supported Git versions

### Backward Compatibility

- No changes to existing commands
- No changes to measurement storage format
- Safe to use alongside existing workflows
- No breaking changes

## Future Enhancements

### Near-term

1. Selective reset by measurement name
   - Requires rewriting git notes
   - Filter measurements, keep some, discard others
2. Reset by commit range
   - Reset only measurements for specific commits
3. JSON output for status
   - Machine-readable format
4. Status integration with git status
   - Show git-perf status in git status output (via hooks)

### Long-term

1. Interactive reset mode
   - Choose which measurements to discard
   - Similar to `git add -p`
2. Measurement stashing
   - Save pending measurements for later
   - Restore stashed measurements
3. Undo reset
   - Keep deleted refs for recovery
   - Time-limited undo window
4. Comparison view
   - Compare local vs remote measurements
   - Show differences

## Success Criteria

- [ ] `status` command shows pending measurements correctly
- [ ] `status` command handles empty state gracefully
- [ ] `status --detailed` shows per-commit breakdown
- [ ] `status --measurement` filters correctly
- [ ] `reset` command deletes write refs correctly
- [ ] `reset` command prompts for confirmation
- [ ] `reset --dry-run` previews without changes
- [ ] `reset --force` skips confirmation
- [ ] Reset never affects published measurements
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation generated and committed
- [ ] Manual testing with various scenarios

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Accidental data loss | User loses measurements | Require confirmation, add --dry-run, clear warnings |
| Confusion about pending vs published | User expectations violated | Clear documentation, helpful output messages |
| Performance with many commits | Slow status command | Document limitation, optimize if needed |
| Complexity of selective reset | Bugs, data corruption | Defer to future version, only support full reset in v1 |
| Concurrent modifications | Inconsistent state | Use atomic ref operations, document behavior |

## Related Work

- **Existing Commands**:
  - `git perf push` - Publishes pending measurements
  - `git perf pull` - Fetches published measurements
  - `git perf list-commits` - Lists all commits with measurements
  - `git perf remove` - Removes published measurements by date
  - `git perf prune` - Removes orphaned measurements

- **Git Analogues**:
  - `git status` - Shows working directory status
  - `git reset` - Resets staging area/working directory
  - `git clean` - Removes untracked files

- **Design Inspiration**:
  - Git's staging area model (index)
  - Clear distinction between local and published state
  - Confirmation prompts for destructive operations

## References

### Codebase References

- `git_perf/src/git/git_interop.rs` - Git operations, write refs
- `git_perf/src/git/git_lowlevel.rs` - Low-level git commands
- `git_perf/src/git/git_definitions.rs` - Reference name constants
- `git_perf/src/serialization.rs` - Measurement serialization
- `test/test_remove.sh` - Example of ref manipulation tests

### Documentation

- Git notes documentation
- Git references documentation
- git-perf architecture (CLAUDE.md)

## Appendix: Alternative Approaches Considered

### Approach 1: Status reads from consolidated branch (local + remote)

**Pros**: Simpler implementation, reuse existing code
**Cons**: Shows all measurements, not just pending
**Decision**: Rejected - users want to see only unpushed changes

### Approach 2: Reset rewrites notes to remove specific measurements

**Pros**: More flexible, supports selective reset
**Cons**: Complex, error-prone, risk of data corruption
**Decision**: Deferred to future version, start with full reset only

### Approach 3: Reset with automatic backup

**Pros**: Safer, allows undo
**Cons**: More complex, adds storage overhead
**Decision**: Deferred to future version, focus on safety through confirmation

### Approach 4: Single command "pending" with subcommands

**Example**: `git perf pending list`, `git perf pending clear`

**Pros**: Groups related functionality
**Cons**: Less familiar than separate status/reset commands
**Decision**: Rejected - separate commands follow git patterns better
