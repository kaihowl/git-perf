#!/bin/bash

# Script to generate manpages and markdown documentation
# This script uses a normalized version number to avoid version-based diffs in generated docs

set -e

# Default version to use for documentation generation (can be overridden with GIT_PERF_VERSION env var)
NORMALIZED_VERSION="${GIT_PERF_VERSION:-0.0.0}"

echo "Generating manpages and markdown documentation with version: $NORMALIZED_VERSION"

# Generate manpages and markdown documentation with normalized version
# Set version for both crates to ensure consistent documentation
GIT_PERF_DOC_VERSION="$NORMALIZED_VERSION" cargo build --package git_perf_cli_types --package git-perf

echo "Manpage and markdown documentation generated successfully ✓"