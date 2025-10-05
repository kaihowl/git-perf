#!/bin/bash

# Script to generate manpages and markdown documentation
# Generates docs in OUT_DIR (cargo publish compliant), then copies to repo for version control

set -e

echo "Generating manpages and markdown documentation (manpages without version)"

# Build to generate docs in OUT_DIR
cargo build --package git_perf_cli_types --package git-perf

# Find the OUT_DIR by looking for the generated files
OUT_DIR=$(find target/debug/build/git-perf-*/out -type d 2>/dev/null | head -n 1)

if [[ -z "$OUT_DIR" ]]; then
  echo "Error: Could not find OUT_DIR in target/debug/build/git-perf-*/out"
  exit 1
fi

echo "Found generated docs in: $OUT_DIR"

# Ensure the repository directories exist
mkdir -p man/man1
mkdir -p docs

# Copy generated files from OUT_DIR to repository
cp -r "$OUT_DIR/man/man1/"*.1 man/man1/
cp "$OUT_DIR/docs/manpage.md" docs/manpage.md

echo "✓ Manpages and documentation copied to repository"

# Validate that files were copied successfully
if [[ ! -f "docs/manpage.md" ]]; then
  echo "Error: Failed to copy markdown documentation to docs/manpage.md"
  exit 1
fi
if [[ ! -f "man/man1/git-perf.1" ]]; then
  echo "Error: Failed to copy manpage to man/man1/git-perf.1"
  exit 1
fi

echo "✓ Documentation generation complete"
