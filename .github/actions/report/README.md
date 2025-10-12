# git-perf Report Action

A GitHub Action to generate and publish git-perf performance reports to GitHub Pages.

## Features

- Generates HTML performance reports using `git-perf report`
- Optionally runs `git-perf audit` for performance analysis
- Publishes reports to GitHub Pages
- Supports custom report naming and depth configuration
- Returns report URL and audit output for use in subsequent steps

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
    additional-args: '--format html'
    audit-args: '--threshold 10%'
    git-perf-version: 'latest'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Using Outputs

```yaml
- id: perf-report
  uses: kaihowl/git-perf/.github/actions/report@master
  with:
    audit-args: '--threshold 5%'
    github-token: ${{ secrets.GITHUB_TOKEN }}

- name: Comment on PR
  if: github.event_name == 'pull_request'
  uses: actions/github-script@v8
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
    script: |
      const reportUrl = '${{ steps.perf-report.outputs.report-url }}'
      const auditOutput = `${{ steps.perf-report.outputs.audit-output }}`

      const auditSection = auditOutput ? `\n\n## Audit Results\n\n\`\`\`\n${auditOutput}\n\`\`\`` : ''
      const body = `⏱  [Performance Results](${reportUrl})${auditSection}`

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
| `audit-args` | Additional arguments to git-perf audit invocation | No | `` |
| `git-perf-version` | Version of git-perf to use (latest, or specific version) | No | `latest` |
| `github-token` | GitHub token for publishing to gh-pages | Yes | - |

## Outputs

| Output | Description |
|--------|-------------|
| `report-url` | URL of the published report on GitHub Pages |
| `audit-output` | Output from git-perf audit command (if audit-args provided) |

## Prerequisites

- Repository must have GitHub Pages enabled
- Workflow must have appropriate permissions:
  ```yaml
  permissions:
    pages: write
    contents: write
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
    steps:
      - uses: actions/checkout@v5
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
    audit-args: '--threshold 5% --measurements build-time,test-duration'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Using Specific git-perf Version

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    git-perf-version: 'v1.2.3'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

## Notes

- The action uses `peaceiris/actions-gh-pages@v4` to publish to GitHub Pages with `keep_files: true`, preserving previous reports
- If `git perf pull` fails, the action continues with a warning (useful for missing git objects)
- Audit results are captured even if the audit command fails, ensuring workflow continues
