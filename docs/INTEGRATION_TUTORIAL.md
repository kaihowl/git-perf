# Integration Tutorial: Adding git-perf to Your Project

This tutorial walks you through integrating git-perf into your GitHub project for automated performance tracking, regression detection, and reporting.

## Prerequisites

- A Git repository (local or on GitHub)
- Git version 2.0 or higher
- For GitHub Actions integration: A GitHub repository with Actions enabled
- Basic familiarity with Git and YAML (for GitHub Actions setup)

## Step 1: Install git-perf Locally

Choose one of the following installation methods:

### Option A: Install from crates.io (Recommended)

```bash
cargo install git-perf
```

### Option B: Install from Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/terragonlabs/git-perf/releases):

```bash
# Example for Linux
curl -L https://github.com/terragonlabs/git-perf/releases/latest/download/git-perf-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv git-perf /usr/local/bin/
```

### Option C: Build from Source

```bash
git clone https://github.com/terragonlabs/git-perf.git
cd git-perf
cargo install --path .
```

### Verify Installation

```bash
git-perf --version
```

## Step 2: Add Initial Measurements

### Create Your First Measurement

Navigate to your project repository and add a measurement:

```bash
cd /path/to/your/project

# Measure a specific metric (e.g., build time)
# You'll typically get this value from your build or test process
git-perf measure build_time 42.5 --unit seconds

# Verify the measurement was recorded
git-perf list
```

### Configure Measurement Metadata

Create a `.gitperfconfig` file in your repository root to customize how measurements are displayed:

```toml
# .gitperfconfig
[measurements.build_time]
name = "Build Time"
unit = "seconds"
description = "Time to compile the entire project"
lower_is_better = true

[measurements.test_duration]
name = "Test Suite Duration"
unit = "seconds"
description = "Time to run all tests"
lower_is_better = true

[measurements.binary_size]
name = "Binary Size"
unit = "bytes"
description = "Size of release binary"
lower_is_better = true
```

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
        uses: terragonlabs/git-perf/.github/actions/install@master
        with:
          version: latest

      # Example: Measure build time
      - name: Build project and measure
        run: |
          # Start timing
          start_time=$(date +%s.%N)

          # Your build command
          cargo build --release

          # Calculate duration
          end_time=$(date +%s.%N)
          duration=$(echo "$end_time - $start_time" | bc)

          # Record measurement
          git-perf measure build_time "$duration" --unit seconds

      # Example: Measure binary size
      - name: Measure binary size
        run: |
          binary_size=$(stat -c%s target/release/your-binary)
          git-perf measure binary_size "$binary_size" --unit bytes

      # Push measurements back to the repository
      - name: Push measurements
        if: github.event_name == 'push'
        run: |
          git-perf push
```

**Important Notes:**
- The `fetch-depth: 0` is required so git-perf has access to the full git history
- The `contents: write` permission is needed to push measurement data
- Only push measurements from the main branch (not from PRs) to avoid conflicts

## Step 4: Set Up Automatic Reporting

### Generate HTML Reports

Add a job to generate visual reports of your performance data:

```yaml
# Add this job to your .github/workflows/performance-tracking.yml

  generate-report:
    runs-on: ubuntu-latest
    needs: measure-performance
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    permissions:
      contents: write
      pages: write

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install git-perf
        uses: terragonlabs/git-perf/.github/actions/install@master
        with:
          version: latest

      - name: Pull latest measurements
        run: git-perf pull

      - name: Generate report
        run: |
          mkdir -p reports
          git-perf report --output reports/index.html --format html

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./reports
          publish_branch: gh-pages
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
        uses: terragonlabs/git-perf/.github/actions/cleanup@master
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

Add audit configuration to your `.gitperfconfig`:

```toml
[audit]
enabled = true
stddev_threshold = 2.0  # Alert if metric deviates by 2 standard deviations
mad_threshold = 3.0     # Alert if metric deviates by 3 MAD (more robust)
min_samples = 5         # Need at least 5 samples before auditing

[measurements.build_time]
name = "Build Time"
unit = "seconds"
lower_is_better = true
audit = true  # Enable auditing for this measurement

[measurements.binary_size]
name = "Binary Size"
unit = "bytes"
lower_is_better = true
audit = true
```

### Add Audit Step to CI

Update your workflow to run audits and fail on regressions:

```yaml
# Add this step after measuring performance
- name: Run audit for regressions
  run: |
    # Pull latest measurements to ensure we have historical data
    git-perf pull || true

    # Run audit
    if ! git-perf audit --format github; then
      echo "⚠️ Performance regression detected!"
      exit 1
    fi
```

The `--format github` option outputs results in a format that GitHub Actions will display nicely in the workflow logs.

## Step 7: Advanced Configuration

### Custom Statistical Methods

git-perf supports multiple dispersion methods for regression detection:

```toml
[audit]
enabled = true
dispersion_method = "mad"  # Options: "stddev", "mad", "both"
```

- **stddev**: Standard deviation (sensitive to outliers)
- **mad**: Median Absolute Deviation (robust to outliers)
- **both**: Use both methods (more conservative)

### Measurement Categories

Organize measurements into categories:

```toml
[measurements.cpu_benchmark]
name = "CPU Benchmark"
unit = "ops/sec"
category = "performance"
lower_is_better = false

[measurements.memory_usage]
name = "Memory Usage"
unit = "MB"
category = "resources"
lower_is_better = true
```

### Multi-Environment Tracking

Track measurements across different environments:

```yaml
# In your workflow
- name: Measure performance (development)
  run: |
    git-perf measure build_time_dev "$duration" --unit seconds

- name: Measure performance (production)
  run: |
    git-perf measure build_time_prod "$duration" --unit seconds
```

Configure separately in `.gitperfconfig`:

```toml
[measurements.build_time_dev]
name = "Build Time (Dev)"
unit = "seconds"
environment = "development"

[measurements.build_time_prod]
name = "Build Time (Prod)"
unit = "seconds"
environment = "production"
```

## Troubleshooting

### Issue: Measurements Not Appearing

**Symptom**: `git-perf list` shows no measurements

**Solutions**:
1. Verify git-perf is installed: `git-perf --version`
2. Check you're in a git repository: `git status`
3. Ensure measurements were committed: `git log --notes=perf/v3`
4. Try pulling measurements: `git-perf pull`

### Issue: Push Fails in GitHub Actions

**Symptom**: `git-perf push` fails with authentication errors

**Solutions**:
1. Ensure `contents: write` permission is set in the workflow
2. Use `fetch-depth: 0` when checking out the repository
3. Verify the branch is not protected (or add exception for Actions)

### Issue: Audit Fails Unexpectedly

**Symptom**: Audit reports regressions for normal variations

**Solutions**:
1. Increase thresholds in `.gitperfconfig`:
   ```toml
   [audit]
   stddev_threshold = 3.0  # More lenient
   mad_threshold = 4.0
   ```
2. Use MAD instead of stddev for more robust detection:
   ```toml
   [audit]
   dispersion_method = "mad"
   ```
3. Increase minimum samples:
   ```toml
   [audit]
   min_samples = 10
   ```

### Issue: Reports Not Generating

**Symptom**: `git-perf report` produces empty or incomplete reports

**Solutions**:
1. Pull measurements first: `git-perf pull`
2. Verify measurements exist: `git-perf list`
3. Check `.gitperfconfig` format: `git-perf config --validate`
4. Generate with verbose output: `git-perf report -v`

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
- Use consistent units across related measurements
- Measure on the same hardware/environment for comparability

**Don't:**
- Measure every small operation (too much noise)
- Mix units (use bytes, not "KB" or "MB" strings)
- Measure on different runner types without noting the environment

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
- Separate measurement collection from reporting
- Only push from protected branches
- Use workflow dispatch for manual triggers

**Don't:**
- Push measurements from PR builds (creates conflicts)
- Generate reports on every commit (once per merge is enough)
- Skip permissions declarations

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
  measure:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: terragonlabs/git-perf/.github/actions/install@master

      - name: Build and measure
        run: |
          start=$(date +%s.%N)
          cargo build --release
          duration=$(echo "$(date +%s.%N) - $start" | bc)
          git-perf measure build_time "$duration" --unit seconds

          size=$(stat -c%s target/release/my-app)
          git-perf measure binary_size "$size" --unit bytes

      - name: Test and measure
        run: |
          start=$(date +%s.%N)
          cargo test --release
          duration=$(echo "$(date +%s.%N) - $start" | bc)
          git-perf measure test_duration "$duration" --unit seconds

      - name: Audit
        run: git-perf audit --format github

      - name: Push measurements
        if: github.event_name == 'push'
        run: git-perf push

  report:
    needs: measure
    if: github.event_name == 'push' || github.event_name == 'schedule'
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pages: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: terragonlabs/git-perf/.github/actions/install@master

      - run: git-perf pull

      - run: |
          mkdir -p reports
          git-perf report --output reports/index.html --format html

      - uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./reports
```

## Next Steps

- Customize `.gitperfconfig` for your project's specific metrics
- Set up GitHub Pages to view your performance dashboard
- Configure audit thresholds based on your team's tolerance for variation
- Schedule regular cleanup to maintain repository size
- Share your performance dashboard URL with your team

## Additional Resources

- [git-perf README](../README.md) - Full feature documentation
- [Configuration Guide](../README.md#configuration) - Detailed `.gitperfconfig` options
- [GitHub Actions](../.github/actions/) - Reusable actions reference
- [Example Report](https://terragonlabs.github.io/git-perf/) - Live performance dashboard
- [Statistical Methods](../docs/statistical-comparison.md) - Understanding stddev vs MAD

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/terragonlabs/git-perf/issues)
- **Discussions**: [GitHub Discussions](https://github.com/terragonlabs/git-perf/discussions)
- **Documentation**: Check the `docs/` directory for detailed guides
