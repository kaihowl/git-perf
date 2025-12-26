# git-perf Report Action

A GitHub Action to generate and publish git-perf performance reports to GitHub Pages.

> **⚠️ First-Time Setup Required**: On your first workflow run, the action will fail with clear instructions to configure GitHub Pages. After enabling Pages in your repository settings, simply re-run the workflow. See [Prerequisites](#prerequisites) for details.

## Features

- Generates HTML performance reports using `git-perf report`
- Optionally runs `git-perf audit` for performance analysis
- Publishes reports to GitHub Pages
- **Supports subdirectory organization** for coexistence with existing documentation
- Automatically comments on pull requests with report URL and audit results
- Supports custom report naming and depth configuration
- Returns report URL and audit output for use in subsequent steps
- **Provides clear setup instructions** when GitHub Pages is not configured

## Usage

### Basic Usage

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Advanced Usage

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    depth: 100
    report-name: 'my-custom-report'
    additional-args: '--csv-aggregate mean'
    audit-args: '-m build_time -m memory_usage -d 3.0 --min-measurements 10'
    git-perf-version: 'latest'
    comment-on-pr: 'true'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Using Outputs

The action automatically comments on PRs by default. To disable automatic commenting and use custom comments:

```yaml
- id: perf-report
  uses: kaihowl/git-perf/.github/actions/report@master
  with:
    audit-args: '-m build_time -d 2.0 --min-measurements 5'
    comment-on-pr: 'false'  # Disable automatic PR commenting
    github-token: ${{ secrets.GITHUB_TOKEN }}

- name: Custom PR comment with additional info
  if: github.event_name == 'pull_request'
  uses: actions/github-script@v8
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    script: |
      const reportUrl = '${{ steps.perf-report.outputs.report-url }}'
      const auditOutput = `${{ steps.perf-report.outputs.audit-output }}`

      const auditSection = auditOutput ? `\n\n## Audit Results\n\n\`\`\`\n${auditOutput}\n\`\`\`` : ''
      const body = `⏱  [Performance Results](${reportUrl})${auditSection}\n\n_Custom additional information here_`

      github.rest.issues.createComment({
        issue_number: context.issue.number,
        owner: context.repo.owner,
        repo: context.repo.repo,
        body: body
      })
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `depth` | Depth of the report in number of commits | No | `40` |
| `report-name` | Name of the report file (without .html). If empty, uses branch name or commit SHA | No | `` |
| `additional-args` | Additional arguments to git-perf report invocation | No | `` |
| `audit-args` | Additional arguments to git-perf audit (e.g., `-m <measurement> -d <threshold>`) | No | `` |
| `git-perf-version` | Version of git-perf to use (latest, or specific version) | No | `latest` |
| `comment-on-pr` | Whether to comment on the PR with the report URL (only for pull_request events) | No | `true` |
| `show-size` | Whether to show measurement storage size in output | No | `false` |
| `size-use-disk-size` | Whether to use disk-size (compressed) instead of logical size | No | `true` |
| `reports-subdirectory` | Subdirectory within gh-pages for reports (e.g., "perf", "reports"). Empty for root. | No | `` |
| `preserve-existing` | Preserve existing gh-pages content outside reports subdirectory | No | `true` |
| `show-epochs` | Whether to show epoch boundaries in the report | No | `false` |
| `show-changes` | Whether to detect and display change points in the report | No | `false` |
| `template` | Path to custom report template (relative to repo root). Empty for default single-plot report | No | `` |
| `github-token` | GitHub token for publishing to gh-pages and commenting on PRs | Yes | - |

### Common Audit Arguments

- `-m <measurement>`: Specify measurement(s) to audit (can be used multiple times)
- `-d <threshold>`: Deviation threshold (e.g., `2.0` for 2 standard deviations)
- `-s <selector>`: Filter by selector (e.g., `-s os=ubuntu`)
- `--min-measurements <n>`: Minimum number of measurements required
- `--dispersion-method <method>`: Use `stddev` or `mad` (Median Absolute Deviation)
- `-a <aggregation>`: Aggregation method (`min`, `max`, `mean`, `median`)

## Outputs

| Output | Description |
|--------|-------------|
| `report-url` | URL of the published report on GitHub Pages |
| `audit-output` | Output from git-perf audit command (if audit-args provided) |

## Prerequisites

### 1. GitHub Pages Configuration (REQUIRED)

**The action will fail with a clear error message if GitHub Pages is not configured.**

On first run, the workflow will:
1. ✅ Generate the performance report successfully
2. ✅ Push the `gh-pages` branch to your repository
3. ❌ Fail at the "Get Pages URL" step with instructions

**Setup Instructions:**

After the first workflow run creates the `gh-pages` branch, configure GitHub Pages:

1. Go to **Settings → Pages** in your repository
   - Direct link: `https://github.com/<owner>/<repo>/settings/pages`

2. Under **"Build and deployment"**:
   - **Source**: Select "Deploy from a branch"
   - **Branch**: Select `gh-pages` and `/ (root)`
   - Click **"Save"**

3. Wait a few minutes for GitHub Pages to initialize

4. **Re-run the workflow** and it will succeed

The action will provide detailed setup instructions if Pages is not configured.

### 2. Workflow Permissions

Workflow must have appropriate permissions:
```yaml
permissions:
  pages: write
  contents: write
  pull-requests: write  # If using comment-on-pr
```

### 3. Concurrency Control

**Important**: To prevent concurrent pushes to gh-pages branch, add concurrency control to your workflow:
```yaml
concurrency:
  group: gh-pages-${{ github.ref }}
  cancel-in-progress: false  # Don't cancel, let them queue
```

## Report Naming

The action automatically determines the report name based on context:

1. If `report-name` input is provided, uses that
2. For pull requests, uses the commit SHA
3. For other events, uses the branch name (via `GITHUB_REF_SLUG`)

## Examples

### Generate Report for Pull Request

```yaml
name: Performance Report
on: pull_request

permissions:
  pages: write
  contents: write
  pull-requests: write

jobs:
  report:
    runs-on: ubuntu-22.04
    concurrency:
      group: gh-pages-${{ github.ref }}
      cancel-in-progress: false
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 40

      - uses: kaihowl/git-perf/.github/actions/report@master
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Generate Report with Audit

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    depth: 100
    audit-args: '-m build_time -m test_duration -d 2.5 --min-measurements 10'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Using Specific git-perf Version

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    git-perf-version: 'v1.2.3'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Disable PR Comments

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    comment-on-pr: 'false'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

## Subdirectory Organization

The action supports deploying reports to a subdirectory within GitHub Pages, allowing coexistence with existing documentation sites.

### Deploy to Subdirectory

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'
    preserve-existing: 'true'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

This deploys reports to `https://user.github.io/repo/perf/` instead of the root.

### Enable Epoch and Change Point Detection

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    show-epochs: 'true'
    show-changes: 'true'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Using Dashboard Templates

The action supports multi-section dashboard templates for comprehensive performance reports. **Important**: Template files must be present in your repository, as the action executes `git perf report` in your repository's working directory.

**Setup**:

1. Copy template files from the [git-perf repository](https://github.com/kaihowl/git-perf/tree/master/.git-perf/templates) to your repository:
   ```bash
   # In your repository
   mkdir -p .git-perf/templates
   curl -o .git-perf/templates/performance-overview.html \
     https://raw.githubusercontent.com/kaihowl/git-perf/master/.git-perf/templates/performance-overview.html
   ```

2. Use the template in your workflow:
   ```yaml
   - uses: kaihowl/git-perf/.github/actions/report@master
     with:
       template: '.git-perf/templates/performance-overview.html'
       github-token: ${{ secrets.GITHUB_TOKEN }}
   ```

**Available Templates:**
- `performance-overview.html` - Professional multi-section dashboard (recommended)
- `simple-dashboard.html` - Basic multi-section template
- `dashboard-example.html` - Comprehensive example template

See [Dashboard Templates Guide](../../docs/dashboard-templates.md) for template syntax and customization.

### Multi-Workflow Coordination

When combining performance reports with existing documentation (MkDocs, Jekyll, etc.), use proper concurrency control:

```yaml
name: Performance Reports
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: write
  pages: write
  pull-requests: write

concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false  # Queue deployments, don't cancel

jobs:
  report:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100

      - uses: kaihowl/git-perf/.github/actions/report@master
        with:
          reports-subdirectory: 'perf'
          preserve-existing: 'true'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Documentation Workflow Example:**

```yaml
name: Deploy Documentation
on:
  push:
    branches: [main]
    paths: ['docs/**']

permissions:
  contents: write
  pages: write

concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false  # Same group as performance workflow

jobs:
  deploy-docs:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-python@v5
      - run: pip install mkdocs-material
      - run: mkdocs build

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./site
          keep_files: true  # Preserve perf/ reports
```

### Subdirectory Best Practices

1. **Use the same concurrency group** across all workflows deploying to gh-pages
2. **Set `cancel-in-progress: false`** to queue deployments instead of canceling
3. **Set `keep_files: true`** in all deployment actions to preserve existing content
4. **Use descriptive subdirectory names**: `perf`, `reports`, `benchmarks`, etc.
5. **Match subdirectory in cleanup action**: Ensure cleanup uses the same subdirectory

### Repository Structure Result

```
gh-pages branch:
├── index.html           # Documentation root (MkDocs/Jekyll)
├── docs/               # Documentation pages
│   └── ...
└── perf/               # Performance reports
    ├── main.html       # Branch reports
    ├── develop.html
    └── abc123....html  # Commit reports
```

## Notes

- **Concurrency Control**: The action does NOT enforce concurrency control internally. You MUST add concurrency control at the job/workflow level to prevent concurrent pushes to the gh-pages branch, which could cause conflicts.
- **Automatic PR Comments**: By default, the action automatically comments on pull requests with the report URL and audit results. Set `comment-on-pr: 'false'` to disable.
- **PR Comment Updates**: If a performance comment already exists, the action updates it instead of creating a new one.
- **GitHub Pages**: The action uses `peaceiris/actions-gh-pages@v4` to publish to GitHub Pages with `keep_files: true`, preserving previous reports.
- **Subdirectory Security**: The action validates subdirectory paths to prevent path traversal attacks (rejects `..`, absolute paths, and special characters).
- **Error Handling**: If `git perf pull` fails, the action continues with a warning (useful for missing git objects).
- **Audit Failures**: Audit results are captured even if the audit command fails, ensuring workflow continues.
