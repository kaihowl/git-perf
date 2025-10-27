# Plan: Add `size` Subcommand to Estimate Measurement Storage

**Status:** Planned
**Created:** 2025-10-26

## Overview

Add a new `size` subcommand to git-perf that estimates the storage size taken by current live measurements in the git repository. This will help users understand the overhead of their performance measurement data stored in git notes and make informed decisions about pruning old measurements.

## Motivation

Currently, git-perf stores performance measurements as git notes in the `refs/notes/perf-v3` branch. As measurement data accumulates over time, users may want to:

- Understand how much storage space their measurements are consuming
- Identify which measurements are taking up the most space
- Make data-driven decisions about when to prune old measurements
- Monitor measurement storage growth over time
- Optimize their measurement strategy based on storage impact

Without a dedicated tool, users must manually inspect git objects or use generic git commands that don't provide measurement-specific insights.

## Goals

1. **Calculate total measurement size** - Sum up all measurement data stored in git notes
2. **Show measurement count** - Display how many commits have measurements
3. **Detailed breakdown** - Optionally show size breakdown by measurement name
4. **Multiple output formats** - Support human-readable and machine-readable formats
5. **Understand git overhead** - Optionally include git object storage overhead
6. **Accurate calculations** - Use standard git methods for reliable size estimation

## Non-Goals (Future Work)

- Tracking size trends over time
- Comparing measurement size with repository size
- Automatic pruning recommendations based on size
- Size filtering by date range or commit range
- Exporting size data as measurements for tracking storage overhead

## Background: Git Object Storage

### How Measurements are Stored

Based on analysis of the git-perf codebase:

1. **Storage Location**: Measurements are stored as git notes in `refs/notes/perf-v3`
2. **Format**: Each measurement is serialized compactly without delimiters:
   - Format: `{epoch}{name}{timestamp}{val}{key=value pairs}`
   - Example: `3Mymeasurement1234567.042.0mykey=myvalue\n`
   - See: `git_perf/src/serialization.rs`
3. **Attachment**: Notes are attached to commit objects via git's notes system
4. **Retrieval**: Uses `git log --notes=refs/notes/perf-v3` to fetch measurements

### Git Object Storage Model

Git stores data as objects in a content-addressable file system:

- **Object Types**: blob, tree, commit, tag (notes are stored as blobs)
- **Storage**: Objects are identified by SHA-1 hashes
- **Optimization**: Git uses "packing" to compress objects into packfiles
- **Two Forms**:
  - **Loose objects**: Individual files in `.git/objects/XX/` directories (fast to create)
  - **Packed objects**: Compressed and delta-compressed in `.git/objects/pack/` (efficient storage)

### Size Calculation Methods (Research-Based)

Based on research of standard git practices, there are several approaches to estimating git object sizes:

#### Method 1: Individual Object Sizes (Traditional)

```bash
# List all notes and get individual sizes
git notes --ref=refs/notes/perf-v3 list | while read note_oid commit_oid; do
  git cat-file -s "$note_oid"
done
```

**Characteristics**:
- Returns logical object size (uncompressed)
- Accurate for individual object measurement
- Does not account for pack compression

#### Method 2: On-Disk Size (More Accurate)

```bash
# Get actual on-disk size including compression
git rev-list --objects --all | \
  git cat-file --batch-check='%(objectname) %(objectsize:disk)'
```

**Characteristics**:
- `%(objectsize)`: Logical size (what `git cat-file -s` reports)
- `%(objectsize:disk)`: Actual bytes on disk (accounts for pack compression)
- More accurate for understanding real disk usage
- **Caveat**: "Care should be taken in drawing conclusions about which refs or objects are responsible for disk usage" due to delta compression

#### Method 3: Modern Disk Usage (Fastest - Git 2.38+)

```bash
# Fast calculation with bitmap index
git rev-list --disk-usage --objects --all --use-bitmap-index
```

**Characteristics**:
- Introduced in Git 2.38 (commit 16950f8384)
- Prints sum of bytes used for on-disk storage
- Much faster than batch-check approach (0.219s vs 6.244s with bitmaps)
- Supports `--human` flag for human-readable output
- Equivalent to piping to `git cat-file --batch-check='%(objectsize:disk)'` but optimized

#### Method 4: Repository Statistics

```bash
git count-objects -v
```

**Output Fields**:
- `count`: Number of loose objects
- `size`: Disk space consumed by loose objects (in KiB)
- `in-pack`: Number of in-pack objects
- `size-pack`: Disk space consumed by packs (in KiB)
- `prune-packable`: Loose objects also present in packs

**Use Case**: Understanding overall repository storage, but not specific to notes

### Recommended Approach for git-perf

For the `size` subcommand, we'll use a **hybrid approach**:

1. **Primary Method**: Individual object size with `git cat-file -s` (Method 1)
   - **Pros**: Simple, widely compatible, accurate for logical size
   - **Cons**: Doesn't account for compression
   - **Rationale**: Gives users a clear understanding of measurement data volume

2. **Optional Enhancement**: On-disk size with `%(objectsize:disk)` (Method 2)
   - **Pros**: More accurate for actual disk usage
   - **Cons**: More complex, harder to attribute to specific measurements
   - **Use**: Optional `--disk-size` flag for advanced users

3. **Optional Context**: Repository statistics with `git count-objects -v` (Method 4)
   - **Pros**: Provides repository-level context
   - **Cons**: Not specific to measurements
   - **Use**: Optional `--include-objects` flag

**Rationale**:
- Start simple with logical sizes (most portable, easiest to understand)
- Provide options for more advanced analysis
- Follow git-perf's philosophy of progressive disclosure

## Design

### 1. CLI Definition

**File**: `cli_types/src/lib.rs`

Add to `Commands` enum:

```rust
/// Estimate storage size of live performance measurements
///
/// This command calculates the total size of performance measurement data
/// stored in git notes (refs/notes/perf-v3). Use --detailed to see a
/// breakdown by measurement name.
///
/// By default, shows logical object sizes (uncompressed). Use --disk-size
/// to see actual on-disk sizes accounting for compression.
///
/// Examples:
///   git perf size                    # Show total size in human-readable format
///   git perf size --detailed         # Show breakdown by measurement name
///   git perf size --format bytes     # Show size in raw bytes
///   git perf size --disk-size        # Show actual on-disk sizes
///   git perf size --include-objects  # Include git repository statistics
#[command(name = "size")]
Size {
    /// Show detailed breakdown by measurement name
    #[arg(short, long)]
    detailed: bool,

    /// Output format (human-readable or bytes)
    #[arg(short, long, value_enum, default_value = "human")]
    format: SizeFormat,

    /// Use on-disk size (compressed) instead of logical size
    #[arg(long)]
    disk_size: bool,

    /// Include git repository statistics for context
    #[arg(long)]
    include_objects: bool,
},
```

Add enum for format:

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SizeFormat {
    /// Human-readable format (e.g., "1.2 MB")
    Human,
    /// Raw bytes as integer
    Bytes,
}
```

**Location**: Around line 390 (after `ListCommits`)

### 2. Size Module Implementation

**File**: `git_perf/src/size.rs` (NEW FILE)

#### Core Data Structures

```rust
use crate::git::git_lowlevel::{capture_git_output};
use crate::serialization::deserialize;
use crate::units::Measurement;
use crate::cli_types::SizeFormat;
use anyhow::{Result, Context};
use std::collections::HashMap;

/// Information about measurement storage size
struct NotesSizeInfo {
    /// Total size in bytes
    total_bytes: u64,
    /// Number of commits with measurements
    note_count: usize,
    /// Optional breakdown by measurement name
    by_measurement: Option<HashMap<String, MeasurementSizeInfo>>,
}

/// Size information for a specific measurement name
struct MeasurementSizeInfo {
    /// Total bytes for this measurement
    total_bytes: u64,
    /// Number of occurrences
    count: usize,
}

/// Git repository statistics from count-objects
struct RepoStats {
    /// Number of loose objects
    loose_objects: u64,
    /// Size of loose objects in bytes
    loose_size: u64,
    /// Number of packed objects
    packed_objects: u64,
    /// Size of pack files in bytes
    pack_size: u64,
}
```

#### Main Entry Point

```rust
/// Calculate and display measurement storage size
pub fn calculate_measurement_size(
    detailed: bool,
    format: SizeFormat,
    disk_size: bool,
    include_objects: bool,
) -> Result<()> {
    // 1. Get notes size information
    let notes_info = get_notes_size(detailed, disk_size)?;

    // 2. Optionally get repository statistics
    let repo_stats = if include_objects {
        Some(get_repo_stats()?)
    } else {
        None
    };

    // 3. Display results
    display_size_report(&notes_info, repo_stats.as_ref(), format)?;

    Ok(())
}
```

#### Size Calculation Functions

```rust
/// Get size information for all measurement notes
fn get_notes_size(detailed: bool, disk_size: bool) -> Result<NotesSizeInfo> {
    // Get list of all notes: "note_oid commit_oid" pairs
    let output = capture_git_output(
        &["notes", "--ref", "refs/notes/perf-v3", "list"],
        &None,
    )?;

    let mut total_bytes = 0u64;
    let mut note_count = 0usize;
    let mut by_measurement = if detailed {
        Some(HashMap::new())
    } else {
        None
    };

    for line in output.stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let note_oid = parts[0];

        // Get size of this note object
        let size = get_object_size(note_oid, disk_size)?;

        total_bytes += size;
        note_count += 1;

        // If detailed breakdown requested, parse measurement names
        if let Some(ref mut by_name) = by_measurement {
            accumulate_measurement_sizes(note_oid, size, by_name)?;
        }
    }

    Ok(NotesSizeInfo {
        total_bytes,
        note_count,
        by_measurement,
    })
}

/// Get size of a git object
fn get_object_size(oid: &str, disk_size: bool) -> Result<u64> {
    if disk_size {
        // Use cat-file --batch-check with objectsize:disk format
        let output = capture_git_output(
            &["cat-file", "--batch-check=%(objectsize:disk)"],
            &Some(format!("{}\n", oid)),
        )?;

        output.stdout.trim()
            .parse::<u64>()
            .context("Failed to parse disk size")
    } else {
        // Use cat-file -s for logical size
        let output = capture_git_output(
            &["cat-file", "-s", oid],
            &None,
        )?;

        output.stdout.trim()
            .parse::<u64>()
            .context("Failed to parse object size")
    }
}

/// Parse note contents and accumulate sizes by measurement name
fn accumulate_measurement_sizes(
    note_oid: &str,
    note_size: u64,
    by_name: &mut HashMap<String, MeasurementSizeInfo>,
) -> Result<()> {
    // Get note content
    let output = capture_git_output(
        &["cat-file", "-p", note_oid],
        &None,
    )?;

    // Parse measurements from note
    let measurements = deserialize(&output.stdout);

    if measurements.is_empty() {
        return Ok(());
    }

    // Distribute note size evenly among measurements in this note
    // (Each measurement contributes roughly equally to the note size)
    let size_per_measurement = note_size / measurements.len() as u64;

    for measurement in measurements {
        let entry = by_name
            .entry(measurement.name.clone())
            .or_insert(MeasurementSizeInfo {
                total_bytes: 0,
                count: 0,
            });

        entry.total_bytes += size_per_measurement;
        entry.count += 1;
    }

    Ok(())
}

/// Get git repository statistics
fn get_repo_stats() -> Result<RepoStats> {
    let output = capture_git_output(&["count-objects", "-v"], &None)?;

    let mut loose_objects = 0;
    let mut loose_size = 0;  // in KiB from git
    let mut packed_objects = 0;
    let mut pack_size = 0;  // in KiB from git

    for line in output.stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0].trim();
        let value = parts[1].trim().parse::<u64>().unwrap_or(0);

        match key {
            "count" => loose_objects = value,
            "size" => loose_size = value,
            "in-pack" => packed_objects = value,
            "size-pack" => pack_size = value,
            _ => {}
        }
    }

    Ok(RepoStats {
        loose_objects,
        loose_size: loose_size * 1024,  // Convert KiB to bytes
        packed_objects,
        pack_size: pack_size * 1024,  // Convert KiB to bytes
    })
}
```

#### Display Functions

```rust
/// Display size report to stdout
fn display_size_report(
    info: &NotesSizeInfo,
    repo_stats: Option<&RepoStats>,
    format: SizeFormat,
) -> Result<()> {
    println!("Live Measurement Size Report");
    println!("============================");
    println!();

    println!("Number of commits with measurements: {}", info.note_count);
    println!("Total measurement data size: {}", format_size(info.total_bytes, format));

    // Show repository context if requested
    if let Some(stats) = repo_stats {
        println!();
        println!("Repository Statistics (for context):");
        println!("-------------------------------------");
        println!("  Loose objects: {} ({})",
            stats.loose_objects,
            format_size(stats.loose_size, format));
        println!("  Packed objects: {} ({})",
            stats.packed_objects,
            format_size(stats.pack_size, format));
        println!("  Total repository size: {}",
            format_size(stats.loose_size + stats.pack_size, format));
    }

    // Show detailed breakdown if requested
    if let Some(by_name) = &info.by_measurement {
        println!();
        println!("Breakdown by Measurement Name:");
        println!("------------------------------");

        // Sort by size descending
        let mut sorted: Vec<_> = by_name.iter().collect();
        sorted.sort_by(|a, b| b.1.total_bytes.cmp(&a.1.total_bytes));

        for (name, size_info) in sorted {
            println!("  {} ({} occurrences): {}",
                name,
                size_info.count,
                format_size(size_info.total_bytes, format));
        }
    }

    Ok(())
}

/// Format size according to requested format
fn format_size(bytes: u64, format: SizeFormat) -> String {
    match format {
        SizeFormat::Bytes => bytes.to_string(),
        SizeFormat::Human => {
            // Use existing DataSize from units.rs
            let measurement = Measurement::DataSize(bytes);
            measurement.to_string()
        }
    }
}
```

### 3. Command Handler

**File**: `git_perf/src/cli.rs`

Add to match statement (around line 115):

```rust
Commands::Size {
    detailed,
    format,
    disk_size,
    include_objects,
} => {
    size::calculate_measurement_size(
        *detailed,
        *format,
        *disk_size,
        *include_objects,
    )?;
}
```

Add module declaration at top of file:

```rust
mod size;
```

### 4. Module Registration

**File**: `git_perf/src/lib.rs`

Ensure the `size` module is declared (if not already included via other module declarations).

## Implementation Phases

### Phase 1: Core Implementation ✓

1. Add CLI definition to `cli_types/src/lib.rs`
   - `Size` command variant with flags
   - `SizeFormat` enum
2. Create `git_perf/src/size.rs` module
   - Core data structures
   - Main entry point
   - Size calculation using `git cat-file -s`
3. Wire up command handler in `git_perf/src/cli.rs`
4. Basic manual testing

### Phase 2: Enhanced Features ✓

1. Implement `--detailed` flag
   - Parse measurement names from notes
   - Accumulate sizes by measurement name
   - Display breakdown table
2. Implement `--disk-size` flag
   - Use `git cat-file --batch-check='%(objectsize:disk)'`
   - Update documentation to explain difference
3. Implement `--include-objects` flag
   - Parse `git count-objects -v` output
   - Display repository statistics

### Phase 3: Documentation ✓

1. Add comprehensive doc comments to CLI definition
2. Run `./scripts/generate-manpages.sh` to regenerate docs
3. Commit regenerated manpages and markdown files
4. Update main README if needed (optional)

### Phase 4: Testing ✓

1. Create `test/test_size.sh` integration test
   - Test empty repository (no measurements)
   - Test with measurements added
   - Test `--detailed` flag
   - Test `--format bytes` flag
   - Test `--disk-size` flag
   - Test `--include-objects` flag
2. Add test to `test/run_tests.sh`
3. Run full test suite: `cargo nextest run -- --skip slow`
4. Fix any failures

### Phase 5: Code Quality ✓

1. Run `cargo fmt` to format code
2. Run `cargo clippy` and address warnings
3. Manual testing with various repository configurations
4. Performance testing with large measurement sets

## Integration Tests

**File**: `test/test_size.sh` (NEW FILE)

```bash
#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git perf size functionality

cd "$(mktemp -d)"
git init

echo "Test 1: Empty repository (no measurements)"
output=$(git perf size 2>&1)
assert_output_contains "$output" "0" "Expected 0 commits with measurements"
assert_output_contains "$output" "Total measurement data size" "Expected size info"

echo "Test 2: Add some measurements to first commit"
create_commit
git perf add -m test-measure-one 10.0
git perf add -m test-measure-two 20.0

echo "Test 3: Add measurements to second commit"
create_commit
git perf add -m test-measure-one 15.0
git perf add -m test-measure-three 25.0

echo "Test 4: Basic size output"
output=$(git perf size)
assert_output_contains "$output" "2" "Expected 2 commits"
assert_output_contains "$output" "measurement" "Expected measurement info"

echo "Test 5: Detailed output shows measurement names"
output=$(git perf size --detailed)
assert_output_contains "$output" "test-measure-one" "Expected measurement breakdown"
assert_output_contains "$output" "test-measure-two" "Expected measurement breakdown"
assert_output_contains "$output" "test-measure-three" "Expected measurement breakdown"
assert_output_contains "$output" "occurrences" "Expected occurrence count"

echo "Test 6: Bytes format shows numeric values"
output=$(git perf size --format bytes)
assert_output_contains "$output" "Total measurement data size:" "Expected size label"
# Should contain numeric bytes value (at least 2 digits)
[[ $output =~ [0-9][0-9]+ ]] || {
    echo "Expected numeric bytes value in output"
    exit 1
}

echo "Test 7: Disk size flag works"
output=$(git perf size --disk-size)
assert_output_contains "$output" "Total measurement data size" "Expected size info"
# Should succeed without errors

echo "Test 8: Include objects flag shows repository stats"
output=$(git perf size --include-objects)
assert_output_contains "$output" "Repository Statistics" "Expected repo stats section"
assert_output_contains "$output" "Packed objects" "Expected packed objects info"

echo "Test 9: Detailed + bytes format combination"
output=$(git perf size --detailed --format bytes)
assert_output_contains "$output" "test-measure-one" "Expected measurement breakdown"
[[ $output =~ [0-9][0-9]+ ]] || {
    echo "Expected numeric bytes in detailed output"
    exit 1
}

echo "Test 10: After pruning old commit"
git reset --hard HEAD~1
git perf prune
output=$(git perf size)
assert_output_contains "$output" "1" "Expected 1 commit after prune"

echo "All size tests passed!"
exit 0
```

Add to `test/run_tests.sh`:

```bash
# Add to list of test scripts
bash "$script_dir/test_size.sh"
```

## Example Usage

### Basic Usage

```bash
$ git perf size
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 15.2 KB
```

### Detailed Breakdown

```bash
$ git perf size --detailed
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 15.2 KB

Breakdown by Measurement Name:
------------------------------
  benchmark_render (28 occurrences): 8.5 KB
  benchmark_parse (25 occurrences): 4.2 KB
  benchmark_compile (18 occurrences): 2.5 KB
```

### Raw Bytes Format

```bash
$ git perf size --format bytes
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 15564
```

### Disk Size (Compressed)

```bash
$ git perf size --disk-size
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 8.3 KB
```

Note: Disk size is typically smaller due to pack compression.

### With Repository Context

```bash
$ git perf size --include-objects
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 15.2 KB

Repository Statistics (for context):
-------------------------------------
  Loose objects: 12 (48 KB)
  Packed objects: 7,726 (1.8 MB)
  Total repository size: 1.8 MB
```

### Combining Flags

```bash
$ git perf size --detailed --disk-size --include-objects
Live Measurement Size Report
============================

Number of commits with measurements: 42
Total measurement data size: 8.3 KB

Repository Statistics (for context):
-------------------------------------
  Loose objects: 12 (48 KB)
  Packed objects: 7,726 (1.8 MB)
  Total repository size: 1.8 MB

Breakdown by Measurement Name:
------------------------------
  benchmark_render (28 occurrences): 4.6 KB
  benchmark_parse (25 occurrences): 2.3 KB
  benchmark_compile (18 occurrences): 1.4 KB
```

## Git Commands Reference

Based on integration tests and research:

| Purpose | Git Command | Output | Notes |
|---------|-------------|--------|-------|
| List all notes | `git notes --ref=refs/notes/perf-v3 list` | `note_oid commit_oid` pairs | One per line |
| Logical object size | `git cat-file -s <oid>` | Size in bytes | Uncompressed size |
| Disk object size | `git cat-file --batch-check='%(objectsize:disk)'` (with OID on stdin) | Size in bytes | Compressed size on disk |
| Object content | `git cat-file -p <oid>` | Object contents | For parsing measurements |
| Repo statistics | `git count-objects -v` | Multi-line stats | `size` and `size-pack` in KiB |
| All objects | `git rev-list --objects --all` | List of OIDs | For comprehensive analysis |
| Disk usage (Git 2.38+) | `git rev-list --disk-usage --objects --all` | Total bytes | Fastest method with bitmaps |

## Technical Considerations

### Size Calculation Accuracy

1. **Logical Size** (`git cat-file -s`):
   - **Pros**: Simple, portable, represents actual data volume
   - **Cons**: Doesn't account for compression or delta compression
   - **Use**: Default mode, easiest to understand

2. **Disk Size** (`%(objectsize:disk)`):
   - **Pros**: Accurate for actual disk usage
   - **Cons**: Can be misleading due to delta compression (base vs delta is arbitrary)
   - **Use**: Optional `--disk-size` flag for advanced users
   - **Caveat**: Git documentation warns about drawing conclusions from disk sizes

3. **Per-Measurement Attribution**:
   - Since a single note can contain multiple measurements, we distribute the note size evenly
   - This is an approximation but reasonable for typical use cases
   - Alternative: Calculate serialized size per measurement (more accurate but slower)

### Performance Considerations

1. **Scalability**:
   - One `git cat-file` call per note object
   - For 1000 commits, this is 1000 git invocations
   - Could optimize with `--batch` mode for large repos (future enhancement)

2. **Memory Usage**:
   - Detailed mode parses all measurement names into memory
   - Reasonable for typical repos (thousands of measurements)
   - Could stream for very large repos (future optimization)

3. **Git Compatibility**:
   - Basic commands work with all Git versions
   - `--disk-size` with `%(objectsize:disk)` requires Git 2.13+
   - `git rev-list --disk-usage` requires Git 2.38+ (not used in v1)

### Error Handling

1. **No measurements**: Return size of 0, count of 0
2. **Invalid objects**: Skip and warn (shouldn't happen in normal use)
3. **Git errors**: Propagate with context
4. **Parse errors**: Use existing `deserialize()` error handling

## Compatibility

### Git Version Requirements

- **Minimum**: Git 2.3+ (for git notes)
- **Recommended**: Git 2.13+ (for `%(objectsize:disk)`)
- **Enhanced**: Git 2.38+ (for `--disk-usage` in future versions)

Current implementation targets Git 2.13+ for disk size support.

### Backward Compatibility

- No changes to measurement storage format
- No changes to existing commands
- New command, no risk of breaking existing workflows

## Future Enhancements

### Near-term
1. Batch mode for better performance with large repos
2. JSON output format for scripting
3. Size filtering by date range (`--since`, `--until`)
4. Size per commit (histogram)

### Long-term
1. Track size trends over time (store as measurements)
2. Pruning recommendations based on size analysis
3. Size comparison before/after operations
4. Integration with audit system for size-based alerts
5. Export size data as measurements for tracking

## Success Criteria

- [ ] Command successfully reports size for repositories with measurements
- [ ] Command reports 0 for repositories without measurements
- [ ] Detailed breakdown accurately attributes sizes to measurement names
- [ ] Bytes format outputs parseable integer values
- [ ] Disk-size flag provides compressed size information
- [ ] Include-objects flag provides repository context
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted with `cargo fmt`
- [ ] Documentation generated and committed
- [ ] Manual testing with various repository sizes

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Performance with large repos | Slow execution | Document limitations, add batch mode in future |
| Inaccurate size attribution | Misleading breakdown | Document approximation method, provide caveats |
| Git version compatibility | Disk-size fails on old Git | Feature-detect and fallback gracefully |
| Compressed size confusion | User misunderstanding | Clear documentation, separate flag |

## Related Work

- **Existing Commands**:
  - `git perf list-commits` - Lists commits with measurements
  - `git perf prune` - Removes orphaned measurements
  - `git perf remove --older-than` - Removes old measurements

- **Complementary Tools**:
  - `git-sizer` - Analyzes repository size (external tool)
  - `git count-objects` - Repository statistics

- **Standards**:
  - Git object model and storage
  - Git notes system
  - Conventional git size reporting

## References

### Git Documentation
- `git-notes(1)` - Git notes documentation
- `git-cat-file(1)` - Object inspection
- `git-count-objects(1)` - Repository statistics
- `git-rev-list(1)` - List objects

### Research Sources
- Stack Overflow: "Find size of Git repository"
- Git commit 16950f8384: `git rev-list --disk-usage` feature
- Git official documentation on object storage
- GitHub blog: "Git's database internals"

### Codebase References
- `test/test_remove.sh` - Examples of git object commands
- `test/test_prune.sh` - Git notes list usage
- `git_perf/src/git/git_interop.rs` - Git wrapper functions
- `git_perf/src/serialization.rs` - Measurement serialization
- `git_perf/src/units.rs` - Size formatting

## Appendix: Alternative Approaches Considered

### Approach 1: Use git rev-list --disk-usage
**Pros**: Fastest, modern git feature
**Cons**: Requires Git 2.38+, doesn't provide per-measurement breakdown
**Decision**: Keep for future v2, use compatible approach for v1

### Approach 2: Parse pack files directly
**Pros**: Most accurate
**Cons**: Complex, fragile, implementation-dependent
**Decision**: Rejected, use git commands instead

### Approach 3: Store size metadata with measurements
**Pros**: Instant retrieval
**Cons**: Increases storage, requires migration
**Decision**: Rejected, calculate on-demand instead
