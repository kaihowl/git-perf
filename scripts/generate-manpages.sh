#!/bin/bash

# Script to generate manpages and markdown documentation
# This script uses a normalized version number to avoid version-based diffs in generated docs

set -e

# Default version to use for documentation generation (can be overridden with GIT_PERF_VERSION env var)
NORMALIZED_VERSION="${GIT_PERF_VERSION:-0.0.0}"

echo "Generating manpages and markdown documentation with version: $NORMALIZED_VERSION"

# Check that the generated files exist
if [[ ! -f "docs/manpage.md" ]]; then
  echo "Error: Generated markdown documentation not found at docs/manpage.md"
  exit 1
fi
if [[ ! -f "man/man1/git-perf.1" ]]; then
  echo "Error: Generated manpage not found at man/man1/git-perf.1"
  exit 1
fi

# Create backups of the original files for comparison
cp docs/manpage.md /tmp/original_markdown.md
cp man/man1/git-perf.1 /tmp/original_manpage.1

# Generate manpages and markdown documentation with normalized version
GIT_PERF_DOC_VERSION="$NORMALIZED_VERSION" cargo build --package git_perf_cli_types --package git-perf

# Compare markdown documentation
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

# Compare manpage documentation
if ! diff -u /tmp/original_manpage.1 man/man1/git-perf.1 > /tmp/manpage.diff; then
  echo "Error: Manpage documentation is out of date. A patch file has been created at /tmp/manpage.diff"
  echo ""
  echo "To fix this, run:"
  echo "   ./scripts/generate-manpages.sh"
  echo ""
  echo "Or with a custom version:"
  echo "   GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh"
  echo ""
  echo "The manpage documentation is automatically generated during the build process using clap_mangen."
  exit 1
fi
echo "Manpage documentation is up to date ✓"