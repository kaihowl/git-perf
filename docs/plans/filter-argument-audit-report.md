# Plan: Add Filter Argument to Audit and Report Subcommands

**Status:** Planned
**Created:** 2025-10-26
**Related:** Enhancement to measurement selection in audit and report commands

## Overview

Add a `--filter` (`-f`) argument to both `audit` and `report` subcommands that filters measurements by name using regex pattern matching. Multiple filters can be supplied with OR semantics (if any filter matches, the measurement is included). This argument works in addition to the existing `--measurement` argument.

## Motivation

Currently, users can only select measurements by:
- Exact name matching (`--measurement` / `-m`)
- Key-value pair matching (`--key-value` / `-k` for report, `--selectors` / `-s` for audit)

This is limiting when:
- Working with many related measurements (e.g., `benchmark_x64`, `benchmark_arm64`, `benchmark_riscv`)
- Wanting to include/exclude measurement groups by naming patterns
- Needing flexible filtering without specifying every exact name

**Benefits of regex-based filtering:**
- **Pattern matching** - Select measurements matching a pattern (e.g., `bench.*_release`)
- **Flexibility** - Combine multiple patterns with OR logic
- **Composability** - Works alongside existing `--measurement` argument
- **Power** - Full regex support for complex filtering needs

## Goals

1. **Add `--filter` argument** to Report and Audit commands
2. **Regex pattern matching** - Use regex for flexible pattern matching
3. **OR semantics** - Multiple filters combined with OR logic (any match = include)
4. **Composability** - Works alongside existing `--measurement` argument
5. **Clear documentation** - Update manpages and help text
6. **Backward compatibility** - No breaking changes to existing behavior

## Non-Goals

- Replacing existing `--measurement` argument (they complement each other)
- Supporting exclude/negation filters (can be added later if needed)
- Complex filter expressions with AND/OR/NOT operators
- Filtering by fields other than measurement name (use existing key-value args)

## Design

### CLI Syntax

```bash
# Single filter
git-perf report -f "bench.*"

# Multiple filters (OR logic)
git-perf report -f "benchmark_.*" -f "test_.*"

# Combined with --measurement
git-perf report -m my_bench -f ".*_release"

# Audit with filters
git-perf audit -m my_bench -f ".*_x64"
```

### Filter Semantics

**For Report:**
```
Include measurement IF:
  (measurement_names.empty OR name IN measurement_names)
  AND
  (filters.empty OR name MATCHES ANY filter_regex)
  AND
  key_values_is_superset_of(key_values)
```

**For Audit:**
```
Include measurement IF:
  name == measurement
  AND
  (filters.empty OR name MATCHES ANY filter_regex)
  AND
  key_values_is_superset_of(selectors)
```

### Regex Matching

- Use Rust `regex` crate for pattern matching
- Patterns are **unanchored by default** (matches anywhere in the string)
- Users can anchor explicitly: `^bench.*$` for start/end anchoring
- Case-sensitive matching (consistent with existing behavior)
- Invalid regex patterns return user-friendly error messages

**Examples:**
- `bench` - Matches any measurement containing "bench"
- `^bench` - Matches measurements starting with "bench"
- `bench$` - Matches measurements ending with "bench"
- `^benchmark_x64$` - Exact match for "benchmark_x64"
- `bench.*_v[0-9]+` - Matches "bench_foo_v1", "benchmark_bar_v2", etc.

## Implementation Plan

### 1. Add Dependency

**File:** `Cargo.toml` (workspace or cli_types)

```toml
[dependencies]
regex = "1.10"
```

### 2. CLI Changes

**File:** `cli_types/src/lib.rs`

#### Report Command (lines ~213-236)
```rust
Report {
    /// HTML output file
    #[arg(short, long, default_value = "output.html")]
    output: PathBuf,

    #[command(flatten)]
    report_history: CliReportHistory,

    /// Select an individual measurements instead of all
    #[arg(short, long)]
    measurement: Vec<String>,

    /// Key-value pairs separated by '=', select only matching measurements
    #[arg(short, long, value_parser=parse_key_value)]
    key_value: Vec<(String, String)>,

    /// Filter measurements by regex pattern (can be specified multiple times).
    /// If any filter matches, the measurement is included (OR logic).
    /// Patterns are unanchored by default. Use ^pattern$ for exact matches.
    /// Example: -f "bench.*" -f "test_.*"
    #[arg(short = 'f', long = "filter")]
    filter: Vec<String>,

    /// Create individual traces in the graph by grouping with the value of this selector
    #[arg(short, long, value_parser=parse_spaceless_string)]
    separate_by: Option<String>,

    /// What to aggregate the measurements in each group with
    #[arg(short, long)]
    aggregate_by: Option<ReductionFunc>,
}
```

#### Audit Command (lines ~301-345)
```rust
Audit {
    #[arg(short, long, value_parser=parse_spaceless_string, action = clap::ArgAction::Append, required = true)]
    measurement: Vec<String>,

    #[command(flatten)]
    report_history: CliReportHistory,

    /// Key-value pair separated by "=" with no whitespaces to subselect measurements
    #[arg(short, long, value_parser=parse_key_value)]
    selectors: Vec<(String, String)>,

    /// Filter measurements by regex pattern (can be specified multiple times).
    /// If any filter matches, the measurement is included (OR logic).
    /// Patterns are unanchored by default. Use ^pattern$ for exact matches.
    /// Example: -f "bench.*_x64"
    #[arg(short = 'f', long = "filter")]
    filter: Vec<String>,

    /// Minimum number of measurements needed...
    #[arg(long, value_parser=clap::value_parser!(u16).range(2..))]
    min_measurements: Option<u16>,

    /// What to aggregate the measurements in each group with...
    #[arg(short, long)]
    aggregate_by: Option<ReductionFunc>,

    /// Multiple of the dispersion after which an outlier is detected...
    #[arg(short = 'd', long)]
    sigma: Option<f64>,

    /// Method for calculating statistical dispersion...
    #[arg(short = 'D', long, value_enum)]
    dispersion_method: Option<DispersionMethod>,
}
```

### 3. CLI Dispatcher Changes

**File:** `git_perf/src/cli.rs`

#### Report Handler (lines ~55-69)
```rust
Commands::Report {
    output,
    separate_by,
    report_history,
    measurement,
    key_value,
    aggregate_by,
    filter,
} => report(
    output,
    separate_by,
    report_history.max_count,
    &measurement,
    &key_value,
    aggregate_by.map(ReductionFunc::from),
    &filter,
)
```

#### Audit Handler (lines ~70-95)
```rust
Commands::Audit {
    measurement,
    report_history,
    selectors,
    min_measurements,
    aggregate_by,
    sigma,
    dispersion_method,
    filter,
} => {
    // ... validation logic ...
    audit::audit_multiple(
        &measurement,
        report_history.max_count,
        min_measurements,
        &selectors,
        aggregate_by.map(ReductionFunc::from),
        sigma,
        dispersion_method.map(crate::stats::DispersionMethod::from),
        &filter,
    )
}
```

### 4. Regex Compilation Helper

**File:** `git_perf/src/lib.rs` or new `git_perf/src/filter.rs`

```rust
use regex::Regex;
use anyhow::{Context, Result};

/// Compile filter patterns into regex objects
pub fn compile_filters(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| {
            Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: '{}'", pattern))
        })
        .collect()
}

/// Check if a measurement name matches any of the compiled filters
pub fn matches_any_filter(name: &str, filters: &[Regex]) -> bool {
    if filters.is_empty() {
        return true; // No filters = match all
    }
    filters.iter().any(|re| re.is_match(name))
}
```

### 5. Report Implementation Changes

**File:** `git_perf/src/reporting.rs`

#### Update Signature (line ~395)
```rust
pub fn report(
    output: PathBuf,
    separate_by: Option<String>,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
    filter_patterns: &[String],
) -> Result<()>
```

#### Update Implementation (lines ~400-425)
```rust
use crate::filter::{compile_filters, matches_any_filter};

pub fn report(
    output: PathBuf,
    separate_by: Option<String>,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
    filter_patterns: &[String],
) -> Result<()> {
    // Compile regex filters early to fail fast on invalid patterns
    let filters = compile_filters(filter_patterns)?;

    let commits = measurement_retrieval::walk_commits(num_commits)?
        .collect::<Result<Vec<_>>>()?;

    let relevant = |m: &MeasurementData| {
        // Apply measurement name filter (existing logic)
        if !measurement_names.is_empty() && !measurement_names.contains(&m.name) {
            return false;
        }

        // Apply regex filters (NEW logic)
        if !matches_any_filter(&m.name, &filters) {
            return false;
        }

        // Filter using subset relation: key_values ⊆ measurement.key_values
        m.key_values_is_superset_of(key_values)
    };

    // ... rest of implementation
}
```

### 6. Audit Implementation Changes

**File:** `git_perf/src/audit.rs`

#### Update audit_multiple Signature (line ~91)
```rust
pub fn audit_multiple(
    measurements: &[String],
    max_count: usize,
    min_count: Option<u16>,
    selectors: &[(String, String)],
    summarize_by: Option<ReductionFunc>,
    sigma: Option<f64>,
    dispersion_method: Option<DispersionMethod>,
    filter_patterns: &[String],
) -> Result<()>
```

#### Update audit Signature (line ~146)
```rust
fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
    filters: &[Regex],
) -> Result<AuditResult>
```

#### Update Implementation (lines ~91-184)
```rust
use crate::filter::{compile_filters, matches_any_filter};

pub fn audit_multiple(
    measurements: &[String],
    max_count: usize,
    min_count: Option<u16>,
    selectors: &[(String, String)],
    summarize_by: Option<ReductionFunc>,
    sigma: Option<f64>,
    dispersion_method: Option<DispersionMethod>,
    filter_patterns: &[String],
) -> Result<()> {
    // Compile regex filters early
    let filters = compile_filters(filter_patterns)?;

    // ... config loading ...

    let mut results = Vec::new();
    for measurement in measurements {
        let result = audit(
            measurement,
            max_count,
            min_count_effective,
            selectors,
            summarize_by,
            sigma,
            dispersion_method,
            &filters,
        )?;
        results.push(result);
    }

    // ... result processing ...
}

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
    filters: &[Regex],
) -> Result<AuditResult> {
    let all = measurement_retrieval::walk_commits(max_count)?;

    let filter_by = |m: &MeasurementData| {
        // Existing measurement name check
        if m.name != measurement {
            return false;
        }

        // NEW: Apply regex filters
        if !matches_any_filter(&m.name, filters) {
            return false;
        }

        // Existing selector check
        m.key_values_is_superset_of(selectors)
    };

    let mut aggregates = measurement_retrieval::take_while_same_epoch(
        summarize_measurements(all, &summarize_by, &filter_by)
    );

    // ... rest of implementation
}
```

## Testing Strategy

### Unit Tests

**File:** `git_perf/src/filter.rs` or within existing test modules

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_valid_filters() {
        let patterns = vec!["bench.*".to_string(), "test_.*".to_string()];
        let result = compile_filters(&patterns);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_compile_invalid_regex() {
        let patterns = vec!["[invalid".to_string()];
        let result = compile_filters(&patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_matches_any_filter_empty() {
        let filters = vec![];
        assert!(matches_any_filter("anything", &filters));
    }

    #[test]
    fn test_matches_any_filter_single_match() {
        let patterns = vec!["bench.*".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(!matches_any_filter("test_foo", &filters));
    }

    #[test]
    fn test_matches_any_filter_or_logic() {
        let patterns = vec!["bench.*".to_string(), "test_.*".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(matches_any_filter("test_foo", &filters));
        assert!(!matches_any_filter("other_thing", &filters));
    }

    #[test]
    fn test_anchored_patterns() {
        let patterns = vec!["^bench.*$".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(!matches_any_filter("my_benchmark_x64", &filters));
    }

    #[test]
    fn test_complex_regex() {
        let patterns = vec![r"bench_.*_v\d+".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("bench_foo_v1", &filters));
        assert!(matches_any_filter("bench_bar_v23", &filters));
        assert!(!matches_any_filter("bench_baz_vX", &filters));
    }
}
```

### Integration Tests

```rust
#[test]
fn test_report_with_regex_filter() {
    // Setup test repo with multiple measurements
    // Run report with filter
    // Verify only matching measurements included
}

#[test]
fn test_audit_with_regex_filter() {
    // Setup test repo with measurements
    // Run audit with filter
    // Verify filtering works correctly
}

#[test]
fn test_filter_combined_with_measurement() {
    // Test interaction between --measurement and --filter
}
```

### Manual Testing

```bash
# Create test measurements
git-perf add -m benchmark_x64 --value 1.5
git-perf add -m benchmark_arm64 --value 1.6
git-perf add -m benchmark_riscv --value 1.7
git-perf add -m test_integration --value 2.0
git-perf add -m test_unit --value 0.5

# Test report with single filter
git-perf report -f "benchmark_.*"

# Test report with multiple filters (OR logic)
git-perf report -f "benchmark_x64" -f "test_.*"

# Test anchored patterns
git-perf report -f "^benchmark_"

# Test complex regex
git-perf report -f "bench.*_(x64|arm64)"

# Test audit with filters
git-perf audit -m benchmark_x64 -f ".*_x64"

# Test invalid regex (should error clearly)
git-perf report -f "[invalid"

# Test combined with other args
git-perf report -m benchmark_x64 -k arch=x64 -f "bench.*"
```

## Error Handling

### Invalid Regex Patterns

```rust
// Should provide clear error messages
git-perf report -f "[invalid"

// Error: Invalid regex pattern: '[invalid'
// Caused by: regex parse error:
//     [invalid
//     ^
//     error: unclosed character class
```

Implementation:
```rust
Regex::new(pattern)
    .with_context(|| format!("Invalid regex pattern: '{}'", pattern))
```

### No Matching Measurements

When filters match no measurements:
- **Report:** Generate empty report or show warning
- **Audit:** Show warning but don't fail (consistent with existing behavior)

## Documentation Updates

### Manpage Regeneration

After implementing, run:
```bash
./scripts/generate-manpages.sh
```

This updates:
- `man/git-perf-report.1`
- `man/git-perf-audit.1`
- `docs/commands/*.md`

### Help Text Examples

Add to command documentation:

```
EXAMPLES:
    # Filter measurements by pattern
    git-perf report -f "bench.*"

    # Multiple filters with OR logic
    git-perf report -f "benchmark_.*" -f "test_.*"

    # Exact match using anchors
    git-perf report -f "^benchmark_x64$"

    # Complex regex pattern
    git-perf report -f "bench.*_(x64|arm64)_v[0-9]+"

    # Combine with measurement selection
    git-perf report -m my_bench -f ".*_release"

    # Audit with regex filter
    git-perf audit -m benchmark -f ".*_production"
```

## Backward Compatibility

- No breaking changes - all existing commands work identically
- New `--filter` argument is optional
- Empty filter list (default) matches all measurements (no change in behavior)
- Existing `--measurement` and key-value arguments work as before

## Edge Cases

1. **Empty filter list:** Matches all (existing behavior)
2. **Filter matches nothing:** Empty result (with warning)
3. **Invalid regex:** Clear error message with pattern shown
4. **Filter conflicts with --measurement:**
   - Both must pass (AND logic between arg types)
   - If conflict, results in empty set (expected behavior)
5. **Case sensitivity:** Case-sensitive (use `(?i)pattern` for case-insensitive)
6. **Special regex characters:** Fully supported (escaped by user if literal needed)

## Future Enhancements

Potential future additions (not in this plan):
- Negation filters: `--exclude-filter` or `--filter "!pattern"`
- Case-insensitive flag: `--filter-ignore-case`
- Filter presets/aliases in config file
- Filter by other fields (timestamp, key-values)
- Complex filter expressions with AND/OR/NOT operators

## Pre-Submission Checklist

1. [ ] Add `regex` dependency to appropriate Cargo.toml
2. [ ] Update Report CLI definition
3. [ ] Update Audit CLI definition
4. [ ] Update Report dispatcher
5. [ ] Update Audit dispatcher
6. [ ] Implement filter compilation helper
7. [ ] Update report() function with regex filtering
8. [ ] Update audit_multiple() and audit() with regex filtering
9. [ ] Add unit tests for regex compilation and matching
10. [ ] Add integration tests for report and audit
11. [ ] Manual testing with various regex patterns
12. [ ] Test error handling for invalid regex
13. [ ] Run `cargo fmt`
14. [ ] Run `cargo clippy`
15. [ ] Run `cargo nextest run -- --skip slow`
16. [ ] Regenerate manpages: `./scripts/generate-manpages.sh`
17. [ ] Commit regenerated documentation with code changes
18. [ ] Create PR with conventional commit title: `feat(cli): add regex filter argument to audit and report commands`

## Summary

This plan adds a powerful `--filter` argument to both `audit` and `report` subcommands using regex pattern matching. The implementation:

- Uses the `regex` crate for robust pattern matching
- Supports multiple filters with OR semantics
- Works alongside existing `--measurement` argument
- Provides clear error messages for invalid patterns
- Maintains backward compatibility
- Follows existing code patterns and conventions

The regex-based approach provides maximum flexibility while remaining intuitive for common use cases (simple substring matching works without regex knowledge, while power users can leverage full regex capabilities).
