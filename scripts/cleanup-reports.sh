#!/bin/bash
# Removes HTML reports for commits without performance measurements
# Usage: cleanup-reports.sh [--dry-run]

set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
fi

# Check if git-perf is available
if ! command -v git-perf &> /dev/null; then
    echo "Error: git-perf command not found"
    exit 1
fi

# Fetch the notes ref from remote
echo "Fetching performance measurements from remote..."
git perf pull || echo "Warning: git perf pull failed, using local measurements"

# Get list of commits that have measurements using git-perf
echo "Fetching commits with measurements..."
git perf list-commits | sort > /tmp/commits_with_measurements.txt

MEASUREMENT_COUNT=$(wc -l < /tmp/commits_with_measurements.txt)
echo "Found $MEASUREMENT_COUNT commits with measurements"

# Get list of commit-based reports on gh-pages
echo "Fetching reports from gh-pages branch..."
git ls-tree --name-only gh-pages | \
    grep -E '^[0-9a-f]{40}\.html$' | \
    sed 's/\.html$//' | \
    sort > /tmp/commits_with_reports.txt

REPORT_COUNT=$(wc -l < /tmp/commits_with_reports.txt)
echo "Found $REPORT_COUNT commit-based reports"

# Find reports without measurements (orphaned reports)
ORPHANED_REPORTS=$(comm -13 /tmp/commits_with_measurements.txt /tmp/commits_with_reports.txt)
ORPHAN_COUNT=$(echo "$ORPHANED_REPORTS" | grep -c . || echo 0)

if [ "$ORPHAN_COUNT" -eq 0 ]; then
    echo "✓ No orphaned reports found. All reports have corresponding measurements."
    exit 0
fi

echo "⚠ Found $ORPHAN_COUNT orphaned reports (reports without measurements)"

if [ "$DRY_RUN" = true ]; then
    echo ""
    echo "DRY RUN: Would delete the following reports:"
    echo "$ORPHANED_REPORTS" | head -20
    if [ "$ORPHAN_COUNT" -gt 20 ]; then
        echo "... and $((ORPHAN_COUNT - 20)) more"
    fi
    echo ""
    echo "Run without --dry-run to actually delete these reports"
    exit 0
fi

# Checkout gh-pages and delete orphaned reports
echo "Checking out gh-pages branch..."
CURRENT_BRANCH=$(git branch --show-current)
git checkout gh-pages

echo "Deleting orphaned reports..."
DELETED_COUNT=0
for commit in $ORPHANED_REPORTS; do
    if git rm "${commit}.html" 2>/dev/null; then
        ((DELETED_COUNT++))
    else
        echo "Warning: Could not remove ${commit}.html"
    fi
done

if [ -n "$(git status --porcelain)" ]; then
    git commit -m "chore: remove $DELETED_COUNT reports without measurements

Removed reports for commits that no longer have performance
measurements as reported by 'git perf list-commits'."

    echo "✓ Deleted $DELETED_COUNT orphaned reports"
    echo "✓ Changes committed to gh-pages"
    echo ""
    echo "To push: git push origin gh-pages"
else
    echo "No changes to commit"
fi

# Return to original branch
if [ -n "$CURRENT_BRANCH" ]; then
    git checkout "$CURRENT_BRANCH"
fi
