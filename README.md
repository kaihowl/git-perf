# git-perf

A performance measurement tracking tool for Git repositories that stores metrics using git-notes.

## Overview

git-perf provides a comprehensive solution for tracking and analyzing performance metrics in Git repositories. It stores measurements as git-notes, enabling version-controlled performance tracking alongside your codebase.

**🔗 Live Example**: See the [example report for master branch](https://kaihowl.github.io/git-perf/master.html)

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Key Features](#key-features)
- [Audit System](#audit-system)
- [Understanding Audit Output](#understanding-audit-output)
- [Configuration](#configuration)
- [Migration](#migration)
- [Remote Setup](#remote-setup)
- [Frequently Asked Questions](#frequently-asked-questions)
- [Development](#development)
- [Documentation](#documentation)

## Installation

### Shell Installer (Recommended)

For Linux and macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/kaihowl/git-perf/releases/latest/download/git-perf-installer.sh | sh
```

### From crates.io

```bash
cargo install git-perf
```

### Pre-built Binaries

Download pre-built tarballs for your platform from the [latest release](https://github.com/kaihowl/git-perf/releases/latest):

- **Linux x86_64**: `git-perf-x86_64-unknown-linux-gnu.tar.xz`
- **Linux x86_64 (musl)**: `git-perf-x86_64-unknown-linux-musl.tar.xz`
- **Linux ARM64**: `git-perf-aarch64-unknown-linux-gnu.tar.xz`
- **macOS ARM64 (Apple Silicon)**: `git-perf-aarch64-apple-darwin.tar.xz`

All tarballs include SHA256 checksums for verification.

### From Source

```bash
git clone https://github.com/kaihowl/git-perf.git
cd git-perf
cargo install --path .
```

## Quick Start

```bash
# Add a performance measurement
git perf add build_time 42.5

# Audit for performance regressions
git perf audit -m build_time

# Push measurements to remote
git perf push
```

### Importing Test and Benchmark Results

Track test execution times and benchmark results automatically:

```bash
# Import test results (JUnit XML format)
cargo nextest run --profile ci
git perf import junit target/nextest/ci/junit.xml

# Import benchmark results (Criterion JSON format)
cargo criterion --message-format json > bench-results.json
git perf import criterion-json bench-results.json

# Audit for performance regressions
git perf audit --measurement "test::*"
git perf audit --measurement "bench::*"
```

**Supported formats:**
- **JUnit XML** - Works with cargo-nextest, pytest, Jest, JUnit, and many other test frameworks
- **Criterion JSON** - Rust benchmark results with statistical data

See the [Importing Measurements Guide](./docs/importing-measurements.md) for comprehensive documentation.

## Key Features

- **Git-notes Integration**: Store performance data alongside your Git history
- **Statistical Analysis**: Advanced audit system with configurable dispersion methods
- **Regression Detection**: Automated detection of performance changes
- **Centralized Collection**: Designed for centralized metric gathering (e.g., CI/CD)
- **Multiple Formats**: Support for data migration between format versions

## ⚠️ Important Notes

- **Experimental Status**: This tool is experimental and under active development
- **Performance Impact**: Repeated individual measurements are costly; prefer bulk additions when possible
- **Centralized Design**: Unlike Git's decentralized model, git-perf assumes centralized metric collection

## Limitations

Contrary to git itself, git-perf does not support decentralized collection of
performance measurements. Instead, git-perf assumes that there is a single,
central place for the collection of metrics. This should usually be your source
foundry, e.g., GitHub. As performance measurements become less relevant over
time, we allow metrics to be purged. As a delete in git still preserves the
history before that deletion event, we have to rewrite history. To make
rewriting of shared history safe, git-perf deliberately dropped some basic
ideas of decentralized version control and instead focuses on the collection of
metrics in a single central location.

## Migration

git-perf provides helper scripts to migrate existing performance notes between format versions.

### Available Migration Paths

| From | To | Script | Target Ref |
|------|----|---------|-----------|
| v1 | v2 | `./migration/to_v2.sh <path-to-your-repo>` | `refs/notes/perf-v2` |
| v2 | v3 | `./migration/to_v3.sh <path-to-your-repo>` | `refs/notes/perf-v3` |

### Migration Process

The migration scripts:
1. Clone the target repository into a temporary directory
2. Transform the notes to the new format
3. Commit the changes
4. Push to the appropriate notes ref on `origin`

### Post-Migration Steps

After migration, ensure consumers fetch the new notes ref:

```bash
git fetch origin refs/notes/perf-v3:refs/notes/perf-v3
```

## Remote Setup
`git-perf push`/`pull` automatically use a special remote called `git-perf-origin`.
If this remote doesn't exist, git-perf will automatically create it using your
`origin` remote's URL.

To use a different remote for performance measurements:

```bash
# Option 1: Set the git-perf-origin remote to a different URL
git remote set-url git-perf-origin git@github.com:org/repo.git

# Option 2: Add a new remote and set git-perf-origin to use it
git remote add perf-upstream git@github.com:org/repo.git
git remote set-url git-perf-origin git@github.com:org/repo.git

# Now git-perf push/pull will use the new remote
git perf push
git perf pull
```


## Audit System

git-perf includes a powerful audit system for detecting performance regressions and improvements. The system uses statistical analysis to identify meaningful performance changes while filtering out noise.

### Statistical Dispersion Methods

Choose between two statistical methods for calculating dispersion:

| Method | Description | Best For |
|--------|-------------|----------|
| **Standard Deviation (stddev)** | Traditional method, sensitive to outliers | Normally distributed data, stable measurements |
| **Median Absolute Deviation (MAD)** | Robust method, less sensitive to outliers | Data with outliers, variable environments |

### When to Use Each Method

**Standard Deviation** is ideal when:
- ✅ Performance data is normally distributed
- ✅ You want to detect all changes, including outlier-caused ones
- ✅ You have consistent, stable measurement environments

**MAD** is recommended when:
- ✅ Performance data has occasional outliers or spikes
- ✅ You want to focus on typical performance changes
- ✅ You're measuring in environments with variable system load
- ✅ You need more robust regression detection

## Configuration

Create a `.gitperfconfig` file in your repository root:

```toml
# Default settings for all measurements (parent table)
[measurement]
min_relative_deviation = 5.0
dispersion_method = "mad"  # Use MAD for all measurements by default
epoch = "00000000"  # Default epoch for performance changes

# Measurement-specific settings (override defaults)
[measurement."build_time"]
min_relative_deviation = 10.0
dispersion_method = "mad"  # Build times can have outliers, use MAD
epoch = "12345678"

[measurement."memory_usage"]
min_relative_deviation = 2.0
dispersion_method = "stddev"  # Memory usage is more stable, use stddev
epoch = "abcdef12"

[measurement."test_runtime"]
min_relative_deviation = 7.5
dispersion_method = "mad"  # Test times can vary significantly
```

### Unit Configuration

git-perf supports specifying units for measurements in your configuration. Units are displayed in audit output, HTML reports, and CSV exports:

```toml
# Default unit for all measurements (optional)
[measurement]
unit = "ms"

# Measurement-specific units (override defaults)
[measurement."build_time"]
unit = "ms"

[measurement."memory_usage"]
unit = "MB"

[measurement."throughput"]
unit = "requests/sec"

[measurement."test_runtime"]
unit = "seconds"
```

**How Units Work:**
- Units are defined in configuration and applied at display time
- Units are **not** stored with measurement data
- Measurements without configured units display normally (backward compatible)
- Units appear in:
  - **Audit output**: `✓ build_time: 42.5 ms (within acceptable range)`
  - **HTML reports**: Legend entries and axis labels show units (e.g., "build_time (ms)")
  - **CSV exports**: Dedicated unit column populated from configuration

**Example with units:**
```bash
# Configure units in .gitperfconfig
cat >> .gitperfconfig << EOF
[measurement."build_time"]
unit = "ms"
EOF

# Add measurement (no CLI change needed)
git perf add 42.5 -m build_time

# Audit shows unit automatically
git perf audit -m build_time
# Output: ✓ build_time: 42.5 ms (within acceptable range)

# Reports and CSV exports automatically include units
git perf report -o report.html -m build_time
git perf report -o data.csv -m build_time
```

### Usage Examples

```bash
# Basic audit (uses configuration or defaults to stddev)
git perf audit -m build_time

# Audit multiple measurements
git perf audit -m build_time -m memory_usage

# Custom deviation threshold
git perf audit -m build_time -d 3.0

# Override dispersion method
git perf audit -m build_time --dispersion-method mad
git perf audit -m build_time -D stddev  # Short form
```

### How It Works

The audit compares the HEAD measurement against historical measurements:
- **Z-score**: Statistical significance based on chosen dispersion method (stddev or MAD)
- **Relative deviation**: Practical significance as percentage change from median
- **Threshold**: Configurable minimum relative deviation to filter noise
- **Sparkline**: Visualizes measurement range relative to tail median (historical measurements)

### Dispersion Method Precedence

The dispersion method is determined in this order:
1. **CLI option** (`--dispersion-method` or `-D`) - highest priority
2. **Measurement-specific config** (`[measurement."name"].dispersion_method`)
3. **Default config** (`[measurement].dispersion_method`)
4. **Built-in default** (stddev) - lowest priority

### Example Output

```bash
$ git perf audit -m build_time --dispersion-method mad
✅ 'build_time'
z-score (mad): ↓ 2.15
Head: μ: 110.0 σ: 0.0 MAD: 0.0 n: 1
Tail: μ: 101.7 σ: 45.8 MAD: 2.5 n: 3
 [-1.0% – +96.0%] ▁▃▇▁
```

When the relative deviation is below the threshold, the audit passes even if
the z-score indicates statistical significance. This helps focus on meaningful
performance changes while ignoring noise.

## Understanding Audit Output

The audit system provides detailed statistical analysis of your performance measurements. Here's a complete example followed by a breakdown of each component:

### Complete Example

```bash
$ git perf audit -m build_time --dispersion-method mad

✅ 'build_time'
z-score (mad): ↓ 2.15
Head: μ: 110.0 σ: 0.0 MAD: 0.0 n: 1
Tail: μ: 101.7 σ: 45.8 MAD: 2.5 n: 3
 [-1.0% – +96.0%] ▁▃▇▁
```

This output shows a **passing audit** where the current build time (110.0) is being compared against 3 historical measurements. Let's break down each component:

### 1. Status Indicator

```
✅ 'build_time'
```

The first line shows the audit result:

- **✅ 'measurement_name'** - Audit passed (no significant regression detected)
- **❌ 'measurement_name'** - Audit failed (significant performance change detected)
- **⏭️ 'measurement_name'** - Audit skipped (insufficient measurements)

### 2. Z-Score and Direction

```
z-score (mad): ↓ 2.15
```

- **z-score**: `2.15` - Statistical measure of how many MADs (or standard deviations) the HEAD measurement is from the tail mean
  - Higher values indicate more significant deviations
  - Typically, z-scores above 4.0 (default sigma) indicate statistical significance
- **Direction arrows**:
  - **↑** - HEAD measurement is higher than tail average (potential regression for time metrics)
  - **↓** - HEAD measurement is lower than tail average (potential improvement for time metrics)
  - **→** - HEAD measurement is roughly equal to tail average
- **Method indicator**: `(mad)` - Shows which dispersion method was used (`stddev` or `mad`)

### 3. Statistical Summary

```
Head: μ: 110.0 σ: 0.0 MAD: 0.0 n: 1
Tail: μ: 101.7 σ: 45.8 MAD: 2.5 n: 3
```

- **Head**: Statistics for the current commit's measurement(s)
  - In this example: single measurement of 110.0
- **Tail**: Statistics for historical measurements
  - In this example: 3 measurements with mean of 101.7
- **μ (mu)**: Mean (average) value
- **σ (sigma)**: Standard deviation (measure of variability)
- **MAD**: Median Absolute Deviation (robust measure of variability)
- **n**: Number of measurements used in the calculation

### 4. Sparkline Visualization

```
 [-1.0% – +96.0%] ▁▃▇▁
```

- **Percentage range**: `[-1.0% – +96.0%]` - Shows min and max measurements relative to the tail median
  - `-1.0%` means the lowest measurement is 1% below the tail median
  - `+96.0%` means the highest measurement is 96% above the tail median
  - In this example, there's significant variation with one outlier
- **Sparkline**: `▁▃▇▁` - Visual representation of all measurements (tail + head)
  - Each bar represents a measurement's relative magnitude
  - Bars range from ▁ (lowest) to █ (highest)
  - Here: low value, medium value, very high outlier, another low value
  - Helps quickly identify outliers and trends

### 5. Threshold Notes (Optional)

When configured with `min_relative_deviation`, you may see:

```
Note: Passed due to relative deviation (3.2%) being below threshold (5.0%)
```

This indicates the audit passed because the performance change was below the configured threshold, even though it may have been statistically significant. This prevents false alarms from minor fluctuations.

### 6. Skipped Audits

When there aren't enough measurements:

```
⏭️ 'build_time'
Only 3 measurements found. Less than requested min_measurements of 10. Skipping test.
 [-2.5% – +5.1%] ▃▇▁▅
```

The audit is skipped but still shows the sparkline for available data. Adjust `--min-measurements` to change the requirement.

### 7. Failed Audit Example

When a regression is detected:

```
❌ 'build_time'
HEAD differs significantly from tail measurements.
z-score (stddev): ↑ 5.23
Head: μ: 250.0 σ: 0.0 MAD: 0.0 n: 1
Tail: μ: 100.0 σ: 15.2 MAD: 8.3 n: 10
 [-12.5% – +150.0%] ▃▅▄▆▅▄▅▄▅█
```

This shows build time increased from ~100 to 250 (150% increase) with high statistical significance (z-score of 5.23).

### Quick Interpretation Guide

**Audit Passed (✅)**:
- Performance is stable or improved
- Any changes are within acceptable thresholds
- Safe to merge/deploy

**Audit Failed (❌)**:
- Significant performance regression detected
- Review code changes that may have caused the regression
- Consider optimization or investigation before merging

**Audit Skipped (⏭️)**:
- Not enough historical data for statistical analysis
- Continue collecting measurements
- Results will become more reliable over time

## Frequently Asked Questions

### General Usage

#### What are the system requirements for git-perf?

- **Git**: Version 2.43.0 or higher
- **Platforms**: Linux (x86_64, ARM64), macOS (ARM64/Apple Silicon)
- **For building from source**: Rust toolchain (latest stable)

#### Can I use git-perf with a private repository?

Yes! git-perf works with both public and private repositories. For custom remote setups:

```bash
# Set up a custom remote for measurements
git remote add perf-upstream git@github.com:org/private-repo.git
git remote set-url git-perf-origin git@github.com:org/private-repo.git
```

See the [Remote Setup](#remote-setup) section for details.

#### Is there a performance impact when using git-perf?

Measurement operations have minimal overhead:
- **Adding measurements**: Individual `add` commands are slower than bulk operations
- **Recommendation**: Use `git perf measure` for automatic timing or bulk additions when possible
- **Push/pull**: Operations are efficient and similar to git-notes operations

For CI/CD usage, the overhead is typically negligible compared to build times.

### Configuration & Units

#### Why aren't units stored with measurement data?

git-perf uses a **configuration-only approach** for units rather than storing them with each measurement. This design decision provides several advantages:

**Benefits:**
- **Simplicity**: No changes to data model or serialization format
- **Zero risk**: Perfect backward compatibility with existing measurements
- **Centralized management**: Single source of truth in `.gitperfconfig`
- **Flexibility**: Units can be updated without re-recording measurements
- **No storage overhead**: No additional bytes per measurement

**Trade-offs:**
- **No per-measurement validation**: Can't detect if measurements were recorded in different units
- **Manual consistency**: Users must ensure config units match actual measurement units
- **User responsibility**: Changing unit config doesn't change values - config must accurately reflect how measurements were recorded

**Best practices:**
- Choose appropriate units when starting measurements and document them in config
- Use consistent naming conventions (e.g., `build_time_ms` makes the unit clear)
- Keep unit configuration stable once established

This approach matches git-perf's configuration philosophy where display settings (like `dispersion_method`) are config-based. It provides 80% of the value (clear report display) with 20% of the complexity, and can be extended later if validation becomes important.

#### How do I migrate existing measurements to use units?

No migration needed! Simply add unit configuration to your `.gitperfconfig`:

```toml
[measurement."your_metric"]
unit = "ms"
```

Existing measurements will automatically display with units in all output (audit, reports, CSV exports). The configuration is applied at display time, so it works retroactively with all historical measurements.

#### Can I use different units for the same measurement over time?

While technically possible by changing the configuration, this is **not recommended**. Units reflect how measurements were actually recorded. If you change from recording milliseconds to seconds, you should:

1. Create a new measurement name (e.g., `build_time_sec` instead of `build_time_ms`)
2. Update your configuration with the new unit
3. Use the new measurement name going forward

This ensures clarity and prevents confusion when analyzing historical data.

### Audit & Regression Detection

#### Why is my audit reporting false positives?

If audits detect regressions for normal variations, try these solutions:

1. **Increase relative deviation threshold** in `.gitperfconfig`:
   ```toml
   [measurement."build_time"]
   min_relative_deviation = 10.0  # Percentage change required
   ```

2. **Switch to MAD for more robust detection**:
   ```toml
   [measurement."build_time"]
   dispersion_method = "mad"  # Less sensitive to outliers
   ```

3. **Increase sigma threshold** via CLI:
   ```bash
   git perf audit -m build_time -d 6.0  # Default is 4.0
   ```

#### How do I choose between stddev and MAD?

**Use Standard Deviation (stddev)** when:
- Your performance data is normally distributed
- You want to detect all changes, including outlier-caused ones
- You have consistent, stable measurement environments

**Use MAD (Median Absolute Deviation)** when:
- Performance data has occasional outliers or spikes
- You want to focus on typical performance changes
- You're measuring in environments with variable system load
- You need more robust regression detection

See the [Audit System](#audit-system) section for complete details.

#### What's the minimum number of measurements needed before auditing?

By default, git-perf requires at least 10 measurements (configurable with `--min-measurements`). With fewer measurements:
- Audits are skipped with a message
- Sparkline is still shown for available data
- Statistical analysis becomes more reliable as more data is collected

```bash
# Adjust minimum required measurements
git perf audit -m build_time --min-measurements 5
```

#### What does the sparkline visualization show?

The sparkline (e.g., `▁▃▇▁`) shows:
- **Bars**: Each represents a measurement's relative magnitude
- **Height**: From ▁ (lowest) to █ (highest)
- **Percentage range**: Shows min/max measurements relative to tail median
- **Purpose**: Quickly identify outliers, trends, and distribution

Example: `[-1.0% – +96.0%] ▁▃▇▁` shows most values are low, with one significant outlier.

### Data Management

#### Why does the size command show a warning about shallow clones?

When running `git perf size` in a shallow clone, you'll see:

```
⚠️  Shallow clone detected - measurement counts may be incomplete (see FAQ)
```

**What this means:**
- The measurement counts for commits in your current lineage are accurate
- However, measurements for commits outside your shallow clone's history are not counted
- The `git notes list` command only sees notes for locally available commits

**Example:**
If your full repository has 1000 commits with 800 measurements, but your shallow clone only has 100 commits, you might see only ~80 measurements instead of the full 800.

**Important distinction:**
- ✅ **Measurements in your lineage**: Correctly captured and counted
- ❌ **Measurements outside your lineage**: Not visible in shallow clones (e.g., measurements on branches not in your history)

**To see all measurements:**
```bash
git fetch --unshallow
```

This converts your shallow clone to a full clone, allowing all measurements to be counted.

#### How long should I keep measurement data?

**Recommended**: At least 90 days of measurement data for meaningful trend analysis.

Configure cleanup retention:
```yaml
# In .github/workflows/cleanup-measurements.yml
- uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    retention-days: 90  # Days to retain measurements
```

Adjust based on your needs:
- **Active development**: 90-180 days
- **Stable projects**: 180-365 days
- **Long-term analysis**: 365+ days

#### Can I track the same metric across different environments?

Yes! Use key-value pairs to track multi-environment measurements:

```bash
# Record with environment tag
git perf measure -m build_time -k env=dev -- cargo build
git perf measure -m build_time -k env=prod -- cargo build --release

# Filter by environment
git perf report -m build_time -k env=dev
git perf audit -m build_time -s env=prod
```

### GitHub Actions Integration

#### Why does push fail in GitHub Actions?

Common causes and solutions:

1. **Missing permissions**:
   ```yaml
   permissions:
     contents: write  # Required for git perf push
   ```

2. **Insufficient fetch depth**:
   ```yaml
   - uses: actions/checkout@v4
     with:
       fetch-depth: 0  # Fetch full history
   ```

3. **Protected branch**: Add exception for GitHub Actions in branch protection settings

#### How do I integrate audit results into my CI/CD pipeline?

Use the report action for automatic PR comments:

```yaml
- name: Generate report with audit
  uses: kaihowl/git-perf/.github/actions/report@master
  with:
    depth: 40
    audit-args: '-m build_time -m binary_size -d 4.0'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

The action will:
- Run audits automatically
- Comment on PRs with results
- Fail the workflow if regressions are detected

### Troubleshooting

#### Can I export measurements in different formats?

Yes, git-perf supports CSV export using the report command:

```bash
# Export to CSV file
git perf report -m build_time -o data.csv

# Export to stdout
git perf report -m build_time -o -

# Export with aggregation (e.g., mean)
git perf report -m build_time -a mean -o data.csv
```

Units configured in `.gitperfconfig` automatically appear in the CSV output.

## Documentation

### Available Documentation

- **[Importing Measurements Guide](./docs/importing-measurements.md)** - Import test and benchmark results
- **[Manpages](./docs/manpage.md)** - Complete CLI reference
- **[Integration Tutorial](./docs/INTEGRATION_TUTORIAL.md)** - GitHub Actions integration guide
- **[Evaluation Tools](./evaluation/README.md)** - Statistical method comparison tools

### Generating Documentation

Documentation is automatically generated using `clap_mangen` and `clap_markdown`:

```bash
# Generate with default version (0.0.0)
./scripts/generate-manpages.sh

# Generate with custom version
GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh
```

#### Output Locations

- **Manpages**: `target/man/man1/`
- **Markdown**: `docs/manpage.md`

The CI automatically validates that documentation stays current with CLI definitions.

## Development

### Prerequisites

Install required development dependencies:

```bash
# macOS
brew install libfaketime

# Ubuntu/Debian
sudo apt-get install libfaketime
```

### Development Setup

For contributors, run the setup script to install development tools:

```bash
# Install cargo-nextest and other development tools
./scripts/setup-dev-tools.sh
```

This script will install `cargo-nextest` if not already present, enabling faster test execution in your local development environment.

### Testing

This project uses [nextest](https://nexte.st/) for faster, more reliable test execution.

#### Quick Commands

```bash
# Development testing (recommended - skips slow tests)
cargo nextest run -- --skip slow

# Full test suite
cargo nextest run

# Specific test pattern
cargo nextest run --test-pattern "git_interop"

# Verbose output
cargo nextest run --verbose

# Specific package
cargo nextest run -p git-perf
```

#### Alternative: Standard Cargo Tests

```bash
# All tests
cargo test

# Skip slow tests
cargo test -- --skip slow
```

### Code Quality

Before submitting changes, ensure code quality:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy
```
