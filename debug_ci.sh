#!/bin/bash

echo "=== CI DEBUG SCRIPT ==="
echo "Current working directory: $(pwd)"
echo "Environment variables:"
echo "  OUT_DIR: ${OUT_DIR:-not set}"
echo "  CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-not set}"
echo "  CARGO_MANIFEST_DIR: ${CARGO_MANIFEST_DIR:-not set}"

echo ""
echo "Directory structure:"
echo "  Workspace root contents:"
ls -la . || echo "Failed to list workspace root"

echo ""
echo "  Target directory contents:"
ls -la target/ || echo "Target directory does not exist"

echo ""
echo "  Man directory contents:"
ls -la man/ || echo "Man directory does not exist"

echo ""
echo "  Docs directory contents:"
ls -la docs/ || echo "Docs directory does not exist"

echo ""
echo "Running cargo build with verbose output:"
cargo build --verbose

echo ""
echo "After build - checking directories again:"
echo "  Man directory contents:"
ls -la man/ || echo "Man directory does not exist"

echo ""
echo "  Docs directory contents:"
ls -la docs/ || echo "Docs directory does not exist"

echo ""
echo "Running tests with verbose output:"
cargo test --test manpage_tests -- --nocapture

echo "=== END CI DEBUG SCRIPT ==="