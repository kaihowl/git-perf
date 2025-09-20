#!/bin/bash

# Script to generate manpages and markdown documentation
# This script uses a normalized version number to avoid version-based diffs in generated docs

set -e

# Default version to use for documentation generation (can be overridden with GIT_PERF_VERSION env var)
NORMALIZED_VERSION="${GIT_PERF_VERSION:-0.0.0}"

echo "Generating manpages and markdown documentation with version: $NORMALIZED_VERSION"

# Check that the generated markdown file exists
if [[ ! -f "docs/manpage.md" ]]; then
  echo "Error: Generated markdown documentation not found at docs/manpage.md"
  exit 1
fi

# Create a backup of the original file for comparison
cp docs/manpage.md /tmp/original_markdown.md

# Generate manpages and markdown documentation with normalized version
CARGO_PKG_VERSION="$NORMALIZED_VERSION" cargo build

# Compare with the newly generated version
if ! diff -u /tmp/original_markdown.md docs/manpage.md > /tmp/markdown.diff; then
  echo "Error: Markdown documentation is out of date. A patch file has been created at /tmp/markdown.diff"
  echo ""
  echo "To fix this, run:"
  echo "   ./scripts/generate-manpages.sh"
  echo ""
  echo "Or with a custom version:"
  echo "   GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh"
  echo ""
  echo "The markdown documentation is automatically generated during the build process using clap_markdown."
  exit 1
fi

echo "Markdown documentation is up to date ✓"