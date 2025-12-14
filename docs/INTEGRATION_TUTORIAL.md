# Integration Tutorial: Adding git-perf to Your Project

This tutorial walks you through integrating git-perf into your GitHub project for automated performance tracking, regression detection, and reporting.

## Prerequisites

- A Git repository (local or on GitHub)
- Git version 2.43.0 or higher
- For GitHub Actions integration: A GitHub repository with Actions enabled
- Basic familiarity with Git and YAML (for GitHub Actions setup)

### Verify Git Version

Check your Git version:

```bash
git --version
# Should output: git version 2.43.0 or higher
```

If you need to upgrade Git:

**Ubuntu/Debian:**
```bash
sudo add-apt-repository ppa:git-core/ppa
sudo apt update && sudo apt install git
```

**macOS:**
```bash
brew upgrade git
```

**Windows:**
Download the latest version from [git-scm.com](https://git-scm.com/download/win)

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

# Generate an HTML report (creates output.html by default)
git perf report

# Or specify a custom output location
git perf report -o my-report.html
```

**Note**: The `git perf report` command generates an HTML file (default: `output.html`) and produces no terminal output. Open the HTML file in a browser to view your performance data with interactive charts.

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

### Verification

Verify your local setup is working correctly:

```bash
# Check that measurements were recorded
git notes --ref=refs/notes/perf-v3 list

# Verify the measurement data in git notes
git log --show-notes=refs/notes/perf-v3 --oneline -1

# Generate and view a report
git perf report -o test-report.html
# Open test-report.html in your browser to verify the report displays correctly
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
          release: latest

      # Configure git identity (required for git-perf to commit measurements)
      - name: Configure git identity
        run: |
          git config --global user.email "actions@github.com"
          git config --global user.name "GitHub Actions"

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
- **Git identity configuration is required** because git-perf stores measurements as git-notes (git commits)
- The `git perf measure` command automatically times the execution of the supplied command
- Push is unconditional to ensure measurements are always saved

### Verification

After pushing your workflow, verify it runs successfully:

```bash
# Push the workflow file
git add .github/workflows/performance-tracking.yml
git commit -m "ci: add performance tracking workflow"
git push

# Check the workflow status
gh run list --workflow=performance-tracking.yml --limit 5

# View the most recent run
gh run view --log

# Verify measurements were pushed
git perf pull
git perf report
```

**Expected Result**: The workflow should complete successfully and push measurements to git-notes. The `git perf pull` command retrieves the measurements, and `git perf report` displays them.

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

    # Concurrency control prevents race conditions when multiple workflows
    # try to update the gh-pages branch simultaneously. Without this, you may
    # encounter "failed to push" errors or lost reports when multiple PRs or
    # commits trigger the workflow at the same time.
    concurrency:
      group: gh-pages-${{ github.ref }}      # One deployment per branch at a time
      cancel-in-progress: false              # Queue jobs instead of canceling

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install git-perf
        uses: kaihowl/git-perf/.github/actions/install@master
        with:
          release: latest

      - name: Configure git identity
        run: |
          git config --global user.email "actions@github.com"
          git config --global user.name "GitHub Actions"

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

**Important**: The first workflow run will intentionally fail with clear setup instructions. This is expected behavior.

**First Run (Expected to Fail):**

When you push the workflow for the first time:
1. ✅ The workflow will successfully generate your performance report
2. ✅ The workflow will create and push the `gh-pages` branch
3. ❌ The workflow will fail at "Get Pages URL" with detailed instructions

**After First Run, Configure GitHub Pages:**

1. Go to **Settings → Pages** in your repository
   - Direct link: `https://github.com/<owner>/<repo>/settings/pages`

2. Under **"Build and deployment"**:
   - **Source**: Select "Deploy from a branch"
   - **Branch**: Select `gh-pages` and `/ (root)`
   - Click **"Save"**

3. Wait a few minutes for GitHub Pages to initialize

4. **Re-run the workflow**:
   - Go to the Actions tab
   - Find the failed workflow run
   - Click "Re-run all jobs"

5. ✅ The workflow will now succeed and your report will be available at:
   - `https://<username>.github.io/<repository>/`

**Note**: The `gh-pages` branch is automatically created by the report action on the first run. If you don't see it in the dropdown immediately, refresh the Settings page.

### Verification

Verify GitHub Pages is working correctly:

```bash
# Check that gh-pages branch exists
git ls-remote origin gh-pages

# View the GitHub Pages deployment status
gh api repos/$(gh repo view --json nameWithOwner -q .nameWithOwner)/pages

# Visit your report URL (replace with your details)
# https://<username>.github.io/<repository>/<branch-name>.html
```

**Expected Result**: GitHub Pages should show as "built and deployed" and your report should be accessible via the URL.

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

### Issue: Git Identity Not Configured

**Symptom**: Workflow fails with "Author identity unknown" or "empty ident name not allowed"

**Error Message**:
```
Error: Permanent failure while adding note line to head

Caused by:
    Git failed to execute.

    stderr:
    Author identity unknown

    *** Please tell me who you are.
```

**Solution**:
Add git identity configuration before any git-perf measurement commands:
```yaml
- name: Configure git identity
  run: |
    git config --global user.email "actions@github.com"
    git config --global user.name "GitHub Actions"
```

**Why**: Git-perf stores measurements as git-notes, which require git commits. All commits need a configured user identity.

### Issue: Push Fails in GitHub Actions

**Symptom**: `git perf push` fails with authentication errors

**Solutions**:
1. Ensure `contents: write` permission is set in the workflow
2. Use `fetch-depth: 0` when checking out the repository
3. Verify the branch is not protected (or add exception for Actions)

### Issue: Report Action Fails - GitHub Pages Not Configured

**Symptom**: Workflow fails at "Get Pages URL" step with "Not Found (HTTP 404)"

**Error Message**:
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📄 GitHub Pages Setup Required
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

**This is Expected on First Run!**

The workflow intentionally fails with detailed instructions on how to configure GitHub Pages.

**Solution**:
1. The `gh-pages` branch has already been created by the workflow
2. Go to Settings → Pages in your repository
3. Select `gh-pages` branch and `/ (root)` folder
4. Click "Save"
5. Re-run the workflow

See the [Enable GitHub Pages](#enable-github-pages) section for complete instructions.

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

### Issue: Measurements Not Appearing After Push

**Symptom**: Workflow completes but measurements don't show up in reports

**Debug Steps**:
1. Check if notes were actually pushed:
   ```bash
   git ls-remote origin refs/notes/perf-v3
   ```

2. Fetch notes manually:
   ```bash
   git fetch origin refs/notes/perf-v3:refs/notes/perf-v3
   ```

3. Verify measurements exist locally:
   ```bash
   git notes --ref=refs/notes/perf-v3 list
   ```

4. Check workflow logs for push errors:
   ```bash
   gh run view --log | grep -A 10 "Push measurements"
   ```

**Common Causes**:
- Git identity not configured (measurements can't be committed)
- Insufficient permissions (`contents: write` missing)
- Protected branch rules blocking git-notes push

### Issue: Workflow Fails with Permission Errors

**Symptom**: "Resource not accessible by integration" or similar permission errors

**Error Message**:
```
Error: Resource not accessible by integration
```

**Solutions**:
1. Verify workflow permissions in YAML:
   ```yaml
   permissions:
     contents: write
     pages: write
     pull-requests: write
   ```

2. Check repository Settings → Actions → General → Workflow permissions:
   - Ensure "Read and write permissions" is enabled
   - Or grant specific permissions in the workflow file

3. For organization repositories, check organization-level permissions

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

### 5. Testing Your Integration

**Before deploying to your main branch**, test the integration on a feature branch:

1. **Create a test branch**:
   ```bash
   git checkout -b test-git-perf-integration
   ```

2. **Add workflow files and configuration**:
   ```bash
   git add .github/workflows/ .gitperfconfig
   git commit -m "test: add git-perf integration"
   git push -u origin test-git-perf-integration
   ```

3. **Verify the workflow runs successfully**:
   ```bash
   # Watch the workflow run
   gh run watch

   # Check for errors
   gh run list --branch test-git-perf-integration --limit 5
   ```

4. **Test with manual workflow dispatch first** before enabling automatic triggers:
   ```yaml
   on:
     workflow_dispatch:  # Only manual triggering initially
     # push:              # Enable after testing
     #   branches: [main]
   ```

5. **Make small, incremental changes**:
   - Start with just measurement collection (Step 3)
   - Then add reporting (Step 4)
   - Then add cleanup (Step 5)
   - Finally add audit (Step 6)

6. **Once verified, merge to main**:
   ```bash
   gh pr create --title "feat: add performance tracking with git-perf"
   # After review and approval
   gh pr merge --squash
   ```

**Benefits of Testing First**:
- Catch configuration errors before they affect main branch
- Experiment with settings without polluting production data
- Understand the full workflow before team-wide rollout
- Avoid breaking CI/CD for the entire team

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

## What Success Looks Like: End-to-End Flow

Here's what a complete, successful git-perf integration looks like in action:

### Step-by-Step Walkthrough

1. **You make a code change** that affects performance (e.g., optimize a function)

2. **Create a pull request**:
   ```bash
   git checkout -b optimize-parser
   git commit -am "perf: optimize JSON parser"
   git push -u origin optimize-parser
   gh pr create --title "perf: optimize JSON parser"
   ```

3. **GitHub Actions automatically runs**:
   - ✅ Checks out code with full history (`fetch-depth: 0`)
   - ✅ Installs git-perf
   - ✅ Configures git identity
   - ✅ Builds project and measures `build_time`
   - ✅ Runs tests and measures `test_duration`
   - ✅ Measures `binary_size`
   - ✅ Pushes measurements to `refs/notes/perf-v3`
   - ✅ Generates interactive HTML report
   - ✅ Publishes report to GitHub Pages
   - ✅ Runs audit to check for regressions
   - ✅ Comments on your PR with results

4. **You receive a PR comment** with performance analysis:

   ```markdown
   ## Performance Report

   ⏱  [Performance Results](https://username.github.io/repo/abc123def.html)

   ## Audit Results

   ```
   ✅ 'build_time'
   z-score (stddev): ↓ 2.62
   Head: μ: 38.2s σ: 0ns MAD: 0ns n: 1
   Tail: μ: 42.1s σ: 1.8s MAD: 1.2s n: 15
    [-9.26% – +0.50%] ▃▅▄▆▅▄▅▃▅▂▁
   ✅ 'test_duration'
   z-score (stddev): ↓ 1.62
   Head: μ: 1.2s σ: 0ns MAD: 0ns n: 1
   Tail: μ: 1.5s σ: 0.1s MAD: 0.08s n: 15
    [-20.00% – +5.00%] ▃▅▄▆▅▄▅▃▅▄▁
   ❌ 'binary_size'
   z-score (stddev): ↑ 5.23
   Head: μ: 4.8MB σ: 0B MAD: 0B n: 1
   Tail: μ: 4.5MB σ: 8.2kB MAD: 4.1kB n: 15
    [+6.91% – +6.91%] ▃▅▄▆▅▄▅▃▅
   ```

   _Created by [git-perf](https://github.com/kaihowl/git-perf/)_
   ```

5. **You analyze the results**:
   - ✅ Build time improved by 9.3% - Great!
   - ✅ Test duration unchanged - Expected
   - ❌ Binary size increased by 6.9% - Needs investigation

6. **You click the report link** to see the interactive dashboard:
   - View historical trends with Plotly charts
   - Filter by branch, measurement, or time range
   - See sparklines showing performance over time
   - Export data as CSV for further analysis

7. **You investigate the binary size regression**:
   ```bash
   # Check what changed
   git diff main...optimize-parser -- Cargo.lock

   # Turns out the optimization added a new dependency
   # If the performance gain is worth the size increase, accept the regression
   # by bumping the epoch for this measurement:
   git perf add -m binary_size <new_value> --bump-epoch

   # This resets the baseline, and future measurements will be compared
   # against this new baseline instead
   ```

8. **Team reviews and approves** the PR, understanding the performance trade-offs

9. **PR is merged to main**:
   - Measurements become part of the main branch history
   - Future PRs will be compared against this new baseline
   - Performance dashboard updates with main branch data

### What You Get Over Time

After using git-perf for a while, you'll have:

- **Historical Performance Data**: Months of measurements showing trends
- **Automated Regression Detection**: Catch performance regressions in code review
- **Performance Dashboard**: Share `https://yourname.github.io/repo/` with your team
- **Data-Driven Decisions**: "Should we take this dependency? Let's check the performance impact"
- **Performance Culture**: Team awareness of performance implications in every PR

### Visual Example of Report Output

When you open the HTML report, you see:

- **Interactive Charts**: Plotly graphs showing performance over time
  - Line charts for trending metrics (build time, test duration)
  - Bar charts comparing commits
  - Hover for detailed measurement values

- **Filtering Options**:
  - By measurement name
  - By branch
  - By date range
  - By commit

- **Statistical Summaries**:
  - Mean, median, standard deviation
  - Min/max values
  - Outlier detection
  - Change point detection (when performance characteristics shifted)

- **Export Options**:
  - Download as CSV
  - Aggregate by mean, median, min, or max
  - Include all measurements or filter by criteria

This complete workflow ensures you **never accidentally ship performance regressions** and can **confidently make performance improvements** backed by data.

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
