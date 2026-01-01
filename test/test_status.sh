#!/bin/bash

set -e

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

## Test git perf status functionality

cd_empty_repo
create_commit

echo "Test 1: No pending measurements"
output=$(git perf status)
assert_output_contains "$output" "No pending measurements" "Expected empty status"

echo "Test 2: Add measurements and check status"
git perf add -m test-measure 10.0
output=$(git perf status)
assert_output_contains "$output" "1 commit" "Expected 1 commit"
assert_output_contains "$output" "test-measure" "Expected measurement name"

echo "Test 3: Multiple measurements"
git perf add -m another-measure 20.0
output=$(git perf status)
assert_output_contains "$output" "2 unique measurements" "Expected 2 measurements"
assert_output_contains "$output" "test-measure" "Expected test-measure"
assert_output_contains "$output" "another-measure" "Expected another-measure"

echo "Test 4: Multiple commits with measurements"
create_commit
git perf add -m test-measure 15.0
output=$(git perf status)
assert_output_contains "$output" "2 commits" "Expected 2 commits"

echo "Test 5: Detailed output"
output=$(git perf status --detailed)
assert_output_contains "$output" "Per-commit breakdown" "Expected detailed view"

echo "All status tests passed!"
