# Implementation Plan: Extend walk_commits Output with Commit Title and Author

**Issue:** #526 - Hover over git hash shows details of commit
**Branch:** `terragon/extend-walk-commits-output-pvmq26`

## Overview

Extend the `walk_commits` function to capture and store commit title and author information efficiently. This metadata will be used to provide rich hover tooltips in Plotly HTML reports, showing commit details when hovering over data points.

## Current State Analysis

### Data Flow
1. **Low-level Git Interop** (`git/git_interop.rs:895-952`):
   - `walk_commits_from()` returns `Vec<(String, Vec<String>)>` (commit SHA + raw note lines)
   - Uses git format: `--pretty=--,%H,%D%n%N`
   - Only captures: commit hash, decorations (for shallow detection), and notes

2. **High-level Retrieval** (`measurement_retrieval.rs:115-131`):
   - Wraps low-level function, deserializes measurements
   - Returns `Iterator<Item = Result<Commit>>`

3. **Data Structures** (`data.rs:26-29`):
   ```rust
   pub struct Commit {
       pub commit: String,  // 40-char SHA
       pub measurements: Vec<MeasurementData>,
   }
   ```

4. **Reporting** (`reporting.rs:734-746`):
   - Currently displays only shortened commit hashes (7 chars)
   - No hover information available
   - Plotly trace customization not yet using `customdata` or `hovertemplate`

## Design Decision: Efficient String Storage

### Problem
Each measurement is associated with a commit, but commit metadata (title, author) is identical for all measurements within the same commit. Naively storing strings on each measurement would duplicate data unnecessarily.

### Solution: String Interning Pattern

Use Rust's `Arc<str>` (Atomic Reference Counted string) to share immutable string data:

```rust
use std::sync::Arc;

pub struct Commit {
    pub commit: String,
    pub title: Arc<str>,      // Shared immutable string
    pub author: Arc<str>,     // Shared immutable string
    pub measurements: Vec<MeasurementData>,
}
```

**Benefits:**
- **Zero-copy sharing**: All measurements reference the same string allocation
- **Memory efficient**: Single allocation per commit, regardless of measurement count
- **Thread-safe**: `Arc` is thread-safe (important for concurrent reporting)
- **Cheap cloning**: Cloning only increments reference count (no data copy)
- **Immutable**: Strings cannot be modified, ensuring data integrity

**Alternative considered and rejected:**
- `Rc<str>` - Not thread-safe, incompatible with parallel processing
- `&'static str` - Requires leaked memory or compile-time strings
- `String` per measurement - Wastes memory through duplication
- Indexes into lookup table - Complex, error-prone, no performance benefit

## Implementation Steps

### Step 1: Update Git Log Format (git_interop.rs)

**File:** `git_perf/src/git/git_interop.rs:915`

**Current format:**
```rust
"--pretty=--,%H,%D%n%N"
```

**New format:**
```rust
"--pretty=--,%H,%s,%an,%D%n%N"
```

**Format codes:**
- `%H` - Full commit hash (40 hex chars)
- `%s` - Subject (first line of commit message)
- `%an` - Author name
- `%D` - Ref decorations (existing, for shallow detection)
- `%n%N` - Newline + notes (existing)

**Example output:**
```
--,fcafed6...,test(bash_tests): make assertions specific,John Doe,HEAD -> branch
1test1234123
--,cf84239...,test: clean up git perf asserts,Jane Smith,
3test9999999
```

### Step 2: Update Return Type (git_interop.rs)

**File:** `git_perf/src/git/git_interop.rs:895-898`

**Current signature:**
```rust
pub fn walk_commits_from(
    start_commit: &str,
    num_commits: usize,
) -> Result<Vec<(String, Vec<String>)>>
```

**New signature:**
```rust
pub fn walk_commits_from(
    start_commit: &str,
    num_commits: usize,
) -> Result<Vec<CommitWithNotes>>

pub struct CommitWithNotes {
    pub sha: String,
    pub title: String,
    pub author: String,
    pub note_lines: Vec<String>,
}
```

**Rationale:** Struct is clearer than 4-tuple, easier to extend, self-documenting.

### Step 3: Update Parsing Logic (git_interop.rs)

**File:** `git_perf/src/git/git_interop.rs:928-945`

**Current parsing:**
```rust
if l.starts_with("--") {
    let info = l.split(',').collect_vec();
    let commit_hash = info.get(1).expect("...");
    detected_shallow |= info[2..].contains(&"grafted");
    current_commit = Some(commit_hash.to_string());
    commits.push((commit_hash.to_string(), Vec::new()));
}
```

**New parsing:**
```rust
if l.starts_with("--") {
    let parts: Vec<&str> = l.splitn(5, ',').collect();
    if parts.len() < 5 {
        bail!("Invalid git log format: expected 5 fields, got {}", parts.len());
    }

    let sha = parts[1].to_string();
    let title = parts[2].to_string();
    let author = parts[3].to_string();
    let decorations = parts[4];

    detected_shallow |= decorations.contains("grafted");
    current_commit_sha = Some(sha.clone());

    commits.push(CommitWithNotes {
        sha,
        title,
        author,
        note_lines: Vec::new(),
    });
}
```

**Edge cases to handle:**
- Empty commit messages (use "[no subject]")
- Commit messages containing commas (use `splitn(5, ',')` to limit splits)
- Empty author names (use "[unknown]")

### Step 4: Update Commit Data Structure (data.rs)

**File:** `git_perf/src/data.rs:26-29`

**Current:**
```rust
#[derive(Debug, PartialEq, Clone)]
pub struct Commit {
    pub commit: String,
    pub measurements: Vec<MeasurementData>,
}
```

**New:**
```rust
use std::sync::Arc;

#[derive(Debug, PartialEq, Clone)]
pub struct Commit {
    pub commit: String,
    pub title: Arc<str>,
    pub author: Arc<str>,
    pub measurements: Vec<MeasurementData>,
}
```

**Update `PartialEq` implementation:**
- Automatic `PartialEq` derivation works correctly with `Arc<str>` (compares contents)
- No custom implementation needed

### Step 5: Update High-Level API (measurement_retrieval.rs)

**File:** `git_perf/src/measurement_retrieval.rs:115-131`

**Current:**
```rust
pub fn walk_commits_from(
    start_commit: &str,
    num_commits: usize,
) -> Result<impl Iterator<Item = Result<Commit>>> {
    let raw_commits = git_interop::walk_commits_from(start_commit, num_commits)?;
    Ok(raw_commits.into_iter().map(|(commit, note_lines)| {
        let measurements = deserialize_measurements(&note_lines)?;
        Ok(Commit { commit, measurements })
    }))
}
```

**New:**
```rust
pub fn walk_commits_from(
    start_commit: &str,
    num_commits: usize,
) -> Result<impl Iterator<Item = Result<Commit>>> {
    let raw_commits = git_interop::walk_commits_from(start_commit, num_commits)?;
    Ok(raw_commits.into_iter().map(|commit_data| {
        let measurements = deserialize_measurements(&commit_data.note_lines)?;
        Ok(Commit {
            commit: commit_data.sha,
            title: Arc::from(commit_data.title.as_str()),
            author: Arc::from(commit_data.author.as_str()),
            measurements,
        })
    }))
}
```

### Step 6: Add Hover Data to Plotly Reports (reporting.rs)

**File:** `git_perf/src/reporting.rs`

**Changes needed:**

1. **Store full commit metadata** in `HtmlReporter` (around line 59):
   ```rust
   pub struct HtmlReporter {
       all_commits: Vec<Commit>,  // Already stores Commit, will now have metadata
       // ... rest of fields
   }
   ```

2. **Prepare hover data arrays** when building plots (new helper method):
   ```rust
   fn prepare_hover_data(&self, measurement_commits: &[&Commit]) -> Vec<String> {
       measurement_commits
           .iter()
           .map(|c| format!(
               "Commit: {}<br>Author: {}<br>Title: {}",
               &c.commit[..7],  // Short hash
               c.author,
               c.title
           ))
           .collect()
   }
   ```

3. **Update trace creation** (around line 650-700):
   ```rust
   let hover_texts = self.prepare_hover_data(&commits_for_this_measurement);

   let trace = Scatter::new(x_values, y_values)
       .name(&measurement_name)
       .mode(Mode::LinesMarkers)
       .hover_text_array(hover_texts)  // Add hover data
       .hover_info(HoverInfo::Text);   // Use custom text
   ```

**Plotly hover format example:**
```
Commit: fcafed6
Author: John Doe
Title: test(bash_tests): make assertions specific
```

### Step 7: Update Tests

**Files to update with new data structure:**

1. **Unit tests** in `data.rs`:
   - Update test fixtures to include `title` and `author` fields
   - Example:
     ```rust
     let commit = Commit {
         commit: "abc123".to_string(),
         title: Arc::from("test: example commit"),
         author: Arc::from("Test Author"),
         measurements: vec![],
     };
     ```

2. **Integration tests** in `measurement_retrieval.rs`:
   - Mock `CommitWithNotes` instead of tuples
   - Verify metadata is correctly propagated

3. **Bash integration tests** (if any check output format):
   - Review tests in `/test/` directory
   - Check if any tests parse commit output directly
   - Update expectations if needed (unlikely - tests focus on measurements)

### Step 8: Update Documentation

**Files to update:**

1. **CHANGELOG.md** (or create entry):
   ```markdown
   ### Added
   - Commit title and author information now displayed in Plotly report hover tooltips (#526)
   ```

2. **CLAUDE.md** (architecture section):
   - Update data structure documentation
   - Note the use of `Arc<str>` for efficient string storage

3. **Code comments**:
   - Add doc comments to `CommitWithNotes` struct
   - Document the Arc string interning pattern

## Testing Strategy

### Unit Tests
- Test `CommitWithNotes` parsing with various edge cases:
  - Empty commit messages
  - Commit messages with commas
  - Empty author names
  - Unicode characters in titles/authors

### Integration Tests
- Verify metadata propagates through the full pipeline:
  - Low-level git interop → high-level API → reporting
- Test that `Arc<str>` cloning works correctly (reference counting)

### Manual Testing
1. Generate report with `git-perf report --html`
2. Open in browser
3. Hover over data points
4. Verify tooltip shows: commit hash (short), author name, commit title

### Performance Testing
- Benchmark memory usage with/without Arc (should be significantly lower)
- Verify no performance regression in report generation

## Rollback Plan

If issues arise:
1. Changes are localized to specific modules
2. Can revert to returning just commit SHA
3. Backward compatible - existing git notes unchanged
4. No migration needed for stored data

## Success Criteria

- [ ] Commit title and author captured from git log
- [ ] Data stored efficiently using `Arc<str>` pattern
- [ ] Hover tooltips display commit metadata in HTML reports
- [ ] All tests pass (unit, integration, bash)
- [ ] No performance regression
- [ ] Memory usage reduced compared to naive string duplication
- [ ] Code formatted with `cargo fmt`
- [ ] No warnings from `cargo clippy`
- [ ] Documentation updated

## Timeline Estimate

**Total: Single session (2-3 hours of agent work)**

- Step 1-3 (Git interop): 30 minutes
- Step 4-5 (Data structures): 20 minutes
- Step 6 (Reporting): 45 minutes
- Step 7 (Tests): 30 minutes
- Step 8 (Docs): 15 minutes
- Testing & iteration: 30 minutes

## Open Questions

1. **Commit message truncation**: Should we truncate long commit titles in hover text?
   - Recommendation: Yes, limit to ~80 chars with ellipsis
   - Implementation: Add `.chars().take(80).collect()` in hover formatting

2. **Additional metadata**: Should we also capture commit date?
   - Current plan: No, to keep changes minimal
   - Can be added later if requested (format: `%ct` for Unix timestamp)

3. **CSV export**: Should commit metadata be added to CSV output?
   - Current plan: No, CSV is for measurements only
   - Commit hash is sufficient for external joins

4. **Backward compatibility**: What about old git-perf versions reading new data?
   - No issue: Commit metadata not stored in git notes (only used in memory)
   - Git notes format unchanged

## References

- **Issue #526**: "Hover over git hash shows details of commit"
- **Git log formats**: `man git-log` (search for "PRETTY FORMATS")
- **Plotly hover docs**: https://plotly.com/javascript/hover-text-and-formatting/
- **Rust Arc docs**: https://doc.rust-lang.org/std/sync/struct.Arc.html
