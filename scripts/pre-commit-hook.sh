#!/bin/bash
set -euo pipefail

# Pre-commit hook to validate manpage generation
# This can be installed as a git pre-commit hook to catch manpage issues early

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🔍 Pre-commit: Validating manpage generation..."

# Change to workspace root
cd "$WORKSPACE_ROOT"

# Check if docs/manpage.md has been modified
if git diff --cached --name-only | grep -q "docs/manpage.md"; then
    echo "📝 docs/manpage.md is staged for commit"
    
    # Validate the manpage
    if ./scripts/validate-manpage.sh; then
        echo "✅ Manpage validation passed"
    else
        echo "❌ Manpage validation failed!"
        echo ""
        echo "The staged docs/manpage.md doesn't match CI expectations."
        echo "To fix this, run:"
        echo "  ./scripts/generate-manpage.sh"
        echo "  git add docs/manpage.md"
        echo ""
        echo "Or to skip this check, commit with:"
        echo "  git commit --no-verify"
        exit 1
    fi
else
    echo "ℹ️  docs/manpage.md not modified, skipping validation"
fi

echo "✅ Pre-commit checks passed"