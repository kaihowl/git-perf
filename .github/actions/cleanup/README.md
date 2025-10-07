# git-perf Cleanup Action

A GitHub Action to automatically remove old measurements and orphaned reports from git-perf tracking.

## Overview

This action helps maintain your git-perf measurement history by:
- Removing measurements older than a specified retention period
- Creating backups before deletion (optional)
- Cleaning up orphaned reports on gh-pages (optional)
- Pushing updated notes and reports back to the repository

## Usage

### Basic Example

```yaml
name: Cleanup Old Measurements

on:
  schedule:
    # Run every Sunday at 02:00 UTC
    - cron: '0 2 * * 0'
  workflow_dispatch: # Allow manual triggering

permissions:
  contents: write
  pages: write

jobs:
  cleanup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0  # Full checkout required for git perf operations

      - name: Cleanup measurements
        uses: kaihowl/git-perf/.github/actions/cleanup@master
```

### Custom Retention Period

```yaml
- name: Cleanup measurements (retain 30 days)
  uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    retention-days: 30
```

### Disable Backup

```yaml
- name: Cleanup measurements (no backup)
  uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    backup: false
```

### Cleanup Measurements Only (Skip Reports)

```yaml
- name: Cleanup measurements only
  uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    cleanup-reports: false
```

### Use Specific git-perf Version

```yaml
- name: Cleanup measurements
  uses: kaihowl/git-perf/.github/actions/cleanup@master
  with:
    git-perf-version: '0.17.2'
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|---------|
| `retention-days` | Number of days to retain measurements | No | `90` |
| `backup` | Create backup of measurements before removal | No | `true` |
| `cleanup-reports` | Also cleanup orphaned reports on gh-pages branch | No | `true` |
| `git-perf-version` | Version of git-perf to use (latest or specific version) | No | `latest` |

## Permissions Required

This action requires the following permissions:

```yaml
permissions:
  contents: write  # Required to push updated notes and reports
  pages: write     # Required if cleanup-reports is enabled
```

## How It Works

1. **Fetch Notes**: Downloads the git-perf notes ref (`refs/notes/perf-v3`)
2. **Backup** (optional): Creates a backup at `refs/notes/perf-v3-backup`
3. **Remove Old Measurements**: Runs `git perf remove --older-than <days>d`
4. **Push Notes**: Pushes updated notes back to origin
5. **Cleanup Reports** (optional): Removes orphaned reports from gh-pages
6. **Push Reports** (optional): Pushes cleaned gh-pages branch

## Recovery from Backup

If you need to recover measurements from a backup:

```bash
# Fetch the backup
git fetch origin refs/notes/perf-v3-backup:refs/notes/perf-v3-backup

# Restore from backup
git push origin refs/notes/perf-v3-backup:refs/notes/perf-v3 --force
```

## Complete Workflow Example

```yaml
name: 'Cleanup Old Measurements and Reports'

on:
  schedule:
    # Run every Sunday at 02:00 UTC
    - cron: '0 2 * * 0'
  workflow_dispatch: # Allow manual triggering

permissions:
  contents: write
  pages: write

jobs:
  cleanup:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v5
        with:
          fetch-depth: 0  # Full checkout required for git perf operations

      - name: Cleanup old measurements and reports
        uses: kaihowl/git-perf/.github/actions/cleanup@master
        with:
          retention-days: 90
          backup: true
          cleanup-reports: true
          git-perf-version: latest
```

## Scheduling Recommendations

Consider running cleanup on a schedule that balances storage concerns with historical data needs:

- **Weekly**: Good for projects with frequent measurements
  ```yaml
  - cron: '0 2 * * 0'  # Sundays at 02:00 UTC
  ```

- **Monthly**: For projects with moderate measurement frequency
  ```yaml
  - cron: '0 2 1 * *'  # First day of month at 02:00 UTC
  ```

- **Manual Only**: For projects where you want full control
  ```yaml
  on:
    workflow_dispatch:  # Manual triggering only
  ```

## Troubleshooting

### No measurements removed

If the action runs but doesn't remove measurements:
- Verify measurements exist with `git perf list`
- Check that measurements are actually older than the retention period
- Ensure the notes ref is properly fetched

### Backup creation fails

If backup creation fails:
- Ensure the workflow has `contents: write` permission
- Check that the notes ref exists and is accessible

### Report cleanup fails

If report cleanup fails:
- Verify the `scripts/cleanup-reports.sh` script exists in your repository
- Ensure the workflow has `pages: write` permission
- Check that the gh-pages branch exists

## See Also

- [git-perf Install Action](../install/README.md)
- [git-perf Documentation](https://github.com/kaihowl/git-perf)
- [Integration Tutorial](../../../docs/INTEGRATION_TUTORIAL.md) *(coming soon)*
