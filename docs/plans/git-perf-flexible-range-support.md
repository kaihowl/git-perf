# Implementation Plan: Flexible Date and Range Support for git-perf

## Overview

Extend `git-perf` to support more flexible commit range specifications, similar to `git log`. Currently, both `git-perf audit` and `git-perf report` only support `-n/--max-count` to limit the number of commits examined. This plan adds support for date-based filtering (`--since`, `--until`) and flexible range specifications for both commands.

**Key Insight**: The `report` command can easily adopt the same range options with clean semantics - the range of reported commits would simply adapt with the given range. For multi-section reports using templates, the `depth` parameter needs enhanced parsing to support time-based cutoffs (days/hours/minutes) similar to how the `remove` command parses cutoff dates.

## Current State

### Existing Implementation

- **Repository**: `kaihowl/git-perf` (Rust-based external tool)
- **Current flag**: `-n, --max-count <MAX_COUNT>` (default: 40)
- **Current behavior**: Examines the last N commits from HEAD using `--first-parent`
- **Git invocation**:
  ```bash
  git --no-pager log --no-color --ignore-missing -n <num_commits> \
    --first-parent --pretty=--,%H,%s,%an,%D%n%N --decorate=full \
    --notes=<temp_ref> <resolved_commit>
  ```

### Usage in dotfiles repository

- **Script**: `/root/repo/script/ci.sh:68`
  ```bash
  git perf audit -n 40 -m nvim -m zsh -m ci -m test -m nix-closure-size \
    -s "os=$os" --min-measurements 10
  ```

### Key Files (in git-perf repo)

#### Common Infrastructure
- `cli_types/src/lib.rs` - Shared CLI type definitions including `CliReportHistory`
- `git_perf/src/cli.rs` - Command-line argument parsing and dispatch
- `git_perf/src/measurement_retrieval.rs` - Commit walking wrapper
- `git_perf/src/git/git_interop.rs` - Git command invocation

#### Audit-Specific
- `git_perf/src/audit.rs` - Audit subcommand implementation

#### Report-Specific
- `git_perf/src/reporting.rs` - Report generation and rendering (HTML/CSV)
- `git_perf/src/reporting_config.rs` - Template parsing and section configuration
- `docs/dashboard-templates.md` - Template documentation and syntax guide

## Proposed Changes

### 1. New Command-Line Options

Add the following options to both `git perf audit` and `git perf report` commands:

#### Date-Based Options

- `--since <date>` / `--after <date>`
  - Include commits more recent than specific date
  - Examples: `--since="2 weeks ago"`, `--since="2025-01-01"`, `--since="last monday"`
  - Accepts all formats that `git log --since` accepts

- `--until <date>` / `--before <date>`
  - Include commits older than specific date
  - Examples: `--until="yesterday"`, `--until="2025-12-31"`
  - Accepts all formats that `git log --until` accepts

#### Range Options

- `<revision-range>`
  - Support git revision range syntax as positional argument
  - Examples:
    - `main..feature` - commits in feature but not in main
    - `main...feature` - commits in either but not both (symmetric difference)
    - `HEAD~10..HEAD` - last 10 commits
    - `v1.0..v2.0` - commits between two tags

### 2. Report Command Specifics

The `git perf report` command shares the same commit retrieval infrastructure as `audit`, making it straightforward to adopt the same range options. However, the report command has unique considerations due to its multi-section templating functionality.

#### Current Report Behavior

- **Single-section reports**: Use CLI flags for filtering, aggregation, and depth (`-n`)
- **Multi-section reports**: Define multiple `{{SECTION[id]}}` blocks in HTML templates, each with its own configuration
- **Section depth parameter**: Each section can override the global `-n` flag with a section-specific `depth: N` parameter

#### Template Depth Parameter Enhancement

Currently, the `depth` parameter in templates only accepts integer commit counts:
```html
{{SECTION[build-times]
    measurement-filter: ^test::
    depth: 20
}}
```

**Proposed Enhancement**: Parse `depth` to support time-based specifications similar to `git perf remove --older-than`:

```html
{{SECTION[recent-activity]
    measurement-filter: ^test::
    depth: 7d          # Last 7 days
}}

{{SECTION[hourly-performance]
    measurement-filter: ^bench::
    depth: 48h         # Last 48 hours
}}

{{SECTION[detailed-history]
    measurement-filter: ^integration::
    depth: 100         # Last 100 commits (existing behavior)
}}
```

**Implementation Reference**: The `remove` command already implements time-based parsing in `cli_types/src/lib.rs`:
```rust
fn parse_datetime_value(now: &DateTime<Utc>, input: &str) -> Result<DateTime<Utc>> {
    // Supports: "2w" (weeks), "30d" (days), "72h" (hours), "15m" (minutes)
    let (num, unit) = input.split_at(input.len() - 1);
    let num: i64 = num.parse()?;
    let subtractor = match unit {
        "w" => Duration::weeks(num),
        "d" => Duration::days(num),
        "h" => Duration::hours(num),
        _ => bail!("Unsupported datetime format"),
    };
    Ok(*now - subtractor)
}
```

**Adaptation for depth parameter**:
1. Parse `depth` value as either integer (commit count) or time-based string (e.g., `7d`)
2. If time-based, convert to `--since` parameter when retrieving commits for that section
3. Maintain backward compatibility with integer `depth` values

#### Range Semantics Comparison

**Audit Command**:
- The **latest commit** in the specified range is the "head" commit
- The "head" is compared against the "tail" (all other commits in the range)
- Statistical analysis compares head value vs. tail distribution
- This applies to all range specifications (`--since`, `--until`, revision ranges, `-n`)

Example:
```bash
# With range v1.0..v2.0, the latest commit reachable from v2.0
# (but not from v1.0) is the "head"
git perf audit v1.0..v2.0 -m nvim

# With --since="1 week ago", HEAD is the "head" commit
# (assuming HEAD is within the last week)
git perf audit --since="1 week ago" -m nvim
```

**Report Command**:
- **All commits** in the range are visualized equally
- No head/tail distinction - all measurements are plotted
- Range simply defines the visualization scope
- Multi-section templates can have different ranges per section

Example:
```bash
# All commits from v1.0 to v2.0 are plotted
git perf report v1.0..v2.0 -o report.html

# All commits from the last week are plotted
git perf report --since="1 week ago" -o report.html
```

**Template with Mixed Depth Types**:

`dashboard.html` template:
```html
{{SECTION[recent]
    depth: 7d        # Last week
}}
{{SECTION[monthly]
    depth: 30d       # Last month
}}
{{SECTION[all-time]
    depth: 1000      # Last 1000 commits
}}
```

Usage:
```bash
git perf report -t dashboard.html -o report.html
```

This creates a report with three sections, each showing a different time window or commit count.

### 3. Date Parsing Implementation Options

**Problem Statement**: Supporting time-based `depth:` specifications in report templates (e.g., `depth: 7d`) introduces a date parsing challenge. We need to parse datetime values to filter measurement ranges, but this requires implementing date parsing logic that may differ from the CLI's `--since`/`--until` options (which are handled directly by git). This creates a potential mismatch where templates and CLI accept different date formats, or we duplicate parsing logic in multiple places.

The key question: **Should we fetch measurements once and filter in-memory, or make multiple git calls per section?**

This section evaluates five implementation approaches with different trade-offs for usability, performance, and implementation complexity.

#### Option 1: Unified DepthSpec with Git-Delegated Parsing (Recommended)

**Approach**: Create a `DepthSpec` enum supporting both commit counts and date-based specifications, but delegate ALL date parsing to git at runtime rather than parsing dates in Rust. Filter measurements in-memory after a single fetch.

**Implementation**:
```rust
// cli_types/src/lib.rs
#[derive(Debug, Clone, PartialEq)]
pub enum DepthSpec {
    CommitCount(usize),
    TimeBased(String),  // Store unparsed string, convert to git args later
}

impl DepthSpec {
    pub fn parse(input: &str) -> Result<Self> {
        // Try parsing as integer first
        if let Ok(count) = input.parse::<usize>() {
            return Ok(DepthSpec::CommitCount(count));
        }

        // Otherwise treat as time-based (validate format loosely)
        if input.ends_with(|c: char| matches!(c, 'w' | 'd' | 'h' | 'm')) {
            Ok(DepthSpec::TimeBased(input.to_string()))
        } else {
            bail!("Invalid depth format: {}. Expected integer or time format (e.g., 7d, 48h)", input)
        }
    }
}

// Convert "7d" -> "7 days ago" for git --since
fn format_for_git(spec: &str) -> String {
    let (num, unit) = spec.split_at(spec.len() - 1);
    let unit_name = match unit {
        "w" => "weeks",
        "d" => "days",
        "h" => "hours",
        "m" => "minutes",
        _ => unreachable!(),
    };
    format!("{} {} ago", num, unit_name)
}
```

**Report generation strategy**:
```rust
// 1. Determine maximum depth needed across all sections
let max_depth = calculate_global_depth(&sections, &cli_args);

// 2. Fetch commits ONCE (include timestamps in commit data)
let all_commits = walk_commits_with_depth("HEAD", max_depth)?;

// 3. Each section filters in-memory (no additional git calls)
for section in sections {
    let section_commits = section.depth
        .as_ref()
        .map(|d| filter_commits_by_depth(&all_commits, d))
        .unwrap_or(&all_commits);

    render_section(section_commits, section.filters);
}
```

**Required changes**:
1. Update git log format to include timestamp: `--pretty=--,%H,%s,%an,%ct,%D%n%N` (add `%ct`)
2. Add `timestamp: DateTime<Utc>` field to `Commit` struct
3. Implement `calculate_global_depth()` to find maximum range across sections
4. Implement in-memory filtering based on timestamps

**Benefits**:
- ✅ **Single git invocation** - measurements loaded once, optimal performance
- ✅ **Minimal parsing logic** - just format conversion, no datetime math
- ✅ **Leverages git's robust date parsing** - for CLI `--since`/`--until`
- ✅ **Consistent format** - templates and CLI use same time specifications (`7d`, `48h`)
- ✅ **Simple validation** - basic format check, let git handle the rest
- ✅ **No new dependencies** - uses existing chrono for timestamps

**Drawbacks**:
- ⚠️ Limited to simple relative time formats in templates (can't use "last monday")
- ⚠️ Must fetch enough commits to satisfy all sections (slight over-fetch possible)
- ⚠️ Requires adding timestamp field to `Commit` struct (minor schema change)

**Use cases**: Ideal for most scenarios - balances performance and implementation simplicity.

---

#### Option 2: Reuse Existing parse_datetime_value() with In-Memory Filtering

**Approach**: Make the existing `parse_datetime_value()` function (from the `remove` command) public and reuse it in template parsing. Fetch commits once based on global maximum depth, then filter in-memory per section.

**Implementation**:
```rust
// cli_types/src/lib.rs - make public
pub fn parse_datetime_value(now: &DateTime<Utc>, input: &str) -> Result<DateTime<Utc>> {
    // ... existing implementation supporting "2w", "30d", "72h", "15m"
}

// reporting_config.rs
#[derive(Debug, Clone)]
pub enum DepthSpec {
    CommitCount(usize),
    Since(DateTime<Utc>),  // Pre-computed at template load time
}

fn parse_depth(value: &str) -> Result<DepthSpec> {
    if let Ok(count) = value.parse::<usize>() {
        return Ok(DepthSpec::CommitCount(count));
    }

    let now = Utc::now();
    let datetime = parse_datetime_value(&now, value)?;
    Ok(DepthSpec::Since(datetime))
}

impl DepthSpec {
    // Filter commits in-memory (no git call)
    pub fn filter_commits<'a>(&self, commits: &'a [Commit]) -> &'a [Commit] {
        match self {
            DepthSpec::CommitCount(n) => &commits[..(*n).min(commits.len())],
            DepthSpec::Since(cutoff) => {
                let end = commits.iter()
                    .take_while(|c| c.timestamp >= *cutoff)
                    .count();
                &commits[..end]
            }
        }
    }
}
```

**Report generation**: Same as Option 1 - single fetch, in-memory filtering per section.

**Benefits**:
- ✅ **Single git invocation** - optimal performance
- ✅ **Reuses existing code** - no duplication, uses battle-tested parser
- ✅ **Consistent with `remove` command** - same date format
- ✅ **Early validation** - parse errors caught at template load time
- ✅ **Type-safe** - dates are validated Rust `DateTime` types
- ✅ **In-memory filtering** - fast, no I/O per section

**Drawbacks**:
- ⚠️ Adds public API surface to `cli_types` crate
- ⚠️ Still maintains separate parsing from CLI `--since`/`--until` (which git handles)
- ⚠️ Limited formats - only supports `Nw`, `Nd`, `Nh`, `Nm` (not "last monday")
- ⚠️ Requires timestamp field in `Commit` struct

**Use cases**: Good choice if you want to reuse existing code and maintain consistency with the `remove` command's date format.

---

#### Option 3: Rich Date Format Support with chrono-english

**Approach**: Add a dependency like `chrono-english` to support git-like natural language dates everywhere ("last monday", "2 weeks ago", ISO dates, etc.). Single fetch with in-memory filtering.

**Implementation**:
```rust
// Add to Cargo.toml: chrono-english = "0.1"

use chrono_english::{parse_date_string, Dialect};

#[derive(Debug, Clone)]
pub enum DepthSpec {
    CommitCount(usize),
    Since(DateTime<Utc>),
}

impl DepthSpec {
    pub fn parse(input: &str) -> Result<Self> {
        if let Ok(count) = input.parse::<usize>() {
            return Ok(DepthSpec::CommitCount(count));
        }

        // Parse natural language dates
        let datetime = parse_date_string(input, Utc::now(), Dialect::Uk)?;
        Ok(DepthSpec::Since(datetime))
    }
}
```

**Report generation**: Same as Options 1 & 2 - single fetch, in-memory filtering.

**Benefits**:
- ✅ **Richest format support** - "last monday", "2 weeks ago", "2025-01-01", ISO dates
- ✅ **Consistent UX** - templates accept same formats as CLI could support
- ✅ **User-friendly** - natural language dates in templates
- ✅ **Single fetch** - optimal performance

**Drawbacks**:
- ❌ **New dependency** - adds external crate, increases binary size
- ❌ **Increased complexity** - more parsing logic, edge cases
- ❌ **Maintenance burden** - dependency updates, compatibility issues
- ⚠️ Might parse differently than git (compatibility risk)
- ⚠️ Requires timestamp field in `Commit` struct

**Use cases**: Only if you need rich natural language date support in templates, which is uncommon for version-controlled template files (usually use predictable formats like "7d").

---

#### Option 4: Hybrid Approach - CLI Delegates, Templates Parse Simply

**Approach**: Keep CLI date parsing fully delegated to git (pass-through strings), but implement limited date parsing for templates only. Single fetch with in-memory filtering.

**Implementation**:
```rust
// For CLI: just accept strings, pass to git
pub struct CliReportHistory {
    pub since: Option<String>,  // Pass directly to git --since
    pub until: Option<String>,  // Pass directly to git --until
}

// For templates: parse simple formats only
#[derive(Debug, Clone)]
pub enum DepthSpec {
    CommitCount(usize),
    RelativeTime { num: i64, unit: TimeUnit },
}

pub enum TimeUnit { Weeks, Days, Hours, Minutes }

impl DepthSpec {
    pub fn to_since_datetime(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            DepthSpec::CommitCount(_) => None,
            DepthSpec::RelativeTime { num, unit } => {
                let duration = match unit {
                    TimeUnit::Weeks => Duration::weeks(*num),
                    TimeUnit::Days => Duration::days(*num),
                    TimeUnit::Hours => Duration::hours(*num),
                    TimeUnit::Minutes => Duration::minutes(*num),
                };
                Some(now - duration)
            }
        }
    }
}
```

**Report generation**: Same as Options 1-3 - single fetch, in-memory filtering.

**Benefits**:
- ✅ **Separation of concerns** - CLI and template parsing optimized independently
- ✅ **No CLI complexity** - git handles all CLI date formats natively
- ✅ **Simple template parsing** - only basic formats needed for templates
- ✅ **No new dependencies**
- ✅ **Single fetch** - optimal performance

**Drawbacks**:
- ⚠️ **Inconsistent formats** between CLI and templates (could confuse users)
- ⚠️ Two parsing implementations to maintain
- ⚠️ Requires timestamp field in `Commit` struct

**Use cases**: Good compromise if you want full git date support for CLI but don't need it in templates.

---

#### Option 5: Multi-Fetch Strategy (Independent Git Calls Per Section)

**Approach**: Accept the cost of multiple git calls - each section independently fetches its own commits and measurements. Optimize with caching where sections share the same depth.

**Implementation**:
```rust
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DepthSpec {
    CommitCount(usize),
    TimeBased(String),  // "7d", "48h", etc.
}

impl DepthSpec {
    pub fn to_git_args(&self) -> Vec<String> {
        match self {
            DepthSpec::CommitCount(n) => vec!["-n".to_string(), n.to_string()],
            DepthSpec::TimeBased(spec) => {
                vec!["--since".to_string(), format_for_git_since(spec)]
            }
        }
    }
}

// Report generation with caching
pub fn generate_multi_section_report_cached(
    sections: &[SectionConfig],
    base_commit: &str,
) -> Result<String> {
    // Cache: DepthSpec -> Vec<Commit>
    let mut commit_cache: HashMap<DepthSpec, Vec<Commit>> = HashMap::new();
    let mut section_data = Vec::new();

    for section in sections {
        let depth = section.depth.as_ref()
            .unwrap_or(&DepthSpec::CommitCount(40));

        // Check cache first
        let commits = if let Some(cached) = commit_cache.get(depth) {
            cached.clone()  // Reuse if already fetched
        } else {
            // Fetch and cache
            let fetched = walk_commits_with_depth(base_commit, depth)?
                .collect::<Result<Vec<_>>>()?;
            commit_cache.insert(depth.clone(), fetched.clone());
            fetched
        };

        // Apply section-specific filters
        let filtered = commits.into_iter()
            .filter(|c| section_matches_filters(c, section))
            .collect();

        section_data.push((section.id.clone(), filtered));
    }

    render_template(section_data)
}
```

**Benefits**:
- ✅ **Simple implementation** - straightforward, no complex global depth calculation
- ✅ **True isolation** - each section's git query is independent
- ✅ **Delegates to git** - leverages git's optimized date filtering
- ✅ **Works naturally with CLI** - `--since`/`--until` compose cleanly
- ✅ **Basic caching helps** - identical depth specs share data
- ✅ **Lazy evaluation possible** - only fetch what's actually rendered
- ✅ **Memory efficient** - don't need all commits in memory at once
- ✅ **Progressive rendering** - can start rendering sections before all data loaded
- ✅ **Efficient for sparse measurements** - don't fetch commits without measurements

**Drawbacks**:
- ❌ **Multiple git invocations** - N sections = up to N git calls (or fewer with cache)
- ❌ **Repeated I/O** - git notes fetched multiple times for overlapping ranges
- ❌ **Repeated parsing** - same commits deserialized multiple times without cache
- ❌ **Potential inconsistency** - if commits added during report generation
- ⚠️ Cache requires `DepthSpec` to implement `Hash` and `Eq`

**Performance characteristics**:

*Worst case* (3 sections with distinct ranges):
```html
{{SECTION[recent] depth: 7d }}      → git log --since="7 days ago"
{{SECTION[monthly] depth: 30d }}    → git log --since="30 days ago"
{{SECTION[yearly] depth: 365d }}    → git log --since="365 days ago"
```
- 3 git log invocations
- 3 git notes fetches (with overlap)
- 3 deserialization passes

*Best case* (3 sections with same range):
```html
{{SECTION[tests] depth: 30d, measurement-filter: ^test:: }}
{{SECTION[benches] depth: 30d, measurement-filter: ^bench:: }}
{{SECTION[builds] depth: 30d, measurement-filter: ^build:: }}
```
- 1 git log invocation (cached)
- 1 git notes fetch
- 1 deserialization pass
- Cheap in-memory filtering per section

**Use cases**:
- Small repositories where git operations are fast
- Sparse measurements (only 10% of commits have data)
- Sections with distinct, non-overlapping ranges
- Memory-constrained environments
- Progressive/streaming report generation

**Potential optimization**: Smart range merging to detect overlapping ranges and minimize fetches.

---

#### Comparison Matrix

| Aspect | Option 1 (Git-Delegated) | Option 2 (Reuse Parser) | Option 3 (chrono-english) | Option 4 (Hybrid) | Option 5 (Multi-Fetch) |
|--------|-------------------------|------------------------|--------------------------|------------------|----------------------|
| **Git calls** | 1 | 1 | 1 | 1 | N (or fewer with cache) |
| **I/O overhead** | Low | Low | Low | Low | Higher |
| **Memory usage** | Higher (all commits) | Higher (all commits) | Higher (all commits) | Higher (all commits) | Lower (per section) |
| **Implementation** | Simple | Medium | Medium | Medium | Simple |
| **Date formats** | Limited (`7d`, `48h`) | Limited (`7d`, `48h`) | Rich (natural language) | CLI: rich, Template: limited | Limited (`7d`, `48h`) |
| **Dependencies** | None | None | chrono-english | None | None |
| **Parsing location** | Runtime (git) | Template load time | Template load time | Mixed | Runtime (git) |
| **CLI consistency** | Good | Moderate | Excellent | Separate concerns | Good |
| **Sparse data** | Inefficient | Inefficient | Inefficient | Inefficient | Efficient |
| **Progressive render** | No | No | No | No | Yes |
| **Timestamp field needed** | Yes | Yes | Yes | Yes | No |

---

#### Recommendation

**Option 1 (Unified DepthSpec with Git-Delegated Parsing)** is recommended for most use cases:

**Rationale**:
1. **Performance** - Single git call minimizes I/O and parsing overhead
2. **Simplicity** - Minimal code, delegates complexity to git
3. **Usability** - Consistent format across CLI and templates (`7d`, `48h`)
4. **Maintenance** - No new dependencies, simple validation logic
5. **Acceptable trade-off** - Templates typically use predictable time windows, not complex natural language dates

**When to use alternatives**:
- **Option 2**: If consistency with existing `remove` command format is critical
- **Option 3**: If rich natural language date support in templates is required (rare)
- **Option 4**: If CLI and template parsing should be truly independent
- **Option 5**: If repository is very large with sparse measurements, or memory is constrained

**Implementation path**: Start with Option 1, potentially add Option 5's multi-fetch as an optimization for large repos if performance issues arise in practice.

### 4. Implementation Strategy

#### Phase 1: Date-Based Filtering

1. **Update CLI argument parser** (`cli.rs`)
   - Add `--since` / `--after` options (type: `Option<String>`)
   - Add `--until` / `--before` options (type: `Option<String>`)
   - Make these options mutually compatible with `-n/--max-count`
   - Validation: Warn if both `-n` and date options are used (date options take precedence)

2. **Update git log invocation** (`git/git_interop.rs`)
   - Modify `walk_commits_from()` to accept date parameters
   - Build git log command dynamically:
     ```bash
     git --no-pager log --no-color --ignore-missing \
       [--since=<date>] [--until=<date>] [-n <num_commits>] \
       --first-parent --pretty=--,%H,%s,%an,%D%n%N --decorate=full \
       --notes=<temp_ref> <resolved_commit>
     ```
   - Ensure `--first-parent` is preserved for mainline tracking

3. **Update audit logic** (`audit.rs`)
   - Pass date parameters through the call chain
   - Update measurement retrieval to handle date-filtered commits
   - Ensure statistical analysis works with variable-length commit lists

4. **Update report logic** (`reporting.rs`)
   - Pass date parameters through the call chain
   - Update report generation to handle date-filtered commits
   - Ensure template rendering works with variable-length commit lists

5. **Add validation and warnings**
   - Warn if date range produces fewer commits than `--min-measurements` (audit only)
   - Provide clear error messages for invalid date formats (let git handle validation)
   - Document that `--first-parent` is always used (important for PR-based workflows)

#### Phase 2: Revision Range Support

1. **Update CLI argument parser** (`cli.rs`)
   - Add optional positional argument for revision range
   - Type: `Option<String>`
   - Examples: `HEAD~10..HEAD`, `main..feature`, `v1.0..v2.0`

2. **Parse and validate revision ranges**
   - Use git to validate the range: `git rev-parse <range>`
   - Split ranges into start and end commits
   - Handle special cases:
     - `A..B` - commits reachable from B but not from A
     - `A...B` - symmetric difference (commits in either A or B but not both)
     - Single commit - just that commit and its ancestors (with `-n` or date limits)

3. **Update git log invocation**
   - Support passing ranges directly to `git log`:
     ```bash
     git --no-pager log --no-color --ignore-missing \
       [--since=<date>] [--until=<date>] [-n <num_commits>] \
       --first-parent --pretty=--,%H,%s,%an,%D%n%N --decorate=full \
       --notes=<temp_ref> <revision-range>
     ```
   - Note: When using ranges, `<resolved_commit>` is replaced with the range spec

4. **Handle range precedence**
   - Priority order: revision range > date filters > `-n` count
   - If revision range is specified:
     - Date filters can further restrict the range
     - `-n` limits the number of commits within the range
   - Clear documentation of how options interact

#### Phase 3: Template Depth Enhancement (Report Command)

**Note**: This phase requires choosing an implementation approach from Section 3 (Date Parsing Implementation Options). The steps below describe the general requirements; specific implementation details depend on the chosen option. **Option 1 (Unified DepthSpec with Git-Delegated Parsing) is recommended** as the default approach.

1. **Add time-based depth parsing** (`reporting_config.rs`)
   - Create new function `parse_depth_value()` similar to existing `parse_datetime_value()`
   - Support formats: `Nd` (days), `Nh` (hours), `Nm` (minutes), `N` (commits)
   - Return enum: `DepthSpec::CommitCount(usize)` or `DepthSpec::TimeBased(DateTime<Utc>)`

2. **Update SectionConfig** (`reporting_config.rs`)
   - Change `depth: Option<usize>` to `depth: Option<DepthSpec>`
   - Update `parse()` method to use new `parse_depth_value()` function
   - Maintain backward compatibility with integer-only depth values

3. **Update report generation** (`reporting.rs`)
   - When processing sections with `DepthSpec::TimeBased`, convert to `--since` parameter
   - Pass appropriate parameters to `walk_commits_from()` based on depth type
   - Handle interaction between global CLI date filters and section-specific depth

4. **Add validation**
   - Validate time-based depth format (error on invalid unit)
   - Warn if time-based depth produces zero commits
   - Document that weeks (`Nw`) are supported for consistency with `remove` command

#### Phase 4: Enhanced Usability

1. **Add convenience options**
   - `--last-week` - Alias for `--since="1 week ago"`
   - `--last-month` - Alias for `--since="1 month ago"`
   - `--today` - Alias for `--since="midnight"`

2. **Improve output and diagnostics**
   - Display the actual date range being analyzed
   - Show total commits examined
   - Warn if insufficient historical data for statistical significance (audit)
   - Warn if date range produces empty sections (report)
   - Example output:
     ```
     Analyzing commits from 2025-12-01 to 2025-12-15
     Found 47 commits, 23 with measurements for 'nvim'
     ```

3. **Update documentation**
   - Add examples to README
   - Document interaction between options
   - Provide migration guide from `-n` to date-based filtering
   - Update `docs/dashboard-templates.md` with time-based depth examples

### 5. Backward Compatibility

- **Preserve existing behavior**: `-n/--max-count` continues to work exactly as before
- **No breaking changes**: All existing scripts and CI configurations remain functional
- **Additive changes only**: New options are optional and don't affect existing usage
- **Default behavior unchanged**: Without new options, git-perf behaves identically

### 6. Testing Strategy

1. **Unit tests**
   - CLI argument parsing with various date formats
   - Validation of mutually exclusive options
   - Date format edge cases (relative dates, ISO 8601, etc.)

2. **Integration tests**
   - Test with real git repositories
   - Verify `--since` correctly filters commits
   - Verify `--until` correctly filters commits
   - Test range specifications (`A..B`, `A...B`)
   - Ensure `--first-parent` is always applied

3. **Edge cases**
   - Empty date ranges (no commits match)
   - Invalid date formats
   - Ranges with no measurements
   - Combining `-n`, `--since`, and `--until`
   - Very large date ranges (performance testing)

4. **Regression tests**
   - Ensure existing `-n` behavior is unchanged
   - Verify all existing CI scripts still work
   - Test with current dotfiles CI configuration

## Implementation Checklist

### Phase 1: Date-Based Filtering
- [ ] Add `--since`/`--after` CLI options in `cli_types/src/lib.rs` for both `Audit` and `Report` commands
- [ ] Add `--until`/`--before` CLI options in `cli_types/src/lib.rs` for both commands
- [ ] Update `walk_commits_from()` signature in `git_interop.rs` to accept date parameters
- [ ] Modify git log invocation to include date filters
- [ ] Update `audit.rs` to pass date parameters through call chain
- [ ] Update `reporting.rs` to pass date parameters through call chain
- [ ] Add validation for date options
- [ ] Add warning when date range produces insufficient measurements (audit)
- [ ] Add warning when date range produces empty reports (report)
- [ ] Write unit tests for CLI parsing
- [ ] Write integration tests for date filtering (audit)
- [ ] Write integration tests for date filtering (report)
- [ ] Update README with date filter examples for both commands

### Phase 2: Revision Range Support
- [ ] Add revision range as optional positional argument in `cli_types/src/lib.rs` for both commands
- [ ] Implement revision range validation using `git rev-parse`
- [ ] Handle `..` (range) syntax
- [ ] Handle `...` (symmetric difference) syntax
- [ ] Update git log invocation to accept ranges
- [ ] Define and document option precedence rules
- [ ] Document audit range semantics (latest commit is "head", rest is "tail")
- [ ] Document report range semantics (all commits visualized equally)
- [ ] Write unit tests for range parsing
- [ ] Write integration tests for various range formats (audit)
- [ ] Write integration tests for various range formats (report)
- [ ] Update README with range examples for both commands

### Phase 3: Template Depth Enhancement
**Note**: Choose implementation approach from Section 3. Checklists below cover different options.

#### Common Tasks (All Options)
- [ ] Choose and document implementation option (see Section 3)
- [ ] Create `DepthSpec` enum in appropriate location (CommitCount | TimeBased/Since/String)
- [ ] Update `SectionConfig.depth` type from `Option<usize>` to `Option<DepthSpec>`
- [ ] Update `SectionConfig::parse()` to use new depth parser
- [ ] Validate time-based depth format and provide clear errors
- [ ] Add warning when time-based depth produces zero commits
- [ ] Write unit tests for depth parsing (integers and time-based formats)
- [ ] Write integration tests for multi-section reports with time-based depth
- [ ] Update `docs/dashboard-templates.md` with time-based depth examples

#### Option 1/2/4 Specific (Single Fetch with In-Memory Filtering)
- [ ] Update git log format to include commit timestamp: `--pretty=--,%H,%s,%an,%ct,%D%n%N`
- [ ] Add `timestamp: DateTime<Utc>` field to `Commit` struct in `data.rs`
- [ ] Implement `calculate_global_depth()` to find maximum range across all sections
- [ ] Implement in-memory filtering: `filter_commits_by_depth(commits, depth_spec)`
- [ ] Update report generation to fetch once, filter per section
- [ ] Handle interaction between global CLI date filters and section depth

#### Option 5 Specific (Multi-Fetch Strategy)
- [ ] Implement `walk_commits_with_depth(commit, depth_spec)` with git arg conversion
- [ ] Add caching layer: `HashMap<DepthSpec, Vec<Commit>>`
- [ ] Implement `DepthSpec::Hash` and `DepthSpec::Eq` for cache keys
- [ ] Update report generation to fetch per section with cache lookup
- [ ] Add metrics/logging for cache hit rates (optional)
- [ ] Consider smart range merging optimization (optional, future)

### Phase 4: Enhanced Usability
- [ ] Add convenience date aliases (`--last-week`, `--last-month`, `--today`)
- [ ] Enhance audit output to show analyzed date range
- [ ] Enhance report metadata to include analyzed date range
- [ ] Display commit count and measurement statistics
- [ ] Add detailed diagnostic warnings for both commands
- [ ] Create comprehensive documentation
- [ ] Write migration guide from `-n` to date-based filtering
- [ ] Add examples for common use cases (audit and report)
- [ ] Document template depth enhancement with examples

### Testing & Quality
- [ ] All unit tests passing (CLI, audit, report, templates)
- [ ] All integration tests passing (audit and report)
- [ ] Edge case testing complete
  - [ ] Empty date ranges
  - [ ] Invalid date formats
  - [ ] Invalid depth formats
  - [ ] Zero-commit sections in multi-section reports
  - [ ] Interaction between global dates and section depth
- [ ] Regression testing
  - [ ] Existing `-n` behavior unchanged
  - [ ] Existing audit behavior unchanged
  - [ ] Existing report behavior unchanged
  - [ ] Existing template depth (integer) behavior unchanged
- [ ] Performance testing with large repositories
- [ ] Documentation review
- [ ] Code review

## Examples of New Usage

### Audit Command Examples

#### Date-Based Filtering

```bash
# Audit commits from the last 2 weeks
git perf audit --since="2 weeks ago" -m nvim -m zsh

# Audit commits in December 2025
git perf audit --since="2025-12-01" --until="2025-12-31" -m ci

# Audit today's commits
git perf audit --since="midnight" -m test

# Combine with count limit (audit last 20 commits from past month)
git perf audit --since="1 month ago" -n 20 -m nvim
```

#### Range-Based Filtering

```bash
# Audit commits between two tags
# Latest commit in range is "head", rest is "tail" for comparison
git perf audit v1.0..v2.0 -m nvim -m zsh

# Audit last 10 commits
git perf audit HEAD~10..HEAD -m ci

# Audit commits in feature branch not in main
git perf audit main..feature -m test

# Audit symmetric difference between branches
git perf audit main...feature -m nvim
```

#### Combined Usage

```bash
# Audit commits in a range, but only from last week
git perf audit --since="1 week ago" main..feature -m nvim

# Audit last 10 commits from past month
git perf audit --since="1 month ago" -n 10 -m zsh
```

### Report Command Examples

#### Date-Based Filtering

```bash
# Generate report for last 30 days
git perf report --since="30 days ago" -o monthly-report.html

# Generate report for specific release period
git perf report --since="2025-12-01" --until="2025-12-31" -o december-report.html

# Report on last week with specific measurements
git perf report --since="1 week ago" -m nvim -m zsh -o weekly.html

# Combine with count limit (last 50 commits from past 6 months)
git perf report --since="6 months ago" -n 50 -o report.html
```

#### Range-Based Filtering

```bash
# Report on commits between two releases
git perf report v1.0..v2.0 -o release-comparison.html

# Report on last 100 commits
git perf report HEAD~100..HEAD -o last-100.html

# Report on feature branch changes
git perf report main..feature -o feature-analysis.html

# CSV export for specific range
git perf report v1.0..v2.0 -o data.csv
```

#### Multi-Section Reports with Time-Based Depth

Create `dashboard.html` template:
```html
<!DOCTYPE html>
<html>
<head>
    <title>Performance Dashboard</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    <h1>Performance Overview</h1>

    <h2>Last 24 Hours</h2>
    {{SECTION[recent-activity]
        measurement-filter: ^test::
        depth: 24h
        show-changes: true
    }}

    <h2>Last Week (Build Times)</h2>
    {{SECTION[weekly-builds]
        measurement-filter: ^build::
        depth: 7d
        aggregate-by: median
    }}

    <h2>Last 30 Days (All Metrics)</h2>
    {{SECTION[monthly-overview]
        depth: 30d
        show-epochs: true
    }}

    <h2>All-Time History (Top 1000 commits)</h2>
    {{SECTION[historical]
        depth: 1000
        aggregate-by: median
    }}
</body>
</html>
```

Generate the report:
```bash
git perf report -t dashboard.html -o performance-dashboard.html
```

#### Combined Usage

```bash
# Report on specific range with template
git perf report v1.0..v2.0 -t dashboard.html -o release-report.html

# Report on last month, but use template with multiple depth specifications
# Global --since applies first, then each section's depth further filters
git perf report --since="1 month ago" -t dashboard.html -o report.html
```

## Migration Impact

### dotfiles Repository

The current usage in `/root/repo/script/ci.sh:68` will continue to work without changes:

```bash
# Existing (no changes required)
git perf audit -n 40 -m nvim -m zsh -m ci -m test -m nix-closure-size \
  -s "os=$os" --min-measurements 10

# Could optionally be updated to use date-based filtering
git perf audit --since="6 months ago" -m nvim -m zsh -m ci -m test \
  -m nix-closure-size -s "os=$os" --min-measurements 10
```

### Benefits for dotfiles

1. **Historical analysis**: Easily analyze performance trends over specific time periods
2. **Release auditing**: Compare performance between tagged releases
3. **Flexible CI**: Adjust audit scope based on time since last run
4. **Better debugging**: Narrow down when performance regressions were introduced

## Dependencies

- **git-perf repository**: `kaihowl/git-perf`
- **Git version**: Requires git 1.7.0+ (for `--since`/`--until` support)
- **No new external dependencies**: Uses existing git functionality

## Timeline Considerations

This is a feature enhancement to an external tool (`git-perf`), not this repository. Implementation steps:

1. Fork or contribute to `kaihowl/git-perf`
2. Implement changes following the phases above
3. Submit pull request to upstream repository
4. Update dotfiles to use new version once merged
5. Optionally update CI scripts to leverage new features

## Open Questions

### General Questions

1. Should we maintain strict backward compatibility, or is this a good opportunity for a major version bump?
2. Should date filtering work with other subcommands (`measure`, `add`), or just `audit` and `report`?
3. How should we handle ambiguous date formats (let git handle it vs. explicit validation)?
4. Should we add a `--dry-run` flag to show which commits would be analyzed?

### Audit-Specific Questions

5. What's the best way to communicate when date filters produce insufficient data for statistical analysis?
6. When using revision ranges with audit, should we allow specifying which commit in the range is "head"?
   - Current proposal: Latest commit in range is always "head"
   - Alternative: Add `--head-commit` flag to specify explicitly

### Report-Specific Questions

7. How should time-based depth interact with global date filters in multi-section reports?
   - Option A: Section depth is relative to global filter (depth further restricts)
   - Option B: Section depth is independent (ignores global filter)
   - **Proposed**: Option A for consistency and least surprise

8. Should we support time-based depth in the global `-n` flag, or only in template sections?
   - Current proposal: Only in template sections initially
   - Future enhancement: Allow `-n 7d` as CLI flag

9. Should empty sections (zero commits) in multi-section reports cause:
   - A warning but still generate report?
   - An error that prevents report generation?
   - **Proposed**: Warning with empty placeholder in report

10. Should we add a `{{COMMIT_RANGE}}` placeholder for templates to show the analyzed range?
    - Would be useful for dashboard titles: "Performance Report: 2025-12-01 to 2025-12-31"
    - **Proposed**: Yes, add this placeholder

### Implementation Strategy Questions

11. Which implementation option should we choose for template depth parsing?
    - **Option 1 (Git-Delegated)**: Recommended for simplicity and performance
    - **Option 2 (Reuse Parser)**: Good for consistency with `remove` command
    - **Option 5 (Multi-Fetch)**: Better for large repos with sparse measurements
    - Consider starting with Option 1, adding Option 5 as optimization if needed

12. Should we add performance benchmarks to compare Option 1 vs Option 5?
    - Test scenarios: small repos, large repos, sparse measurements, dense measurements
    - Establish thresholds for when to recommend multi-fetch approach
    - **Proposed**: Yes, add benchmarks after initial implementation

13. Should the timestamp field in `Commit` struct be optional or required?
    - Required: Cleaner code, but requires updating all existing tests
    - Optional: Easier migration, but more error handling
    - **Proposed**: Required - commits always have timestamps

### Template Syntax Questions

14. Should the time-based depth syntax support additional units?
    - Currently proposed: `Nm` (minutes), `Nh` (hours), `Nd` (days), `Nw` (weeks)
    - Should we add: `Nmo` (months), `Ny` (years)?
    - **Proposed**: Start with m/h/d/w, add mo/y if requested

15. Should we allow combining time and commit count in depth?
    - Example: `depth: 30d, max: 100` (last 30 days but cap at 100 commits)
    - **Proposed**: Phase 2 enhancement, not initial implementation

## References

- git-perf repository: `kaihowl/git-perf`
- git log documentation: https://git-scm.com/docs/git-log
- git revision range syntax: https://git-scm.com/docs/gitrevisions
- Current usage in dotfiles: `/root/repo/script/ci.sh:68`, `/root/repo/common/perf.sh`
