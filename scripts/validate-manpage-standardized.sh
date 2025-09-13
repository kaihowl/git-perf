#!/bin/bash
set -euo pipefail

# Standardized script to validate manpage.md using consistent pandoc configuration
# This ensures identical validation across CI and local environments

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🔍 Validating manpage.md with standardized pandoc configuration..."

# Change to workspace root
cd "$WORKSPACE_ROOT"

# Check if pandoc is available
if ! command -v pandoc &> /dev/null; then
    echo "❌ pandoc is not installed. Please install it:"
    echo "   Ubuntu/Debian: sudo apt-get install pandoc"
    echo "   macOS: brew install pandoc"
    exit 1
fi

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ cargo is not installed. Please install Rust:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Backup original Cargo.toml version
ORIGINAL_VERSION=$(grep '^version = ' git_perf/Cargo.toml | sed 's/version = "\(.*\)"/\1/')
echo "📝 Original version: $ORIGINAL_VERSION"

# Temporarily set version to 0.0.0 to match CI behavior
echo "🔄 Temporarily setting version to 0.0.0 for consistent validation..."
sed -i.bak 's/^version = "[0-9]\+\.[0-9]\+\.[0-9]\+"/version = "0.0.0"/' git_perf/Cargo.toml

# Build the project to generate manpages
echo "🔨 Building project to generate manpages..."
cargo build

# Restore original version
echo "🔄 Restoring original version..."
mv git_perf/Cargo.toml.bak git_perf/Cargo.toml

# Check if manpages were generated
MAN_DIR="target/man/man1"
if [[ ! -d "$MAN_DIR" ]]; then
    echo "❌ Manpage directory not found: $MAN_DIR"
    echo "   Make sure the build completed successfully"
    exit 1
fi

# Define expected manpage files (same as CI)
EXPECTED_FILES=(
    "target/man/man1/git-perf.1"
    "target/man/man1/git-perf-add.1"
    "target/man/man1/git-perf-audit.1"
    "target/man/man1/git-perf-bump-epoch.1"
    "target/man/man1/git-perf-measure.1"
    "target/man/man1/git-perf-prune.1"
    "target/man/man1/git-perf-pull.1"
    "target/man/man1/git-perf-push.1"
    "target/man/man1/git-perf-remove.1"
    "target/man/man1/git-perf-report.1"
)

# Check that all expected files exist
echo "🔍 Checking for all expected manpage files..."
MISSING_FILES=()
for file in "${EXPECTED_FILES[@]}"; do
    if [[ ! -f "$file" ]]; then
        MISSING_FILES+=("$file")
    fi
done

if [[ ${#MISSING_FILES[@]} -gt 0 ]]; then
    echo "❌ Missing manpage files:"
    printf '%s\n' "${MISSING_FILES[@]}"
    exit 1
fi

# Generate the expected manpage using standardized pandoc configuration
echo "📝 Generating expected manpage with standardized pandoc configuration..."
TEMP_MANPAGE="/tmp/expected_manpage.md"

# Use the same standardized pandoc options as the generation script
for file in "${EXPECTED_FILES[@]}"; do
    echo "$(basename "$file" .1)";
    echo "================";
    pandoc -f man -t gfm --wrap=none --columns=120 "$file" | sed 's/\\|/|/g';
    echo -e "\n\n";
done > "$TEMP_MANPAGE"

# Compare with existing manpage
echo "🔍 Comparing with existing docs/manpage.md..."
if diff -u docs/manpage.md "$TEMP_MANPAGE" > /tmp/manpage.diff; then
    echo "✅ Manpage is up to date and matches CI expectations!"
    rm -f "$TEMP_MANPAGE" /tmp/manpage.diff
    exit 0
else
    echo "❌ Manpage is out of date. Differences found:"
    echo ""
    cat /tmp/manpage.diff
    echo ""
    echo "🔧 To fix this, run:"
    echo "   ./scripts/generate-manpage-standardized.sh"
    echo ""
    echo "📁 A patch file has been saved to /tmp/manpage.diff"
    echo "   You can apply it with: patch docs/manpage.md < /tmp/manpage.diff"
    
    # Clean up temp file but keep diff for user
    rm -f "$TEMP_MANPAGE"
    exit 1
fi