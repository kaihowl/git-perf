#!/bin/bash
# Removes HTML reports for commits without performance measurements
# Usage: cleanup-reports.sh [--dry-run] [-y|--yes]

set -euo pipefail

DRY_RUN=false
AUTO_YES=false

# Parse arguments
for arg in "$@"; do
    case $arg in
        --dry-run)
            DRY_RUN=true
            ;;
        -y|--yes)
            AUTO_YES=true
            ;;
        *)
            echo "Usage: $0 [--dry-run] [-y|--yes]"
            echo "  --dry-run  Show what would be deleted without making changes"
            echo "  -y, --yes  Skip confirmation prompts (required for non-interactive use)"
            exit 1
            ;;
    esac
done

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

# Show what will be deleted and ask for confirmation
echo ""
echo "The following reports will be deleted:"
echo "$ORPHANED_REPORTS" | head -20
if [ "$ORPHAN_COUNT" -gt 20 ]; then
    echo "... and $((ORPHAN_COUNT - 20)) more"
fi
echo ""

if [ "$AUTO_YES" = false ]; then
    read -p "⚠️  Delete $ORPHAN_COUNT orphaned reports? (yes/no): " -r
    echo
    if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        echo "Aborted. No changes made."
        exit 0
    fi
fi

# Checkout gh-pages and delete orphaned reports
echo "Checking out gh-pages branch..."
CURRENT_BRANCH=$(git branch --show-current)

if [ "$AUTO_YES" = false ]; then
    read -p "Continue with checkout to gh-pages? (yes/no): " -r
    echo
    if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        echo "Aborted. No changes made."
        exit 0
    fi
fi

git checkout gh-pages

# Set up trap to return to original branch on any exit
cleanup() {
    if [ -n "$CURRENT_BRANCH" ] && [ "$CURRENT_BRANCH" != "gh-pages" ]; then
        git checkout "$CURRENT_BRANCH" 2>/dev/null || true
    fi
}
trap cleanup EXIT

echo "Deleting orphaned reports..."
DELETED_COUNT=0
for commit in $ORPHANED_REPORTS; do
    if git rm "${commit}.html" 2>/dev/null; then
        ((DELETED_COUNT++)) || true  # Prevent errexit on arithmetic
    else
        echo "Warning: Could not remove ${commit}.html"
    fi
done

if [ -n "$(git status --porcelain)" ]; then
    echo "✓ Deleted $DELETED_COUNT orphaned reports"

    if [ "$AUTO_YES" = false ]; then
        read -p "Commit changes to gh-pages? (yes/no): " -r
        echo
        if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
            echo "Aborted. Changes not committed."
            echo "Run 'git checkout $CURRENT_BRANCH' to return to your original branch."
            exit 1
        fi
    fi

    git commit -m "chore: remove $DELETED_COUNT reports without measurements

Removed reports for commits that no longer have performance
measurements as reported by 'git perf list-commits'."

    echo "✓ Changes committed to gh-pages"
    echo ""
    echo "To push: git push origin gh-pages"
else
    echo "No changes to commit"
fi

# The trap cleanup will automatically return to the original branch on exit
