# Adding Measurements to Non-HEAD Commits

This guide explains how to add performance measurements to specific commits in your Git history, rather than only to the current HEAD commit.

## Table of Contents

- [Overview](#overview)
- [When to Use Non-HEAD Measurements](#when-to-use-non-head-measurements)
- [Basic Usage](#basic-usage)
- [Command Reference](#command-reference)
- [Examples](#examples)
- [Technical Details](#technical-details)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

By default, git-perf adds measurements to the current HEAD commit. However, you can target specific commits using:

- **Write operations**: Use the `--commit` flag with `add`, `measure`, and `import` commands
- **Read operations**: Use a positional `<COMMIT>` argument with `report` and `audit` commands

This feature enables scenarios like:
- Adding historical performance data from CI runs
- Measuring performance at specific release points
- Importing benchmark results for past commits
- Creating reports for specific branches or tags

## When to Use Non-HEAD Measurements

**Common Use Cases:**

1. **Backfilling Historical Data**: Import performance data from past CI runs or benchmark results
   ```bash
   git perf import junit old-results.xml --commit v1.0.0
   ```

2. **Branch-Specific Measurements**: Add measurements to feature branches without switching
   ```bash
   git perf add 150.5 -m build_time --commit feature-branch
   ```

3. **Release Benchmarking**: Measure performance at specific release points
   ```bash
   git perf measure -m startup_time --commit v2.1.0 -- ./app --benchmark
   ```

4. **Historical Analysis**: Generate reports from specific points in history
   ```bash
   git perf report v1.0.0 -o historical-report.html
   ```

5. **Audit Specific Commits**: Check performance regressions at non-HEAD commits
   ```bash
   git perf audit feature-branch -m critical_path
   ```

## Basic Usage

### Write Operations (Adding Measurements)

Use the `--commit` flag to target a specific commit:

```bash
# Add single measurement to a specific commit
git perf add 100.5 -m metric_name --commit <COMMIT>

# Measure command execution at a specific commit
git perf measure -m test_time --commit <COMMIT> -- <COMMAND>

# Import test results to a specific commit
git perf import junit results.xml --commit <COMMIT>
```

**Default Behavior**: Without `--commit`, measurements are added to HEAD:
```bash
git perf add 100.5 -m metric_name  # Adds to current HEAD
```

### Read Operations (Retrieving Measurements)

Use a positional argument to specify the starting commit:

```bash
# Generate report from a specific commit
git perf report <COMMIT> -o report.html

# Audit measurements at a specific commit
git perf audit <COMMIT> -m metric_name
```

**Default Behavior**: Without `<COMMIT>`, commands operate on HEAD:
```bash
git perf report -o report.html  # Reports from HEAD
```

## Command Reference

### Commands Supporting `--commit` Flag

| Command | Flag | Description | Example |
|---------|------|-------------|---------|
| `add` | `--commit <COMMIT>` | Add measurement to specific commit | `git perf add 100 -m test --commit abc123` |
| `measure` | `--commit <COMMIT>` | Measure command at specific commit | `git perf measure -m build --commit HEAD~5 -- make` |
| `import` | `--commit <COMMIT>` | Import results to specific commit | `git perf import junit test.xml --commit v1.0` |

### Commands Supporting Positional `<COMMIT>` Argument

| Command | Argument | Description | Example |
|---------|----------|-------------|---------|
| `report` | `<COMMIT>` | Generate report from commit | `git perf report HEAD~10 -o report.html` |
| `audit` | `<COMMIT>` | Audit measurements at commit | `git perf audit v2.0.0 -m perf_test` |

### Supported Commit Formats

git-perf accepts any valid Git committish:

- **Full SHA**: `abc123def456...`
- **Short SHA**: `abc123`
- **Relative refs**: `HEAD~5`, `HEAD^`, `HEAD~10`
- **Branch names**: `feature-branch`, `main`, `develop`
- **Tag names**: `v1.0.0`, `release-2024`
- **Symbolic refs**: `HEAD`, `ORIG_HEAD`

## Examples

### Example 1: Backfilling Historical CI Data

Import test results from a previous CI run:

```bash
# You're on main branch, but want to add data for an old commit
git perf import junit old-ci-results.xml --commit abc123

# Verify the data was added
git perf report abc123 -o verify.html
```

### Example 2: Multi-Branch Development

Add measurements to different branches without switching:

```bash
# Current branch: main
git branch
# * main
#   feature-auth
#   feature-perf

# Add measurements to feature branches without checking them out
git perf add 120.5 -m login_time --commit feature-auth
git perf add 85.3 -m query_time --commit feature-perf

# Generate comparative reports
git perf report feature-auth -o auth-report.html
git perf report feature-perf -o perf-report.html
```

### Example 3: Release Performance Tracking

Measure performance at each release point:

```bash
# Tag releases
git tag v1.0.0 <commit1>
git tag v1.1.0 <commit2>
git tag v2.0.0 <commit3>

# Measure each release
git perf measure -m startup --commit v1.0.0 -- ./app --test-startup
git perf measure -m startup --commit v1.1.0 -- ./app --test-startup
git perf measure -m startup --commit v2.0.0 -- ./app --test-startup

# Compare across releases
git perf report v1.0.0 -o v1.0.0-report.html
git perf report v2.0.0 -o v2.0.0-report.html
```

### Example 4: Historical Regression Analysis

Audit past commits to find when a regression was introduced:

```bash
# Check if regression existed 10 commits ago
git perf audit HEAD~10 -m critical_test

# Check various points in history
for i in 5 10 15 20; do
    echo "=== Checking HEAD~$i ==="
    git perf audit HEAD~$i -m critical_test
done
```

### Example 5: Multiple Measurements on Same Commit

Add multiple different metrics to a specific commit:

```bash
# Target commit from history
TARGET=abc123

# Add various measurements
git perf add 100.5 -m build_time --commit $TARGET
git perf add 2048 -m binary_size --commit $TARGET
git perf add 15.3 -m test_duration --commit $TARGET

# All measurements appear in single report
git perf report $TARGET -o multi-metric-report.html
```

### Example 6: Branch Comparison Without Switching

Compare performance across branches:

```bash
# Working on main, want to compare with feature branch
MAIN_COMMIT=$(git rev-parse main)
FEATURE_COMMIT=$(git rev-parse feature-optimization)

# Add measurements to both
git perf measure -m benchmark --commit $MAIN_COMMIT -- ./run-benchmark
git perf measure -m benchmark --commit $FEATURE_COMMIT -- ./run-benchmark

# Generate comparison reports
git perf report $MAIN_COMMIT -o main-perf.html
git perf report $FEATURE_COMMIT -o feature-perf.html
```

## Technical Details

### How Notes Are Attached

git-perf uses Git's `git-notes` feature to store measurements. When you specify `--commit`:

1. **Commit Resolution**: The committish is resolved to a full SHA-1 hash
2. **Validation**: git-perf verifies the commit exists in the repository
3. **Notes Attachment**: The measurement is attached as a note to that specific commit object
4. **Storage**: Notes are stored in `refs/notes/perf-v3`

**Important**: Notes are attached to the **exact commit object** specified, not to HEAD or any other commit.

### Verification

To verify measurements are attached correctly, use `git notes`:

```bash
# View notes for a specific commit
git notes --ref=refs/notes/perf-v3 show <COMMIT>

# List all commits with notes
git log --show-notes=refs/notes/perf-v3
```

### Cross-Contamination Prevention

Each measurement is isolated to its target commit:

```bash
# Add different values to different commits
git perf add 100 -m test --commit commit1
git perf add 200 -m test --commit commit2
git perf add 300 -m test --commit commit3

# Each commit only has its own measurement
git perf report commit1 -o - -n 1  # Shows only 100
git perf report commit2 -o - -n 1  # Shows only 200
git perf report commit3 -o - -n 1  # Shows only 300
```

### Performance Implications

Adding measurements to non-HEAD commits:
- ✅ **No performance penalty**: Same speed as adding to HEAD
- ✅ **No checkout required**: Works without changing working directory
- ✅ **Safe operation**: Does not modify commit objects, only adds notes

### Commit History Walking

When generating reports or auditing, git-perf walks commit history from the specified commit:

```bash
# Report walks from HEAD~10 backwards
git perf report HEAD~10 -o report.html

# Can limit depth with -n flag
git perf report HEAD~10 -o report.html -n 5  # Only walk 5 commits
```

## Best Practices

### 1. Use Full SHA for Scripts

When scripting, use full commit SHAs for reliability:

```bash
# Good: Explicit and unambiguous
TARGET=$(git rev-parse feature-branch)
git perf add 100 -m test --commit $TARGET

# Avoid: Branch references can change
git perf add 100 -m test --commit feature-branch
```

### 2. Verify Commit Existence

Before adding measurements in scripts, verify the commit exists:

```bash
if git rev-parse --verify $COMMIT >/dev/null 2>&1; then
    git perf add 100 -m test --commit $COMMIT
else
    echo "Error: Commit $COMMIT does not exist"
    exit 1
fi
```

### 3. Document Non-HEAD Measurements

When adding historical data, document the reason:

```bash
# Add measurement with metadata explaining the source
git perf add 150.5 -m build_time --commit abc123 \
    -k source=ci-run-2024-01-15 \
    -k notes="backfilled from historical CI data"
```

### 4. Use Tags for Important Milestones

Tag important commits before measuring:

```bash
# Tag before measuring
git tag baseline-2024-01 abc123
git perf measure -m perf_test --commit baseline-2024-01 -- ./benchmark

# Easier to reference later
git perf report baseline-2024-01 -o report.html
```

### 5. Validate After Import

After importing to non-HEAD commits, verify the data:

```bash
# Import
git perf import junit results.xml --commit v1.0.0

# Verify
git perf report v1.0.0 -o verify.html
# Check that verify.html contains expected measurements
```

## Troubleshooting

### Error: "Could not resolve committish"

**Cause**: The specified commit reference doesn't exist or is ambiguous.

**Solution**:
```bash
# Verify commit exists
git rev-parse --verify <COMMIT>

# Use full SHA instead of partial
git perf add 100 -m test --commit $(git rev-parse abc123)
```

### Measurements Not Appearing in Reports

**Cause**: You may be reporting from the wrong commit or limiting the history walk.

**Solution**:
```bash
# Ensure you're reporting from the correct commit
git perf report <COMMIT> -o report.html

# Check if commit is in the ancestry
git log <COMMIT> --oneline | head -20

# Increase history depth if needed
git perf report <COMMIT> -o report.html -n 100
```

### Working with Shallow Clones

**Issue**: Some operations require full Git history.

**Solution**:
```bash
# Check if repository is shallow
git rev-parse --is-shallow-repository

# Unshallow if needed
git fetch --unshallow
```

### Notes Not Syncing

**Issue**: Notes aren't pushed/pulled by default.

**Solution**:
```bash
# Push notes explicitly
git perf push

# Pull notes explicitly
git perf pull
```

### Verify Notes Attachment

**Issue**: Unsure if measurement was added to the correct commit.

**Solution**:
```bash
# View raw notes for commit
git notes --ref=refs/notes/perf-v3 show <COMMIT>

# Use report with -n 1 to see only that commit's data
git perf report <COMMIT> -o - -n 1 | grep "measurement_name"
```

## Related Documentation

- **[CLI Reference](./manpage.md)** - Complete command-line options
- **[Importing Measurements](./importing-measurements.md)** - Import test and benchmark results
- **[Integration Tutorial](./INTEGRATION_TUTORIAL.md)** - GitHub Actions setup
- **[Configuration Guide](../README.md#configuration)** - Configure git-perf behavior

## See Also

- [Git Notes Documentation](https://git-scm.com/docs/git-notes)
- [Git Rev Parse](https://git-scm.com/docs/git-rev-parse)
- [Issue #517](https://github.com/kaihowl/git-perf/issues/517) - Original feature request
