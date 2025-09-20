#!/bin/bash

# Script to generate manpages and markdown documentation
# This script temporarily unsets the version to avoid version-based diffs in generated docs

set -e

# Check that the generated markdown file exists
if [[ ! -f "docs/manpage.md" ]]; then
  echo "Error: Generated markdown documentation not found at docs/manpage.md"
  exit 1
fi

# Create a backup of the original file for comparison
cp docs/manpage.md /tmp/original_markdown.md

# Temporarily set version to 0.0.0 to avoid version-based diffs
sed -i 's/^version = "[0-9]\+\.[0-9]\+\.[0-9]\+"/version = "0.0.0"/' git_perf/Cargo.toml

# Generate manpages and markdown documentation
cargo build

# Restore original version
git checkout -- git_perf/Cargo.toml

# Compare with the newly generated version
if ! diff -u /tmp/original_markdown.md docs/manpage.md > /tmp/markdown.diff; then
  echo "Error: Markdown documentation is out of date. A patch file has been created at /tmp/markdown.diff"
  echo ""
  echo "To fix this, run:"
  echo "   ./scripts/generate-manpages.sh"
  echo ""
  echo "The markdown documentation is automatically generated during the build process using clap_markdown."
  exit 1
fi

echo "Markdown documentation is up to date ✓"