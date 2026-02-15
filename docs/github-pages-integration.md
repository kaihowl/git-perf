# GitHub Pages Integration Guide

This guide shows how to deploy git-perf reports to GitHub Pages alongside existing documentation or other static content.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Subdirectory Deployment](#subdirectory-deployment)
- [Integration Examples](#integration-examples)
- [Workflow Coordination](#workflow-coordination)
- [Index Generation](#index-generation)
- [Troubleshooting](#troubleshooting)

## Overview

Git-perf's GitHub Actions support flexible GitHub Pages deployment:

- **Subdirectory deployment**: Place reports in `/perf/` or `/reports/` to coexist with documentation
- **Preserve existing content**: Keep your docs, blogs, or other Pages content intact
- **Automatic index generation**: Create an index page listing all available reports
- **Safe cleanup**: Remove orphaned reports without touching unrelated files

## Quick Start

### Single-Purpose Deployment (Reports Only)

If you want GitHub Pages to show only git-perf reports:

```yaml
name: Performance Reports
on:
  push:
    branches: [main]
  pull_request:

jobs:
  report:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pages: write
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100

      - uses: kaihowl/git-perf/.github/actions/report@master
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

This deploys reports to the root of your GitHub Pages site.

### Multi-Purpose Deployment (Reports + Docs)

If you have existing documentation on GitHub Pages:

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'
    preserve-existing: 'true'
    generate-index: 'true'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

This deploys reports to `/perf/` and generates an index at `/perf/index.html`.

## Subdirectory Deployment

### Why Use Subdirectories?

Subdirectories allow git-perf reports to coexist with other content on GitHub Pages:

- **Documentation sites** (MkDocs, Sphinx, Jekyll) in the root
- **Performance reports** in `/perf/`
- **API docs** in `/api/`
- **Custom content** in other directories

### Configuration

Use the `reports-subdirectory` input to specify where reports should live:

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'    # Reports go to /perf/
    preserve-existing: 'true'        # Keep existing content
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Important**: The cleanup action must use the **same subdirectory**:

```yaml
- uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    reports-subdirectory: 'perf'    # Must match report action
    cleanup-reports: 'true'
```

### Security Validation

For security, subdirectory paths are validated:

- ✅ Allowed: `perf`, `reports`, `performance`, `perf/benchmarks`
- ❌ Rejected: `../secrets`, `/etc/passwd`, `perf/../root`

Only alphanumeric characters, underscores, hyphens, and forward slashes are permitted.

## Integration Examples

### Example 1: MkDocs Documentation + Performance Reports

**Repository Structure:**
```
repo/
├── docs/               # MkDocs source
│   ├── index.md
│   └── mkdocs.yml
├── .git-perf/
│   └── templates/
│       └── performance-overview.html
└── .github/workflows/
    ├── docs.yml        # Build MkDocs → gh-pages/
    └── perf.yml        # Build reports → gh-pages/perf/
```

**Documentation Workflow** (`.github/workflows/docs.yml`):

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
  cancel-in-progress: false

jobs:
  deploy-docs:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-python@v5
        with:
          python-version: 3.x
      - run: pip install mkdocs-material
      - run: mkdocs build

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./site
          keep_files: true  # Preserve /perf/ reports
```

**Performance Workflow** (`.github/workflows/perf.yml`):

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
  cancel-in-progress: false

jobs:
  report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100

      - uses: kaihowl/git-perf/.github/actions/report@master
        with:
          reports-subdirectory: 'perf'
          preserve-existing: 'true'
          generate-index: 'true'
          index-title: 'Performance Reports'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Linking from MkDocs** (`docs/index.md`):

```markdown
## Quick Links

- [Performance Reports](../perf/)
- [Latest Main Branch Report](../perf/main.html)
```

**Result:**
- Docs: `https://user.github.io/repo/`
- Reports: `https://user.github.io/repo/perf/`
- Index: `https://user.github.io/repo/perf/index.html`

### Example 2: Jekyll Site with Performance Dashboard

**Root Navigation** (`_layouts/default.html`):

```html
<nav>
  <a href="{{ site.baseurl }}/">Home</a>
  <a href="{{ site.baseurl }}/docs/">Docs</a>
  <a href="{{ site.baseurl }}/perf/">Performance</a>
</nav>
```

**Jekyll Workflow:**

```yaml
- name: Build Jekyll
  run: bundle exec jekyll build

- name: Deploy Jekyll + Reports
  uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./_site
    keep_files: true  # Reports added separately by perf workflow
```

### Example 3: Simple Static Site

Create a minimal root `index.html` (committed to `gh-pages`):

```html
<!DOCTYPE html>
<html>
<head>
    <title>Project Name</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body { font-family: system-ui; max-width: 800px; margin: 50px auto; }
        nav { display: flex; gap: 20px; margin-bottom: 30px; }
        nav a { text-decoration: none; color: #0366d6; }
    </style>
</head>
<body>
    <nav>
        <a href="/">Home</a>
        <a href="/perf/">Performance Reports</a>
        <a href="https://github.com/user/repo">GitHub</a>
    </nav>

    <h1>Project Name</h1>

    <h2>Quick Links</h2>
    <ul>
        <li><a href="/perf/main.html">Latest Performance (main branch)</a></li>
        <li><a href="/perf/index.html">All Performance Reports</a></li>
    </ul>
</body>
</html>
```

This index is preserved by `keep_files: true` when reports are deployed to `/perf/`.

## Workflow Coordination

### Preventing Race Conditions

When multiple workflows deploy to `gh-pages`, use `concurrency` to queue deployments:

```yaml
concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false  # Queue, don't cancel
```

This ensures:
- Deployments happen sequentially (no conflicts)
- All deployments complete (no data loss)

### Per-Workflow Concurrency

For more granular control, use separate concurrency groups:

```yaml
# Documentation workflow
concurrency:
  group: gh-pages-docs-${{ github.ref }}
  cancel-in-progress: false

# Performance workflow
concurrency:
  group: gh-pages-perf-${{ github.ref }}
  cancel-in-progress: false
```

## Index Generation

The index page lists all available performance reports with categorization:

- **Branch Reports**: `main.html`, `develop.html`
- **Commit Reports**: `a1b2c3d4...html` (40-character SHA)
- **Other Reports**: Custom-named reports

### Enabling Index Generation

Add the `generate-index` input:

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    generate-index: 'true'
    index-title: 'Performance Reports'
    reports-subdirectory: 'perf'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Manual Index Generation

Generate an index locally:

```bash
# Basic usage
git perf generate-index -o perf/index.html

# With subdirectory (reads from gh-pages branch)
git perf generate-index -o perf/index.html --subdirectory perf

# Custom title
git perf generate-index -o index.html --title "My Performance Reports"

# Custom template
git perf generate-index -o index.html --template .git-perf/index-template.html
```

### Index Structure

The generated index includes:

- **Metadata**: Commit date, author for each commit report
- **Categorization**: Separate sections for branch, commit, and custom reports
- **Sorting**: Commit reports sorted by date (newest first)
- **Direct Links**: Click to view each report

## Troubleshooting

### Reports Not Showing Up

**Symptom**: Workflow succeeds but reports aren't visible.

**Solution**: Ensure GitHub Pages is configured:
1. Go to Settings → Pages
2. Source: "Deploy from a branch"
3. Branch: `gh-pages` and `/ (root)`
4. Click "Save"

### Subdirectory Path Error

**Symptom**: `Error: Invalid subdirectory path`

**Cause**: Subdirectory contains invalid characters or path traversal attempts.

**Solution**: Use only alphanumeric characters, underscores, hyphens:
- ✅ `perf`, `reports`, `performance/benchmarks`
- ❌ `../secret`, `/root`, `perf..`

### Documentation Overwritten by Reports

**Symptom**: Docs disappear after running performance workflow.

**Cause**: Missing `keep_files: true` or incorrect subdirectory.

**Solution**:
```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    keep_files: true  # Add this
    destination_dir: perf  # Use subdirectory
```

### Cleanup Removes Wrong Files

**Symptom**: Documentation or other files deleted by cleanup action.

**Cause**: Cleanup action's subdirectory doesn't match report action.

**Solution**: Verify both actions use the same subdirectory:
```yaml
# Report action
reports-subdirectory: 'perf'

# Cleanup action (must match)
reports-subdirectory: 'perf'
```

### Index Shows No Reports

**Symptom**: Index page generated but shows "No reports found."

**Cause**: Timing issue - index generated before first report is published.

**Solution**: The index generation fetches the `gh-pages` branch *before* the current report is published. On subsequent runs, previous reports will appear. This is expected for the first workflow run.

### Race Condition Between Workflows

**Symptom**: Occasional "non-fast-forward" errors or missing content.

**Cause**: Multiple workflows pushing to `gh-pages` simultaneously.

**Solution**: Add concurrency control to **both** workflows:
```yaml
concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false
```

## Migration from Root-Level Reports

If you currently deploy reports to the root of GitHub Pages and want to move them to a subdirectory:

**Step 1**: Update your performance workflow:
```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'  # New
    preserve-existing: 'true'      # New
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Step 2**: Move existing reports manually:
```bash
git checkout gh-pages
mkdir -p perf
git mv *.html perf/ 2>/dev/null || true  # Move SHA-named reports
git commit -m "refactor: move reports to /perf subdirectory"
git push origin gh-pages
```

**Step 3**: Update cleanup action:
```yaml
- uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    reports-subdirectory: 'perf'  # Must match
    cleanup-reports: 'true'
```

## Best Practices

1. **Use subdirectories** for multi-purpose sites
2. **Enable `preserve-existing`** when deploying to subdirectories
3. **Add concurrency controls** to prevent race conditions
4. **Generate index pages** to make reports discoverable
5. **Link from your docs** to help users find performance reports
6. **Use consistent subdirectory names** across report and cleanup actions

## See Also

- [Report Templating Guide](./report-templating.md) - Customize report appearance
- [Integration Tutorial](./INTEGRATION_TUTORIAL.md) - GitHub Actions setup
- [Dashboard Templates](./dashboard-templates.md) - Multi-section reports
