#!/usr/bin/env bash
#
# Script to create GitHub issue for PR #589 optimization
#
# Prerequisites:
#   - gh CLI installed and authenticated
#   - Alternatively, GITHUB_TOKEN with issues:write scope
#
# Usage:
#   ./create-issue-pr589.sh

set -euo pipefail

REPO="kaihowl/git-perf"
TITLE="perf(status): reuse git log command instead of creating process per commit"
LABELS="enhancement,performance"

BODY=$(cat <<'EOF'
## Context

In PR #589, there's a noted inefficiency in `status.rs` regarding individual commit processing.

## Current Implementation

The current implementation creates a new process for each commit when processing status information.

## Proposed Improvement

Instead of creating a new process for each commit, we should reuse the git log command to improve performance and reduce overhead.

## Benefits

- Reduced process creation overhead
- Better performance when dealing with many commits
- More efficient resource utilization

## Reference

- Original comment from PR #589 (Jan 3, 2026)
- Related file: `git_perf/src/status.rs`

---
*Issue tracked in: docs/issues/pr-589-status-optimization.md*
EOF
)

echo "Creating GitHub issue..."
echo "Repository: $REPO"
echo "Title: $TITLE"
echo ""

if command -v gh &> /dev/null; then
    # Use gh CLI
    gh issue create \
        --repo "$REPO" \
        --title "$TITLE" \
        --label "$LABELS" \
        --body "$BODY"

    echo "✅ Issue created successfully!"
else
    echo "❌ Error: gh CLI not found"
    echo ""
    echo "Please install gh CLI or create the issue manually:"
    echo "https://github.com/$REPO/issues/new"
    echo ""
    echo "Title:"
    echo "$TITLE"
    echo ""
    echo "Labels: $LABELS"
    echo ""
    echo "Body:"
    echo "$BODY"
    exit 1
fi
