# Integration Tutorial: Adding git-perf to Your Project

This tutorial walks you through integrating git-perf into your GitHub project for automated performance tracking, regression detection, and reporting.

## Prerequisites

- A Git repository (local or on GitHub)
- Git version 2.43.0 or higher
- For GitHub Actions integration: A GitHub repository with Actions enabled
- Basic familiarity with Git and YAML (for GitHub Actions setup)

## Step 1: Install git-perf Locally

See the [Installation section in the README](../README.md#installation) for complete installation instructions, including:
- Shell installer (recommended)
- Installing from crates.io
- Pre-built binaries
- Building from source

Verify installation:

```bash
git perf --version
```

## Step 2: Add Initial Measurements

### Create Your First Measurement

Navigate to your project repository and add a measurement:

```bash
cd /path/to/your/project

# Add a measurement (e.g., build time in seconds)
# You'll typically get this value from your build or test process
git perf add -m build_time 42.5

# View measurements in a report
git perf report
```

### Configure Measurement Settings

Create a `.gitperfconfig` file in your repository root. See the [Configuration section in the README](../README.md#configuration) for all available options.

Example configuration:

```toml
# Default settings for all measurements
[measurement]
min_relative_deviation = 5.0
dispersion_method = "mad"
unit = "ms"  # Default unit for all measurements

# Measurement-specific settings
[measurement."build_time"]
min_relative_deviation = 10.0
dispersion_method = "mad"
unit = "seconds"  # Override default unit for build_time

[measurement."binary_size"]
min_relative_deviation = 2.0
dispersion_method = "stddev"
unit = "bytes"

[measurement."test_duration"]
unit = "ms"
```

**Unit Configuration:**
Units are displayed in audit output, HTML reports, and CSV exports. They help make your performance data more readable and professional:
- Configure units for each measurement in `.gitperfconfig`
- Units are applied at display time (not stored with measurement data)
- Existing measurements automatically display with units once configured
- Measurements without units continue to work normally (backward compatible)

### Commit the Configuration

```bash
git add .gitperfconfig
git commit -m "chore: add git-perf configuration"
```

## Step 3: Configure GitHub Actions

### Install Action

Create a workflow that uses the git-perf install action to measure your builds automatically.

Create `.github/workflows/performance-tracking.yml`:

```yaml
name: Performance Tracking

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

jobs:
  measure-performance:
    runs-on: ubuntu-latest
    permissions:
      contents: write  # Required to push measurements

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Fetch all history for git-perf

      # Install git-perf
      - name: Install git-perf
        uses: kaihowl/git-perf/.github/actions/install@master
        with:
          version: latest

      # Example: Measure build time using the measure command
      - name: Build project and measure
        run: |
          git perf measure -m build_time -- cargo build --release

      # Example: Measure binary size
      - name: Measure binary size
        run: |
          binary_size=$(stat -c%s target/release/your-binary)
          git perf add -m binary_size "$binary_size"

      # Push measurements back to the repository
      - name: Push measurements
        run: git perf push
```

**Important Notes:**
- The `fetch-depth: 0` fetches full history; you can use a specific depth (e.g., `fetch-depth: 50`) matching the `-n` flag used in `report` or `audit` commands
- The `contents: write` permission is needed to push measurement data
- The `git perf measure` command automatically times the execution of the supplied command
- Push is unconditional to ensure measurements are always saved

## Step 4: Set Up Automatic Reporting

### Generate HTML Reports

The report generation happens as a step in your workflow using the report action. Update your workflow to include report generation:

```yaml
# Update your .github/workflows/performance-tracking.yml

jobs:
  measure-and-report:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pages: write
      pull-requests: write  # Required for PR comments
    concurrency:
      group: gh-pages-${{ github.ref }}
      cancel-in-progress: false  # Let jobs queue to prevent conflicts
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install git-perf
        uses: kaihowl/git-perf/.github/actions/install@master
        with:
          version: latest

      - name: Build project and measure
        run: git perf measure -m build_time -- cargo build --release

      - name: Measure binary size
        run: |
          binary_size=$(stat -c%s target/release/your-binary)
          git perf add -m binary_size "$binary_size"

      - name: Push measurements
        run: git perf push

      - name: Generate performance report
        uses: kaihowl/git-perf/.github/actions/report@master
        with:
          depth: 40
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Enable GitHub Pages

1. Go to your repository settings
2. Navigate to "Pages" in the left sidebar
3. Under "Source", select the `gh-pages` branch
4. Click "Save"
5. Your report will be available at `https://<username>.github.io/<repository>/`

## Step 5: Configure Measurement Cleanup

To prevent your repository from growing indefinitely with measurement data, use the cleanup action:

Create `.github/workflows/cleanup-measurements.yml`:

```yaml
name: Cleanup Old Measurements

on:
  schedule:
    # Run weekly on Sundays at 2 AM UTC
    - cron: '0 2 * * 0'
  workflow_dispatch:  # Allow manual triggering

jobs:
  cleanup:
    runs-on: ubuntu-latest
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Cleanup measurements and reports
        uses: kaihowl/git-perf/.github/actions/cleanup@master
        with:
          retention-days: 90
          cleanup-reports: true
          reports-retention-days: 30
```

**Configuration Options:**
- `retention-days`: How long to keep measurement data (default: 90 days)
- `cleanup-reports`: Whether to also cleanup old reports (default: true)
- `reports-retention-days`: How long to keep reports (default: 30 days)
- `dry-run`: Preview what would be deleted without actually deleting (default: false)

## Step 6: Enable Regression Detection with Audit

### Configure Audit Thresholds

Configure audit settings in your `.gitperfconfig`. See the [Audit System section in the README](../README.md#audit-system) for complete details.

```toml
# Default settings
[measurement]
min_relative_deviation = 5.0
dispersion_method = "mad"

# Measurement-specific settings
[measurement."build_time"]
min_relative_deviation = 10.0
dispersion_method = "mad"

[measurement."binary_size"]
min_relative_deviation = 2.0
dispersion_method = "stddev"
```

### Add Audit Step to CI

Update your workflow to run audits and fail on regressions. You can also integrate audit into the report action:

```yaml
# Option 1: Add audit as a separate step
- name: Run audit for regressions
  run: |
    # Pull latest measurements to ensure we have historical data
    git perf pull || true

    # Run audit for specific measurements
    git perf audit -m build_time
    git perf audit -m binary_size

# Option 2: Integrate audit with report generation
- name: Generate report with audit
  uses: kaihowl/git-perf/.github/actions/report@master
  with:
    depth: 40
    audit-args: '-m build_time -m binary_size -d 4.0 --min-measurements 5'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

The report action will automatically comment on pull requests with both the report URL and audit results.

## Step 7: Advanced Configuration

### Custom Statistical Methods

git-perf supports multiple dispersion methods for regression detection. See the [Audit System section in the README](../README.md#audit-system) for details on choosing between `stddev` and `mad`.

Configure per measurement:

```toml
[measurement."build_time"]
dispersion_method = "mad"  # Robust to outliers

[measurement."memory_usage"]
dispersion_method = "stddev"  # More sensitive
```

Or override via CLI:

```bash
git perf audit -m build_time --dispersion-method mad
git perf audit -m build_time -D stddev  # Short form
```

### Multi-Environment Tracking

Track measurements across different environments using key-value pairs:

```yaml
# In your workflow
- name: Measure performance (development)
  run: |
    git perf measure -m build_time -k env=dev -- cargo build

- name: Measure performance (production)
  run: |
    git perf measure -m build_time -k env=prod -- cargo build --release
```

Filter in reports:

```bash
git perf report -m build_time -k env=dev
git perf report -m build_time -k env=prod
```

Audit specific environments:

```bash
# Audit development environment only
git perf audit -m build_time -s env=dev

# Audit production environment only
git perf audit -m build_time -s env=prod
```

## Troubleshooting

### Issue: Push Fails in GitHub Actions

**Symptom**: `git perf push` fails with authentication errors

**Solutions**:
1. Ensure `contents: write` permission is set in the workflow
2. Use `fetch-depth: 0` when checking out the repository
3. Verify the branch is not protected (or add exception for Actions)

### Issue: Audit Fails Unexpectedly

**Symptom**: Audit reports regressions for normal variations

**Solutions**:
1. Increase thresholds in `.gitperfconfig`:
   ```toml
   [measurement."build_time"]
   min_relative_deviation = 10.0  # Percentage (0..100), unset by default
   ```
2. Use MAD instead of stddev for more robust detection:
   ```toml
   [measurement."build_time"]
   dispersion_method = "mad"
   ```
3. Increase sigma threshold via CLI:
   ```bash
   git perf audit -m build_time -d 6.0  # Default is 4.0
   ```

### Issue: Cleanup Deleting Too Much Data

**Symptom**: Important historical data is being removed

**Solutions**:
1. Increase retention days in cleanup workflow:
   ```yaml
   retention-days: 180  # Keep 6 months
   ```
2. Use dry-run mode first:
   ```yaml
   dry-run: true
   ```
3. Disable report cleanup if needed:
   ```yaml
   cleanup-reports: false
   ```

## Best Practices

### 1. Measurement Granularity

**Do:**
- Measure discrete, meaningful metrics (build time, test duration, binary size)
- Configure units in `.gitperfconfig` for clarity
- Use consistent units across related measurements
- Measure on the same hardware/environment for comparability

**Don't:**
- Measure every small operation (too much noise)
- Change units for the same measurement over time (creates confusion)
- Mix units (record bytes but configure as "MB", or vice versa)
- Measure on different runner types without noting the environment

**Unit Configuration Example:**
```toml
[measurement."build_time"]
unit = "seconds"

[measurement."binary_size"]
unit = "bytes"

[measurement."memory_peak"]
unit = "MB"
```

Units will automatically appear in:
- Audit output: `✓ build_time: 42.5 seconds (within acceptable range)`
- HTML reports: Legend entries like "build_time (seconds)"
- CSV exports: Dedicated unit column with configured units

### 2. Audit Configuration

**Do:**
- Start with lenient thresholds and tighten over time
- Use MAD for more stable metrics
- Require multiple samples before auditing (min_samples ≥ 5)

**Don't:**
- Set thresholds too tight initially (causes false positives)
- Audit metrics with high natural variation
- Fail CI on audit failures without investigation

### 3. Data Retention

**Do:**
- Keep at least 90 days of measurement data
- Archive old reports separately if needed
- Schedule cleanup during low-traffic times

**Don't:**
- Delete all historical data (defeats trending analysis)
- Run cleanup too frequently (weekly is usually enough)
- Skip dry-runs before production cleanup

### 4. Workflow Organization

**Do:**
- Push measurements before generating reports
- Use the report action for automated reporting and PR comments
- Include concurrency control to prevent gh-pages conflicts
- Use workflow dispatch for manual triggers

**Don't:**
- Skip the push step (measurements won't be saved)
- Skip permissions declarations
- Forget concurrency control when publishing to gh-pages

## Example Real-World Workflow

Here's a complete, production-ready workflow combining all best practices:

```yaml
name: Performance CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 0 * * 0'  # Weekly reports

jobs:
  measure-and-report:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pages: write
      pull-requests: write
    concurrency:
      group: gh-pages-${{ github.ref }}
      cancel-in-progress: false
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: kaihowl/git-perf/.github/actions/install@master

      - name: Build and measure
        run: |
          git perf measure -m build_time -- cargo build --release

          size=$(stat -c%s target/release/my-app)
          git perf add -m binary_size "$size"

      - name: Test and measure
        run: git perf measure -m test_duration -- cargo test --release

      - name: Push measurements
        run: git perf push

      - name: Generate report with audit
        uses: kaihowl/git-perf/.github/actions/report@master
        with:
          depth: 40
          audit-args: '-m build_time -m binary_size -m test_duration -d 4.0 --min-measurements 5'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

## Next Steps

- Customize `.gitperfconfig` for your project's specific metrics
- Set up GitHub Pages to view your performance dashboard
- Configure audit thresholds based on your team's tolerance for variation
- Schedule regular cleanup to maintain repository size
- Share your performance dashboard URL with your team

## Additional Resources

- [git-perf README](../README.md) - Full feature documentation
- [Command Reference](./manpage.md) - Complete CLI documentation
- [Configuration Guide](../README.md#configuration) - Detailed `.gitperfconfig` options
- [Audit System](../README.md#audit-system) - Statistical methods and regression detection
- [GitHub Actions](../.github/actions/) - Reusable actions: [install](../.github/actions/install/), [report](../.github/actions/report/), [cleanup](../.github/actions/cleanup/)
- [Example Report](https://kaihowl.github.io/git-perf/master.html) - Live performance dashboard

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/kaihowl/git-perf/issues)
- **Discussions**: [GitHub Discussions](https://github.com/kaihowl/git-perf/discussions)
- **Documentation**: Check the `docs/` directory for detailed guides
