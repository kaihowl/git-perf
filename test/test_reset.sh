#!/bin/bash

set -e

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

## Test git perf reset functionality

cd_empty_repo
create_commit

echo "Test 1: Reset with no pending measurements"
output=$(git perf reset --force)
assert_output_contains "$output" "No pending measurements" "Expected empty reset"

echo "Test 2: Dry run"
git perf add -m test-measure 10.0
output=$(git perf reset --dry-run)
assert_output_contains "$output" "Dry run" "Expected dry-run indicator"
# Verify measurements still exist
output=$(git perf status)
assert_output_contains "$output" "test-measure" "Expected measurements to remain after dry-run"

echo "Test 3: Force reset"
git perf reset --force
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty after reset"

echo "Test 4: Multiple measurements and commits"
create_commit
git perf add -m measure1 100.0
create_commit
git perf add -m measure2 200.0
git perf add -m measure3 300.0

output=$(git perf status)
assert_output_contains "$output" "2 commit(s)" "Expected 2 commits with measurements"

# Reset should clear all
git perf reset --force
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty after reset"

echo "All reset tests passed!"
