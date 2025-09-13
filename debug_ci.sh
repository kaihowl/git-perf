#!/bin/bash

echo "=== CI DEBUG SCRIPT ==="
echo "Current working directory: $(pwd)"
echo "Environment variables:"
echo "  OUT_DIR: ${OUT_DIR:-not set}"
echo "  CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-not set}"
echo "  CARGO_MANIFEST_DIR: ${CARGO_MANIFEST_DIR:-not set}"
echo "  GITHUB_WORKSPACE: ${GITHUB_WORKSPACE:-not set}"
echo "  GITHUB_ACTIONS: ${GITHUB_ACTIONS:-not set}"
echo "  RUNNER_WORKSPACE: ${RUNNER_WORKSPACE:-not set}"

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
echo "  Git-perf directory contents:"
ls -la git_perf/ || echo "Git-perf directory does not exist"

echo ""
echo "Path analysis:"
echo "  Absolute path to workspace: $(realpath .)"
echo "  Absolute path to target: $(realpath target/ 2>/dev/null || echo 'target does not exist')"
echo "  Absolute path to man: $(realpath man/ 2>/dev/null || echo 'man does not exist')"
echo "  Absolute path to docs: $(realpath docs/ 2>/dev/null || echo 'docs does not exist')"

echo ""
echo "Running cargo build with verbose output:"
cargo build --verbose 2>&1 | tee build_output.log

echo ""
echo "Build script warnings (if any):"
grep "BUILD SCRIPT DEBUG" build_output.log || echo "No build script debug output found"

echo ""
echo "After build - checking directories again:"
echo "  Man directory contents:"
ls -la man/ || echo "Man directory does not exist"

echo ""
echo "  Docs directory contents:"
ls -la docs/ || echo "Docs directory does not exist"

echo ""
echo "Running manpage tests with debug output:"
cargo test --test manpage_tests -- --nocapture 2>&1 | tee test_output.log

echo ""
echo "Test debug output:"
grep -A 20 -B 5 "MANPAGE TEST DEBUG INFO" test_output.log || echo "No test debug output found"

echo "=== END CI DEBUG SCRIPT ==="