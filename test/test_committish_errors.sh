#!/bin/bash

# Test error handling for committish arguments
# Tests that invalid committish references are handled gracefully with clear error messages

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Set PATH to use the built git-perf binary
export PATH="$(cd "$script_dir/.." && pwd)/target/debug:$PATH"

echo "Test 1: Add with nonexistent commit SHA"
cd_temp_repo

# Try to add measurement to nonexistent commit
output=$(git perf add --commit deadbeefdeadbeefdeadbeefdeadbeefdeadbeef -m error_test 100.0 2>&1) && exit 1

# Should fail with error about unresolved commit
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 2: Add with invalid committish format"
cd_temp_repo

# Try to add with completely invalid committish
output=$(git perf add --commit "not@valid#commit" -m error_test 100.0 2>&1) && exit 1

# Should contain error message
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 3: Add with nonexistent branch"
cd_temp_repo

# Try to add to nonexistent branch
output=$(git perf add --commit nonexistent-branch -m error_test 100.0 2>&1) && exit 1

# Should fail with resolution error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 4: Add with nonexistent tag"
cd_temp_repo

# Try to add to nonexistent tag
output=$(git perf add --commit v99.99.99 -m error_test 100.0 2>&1) && exit 1

# Should fail with resolution error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 5: Measure with invalid committish"
cd_temp_repo

# Try to measure to invalid commit
output=$(git perf measure --commit invalid_commit -m error_test -- echo "test" 2>&1) && exit 1

# Should contain error message
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 6: Report from nonexistent commit"
cd_temp_repo
git perf add -m test_metric 100.0

# Try to report from nonexistent commit
output=$(git perf report deadbeefdeadbeefdeadbeefdeadbeefdeadbeef -o - 2>&1) && exit 1

# Should fail with clear error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 7: Audit from nonexistent commit"
cd_temp_repo
git perf add -m test_metric 100.0

# Try to audit nonexistent commit
output=$(git perf audit nonexistent_commit -m test_metric 2>&1) && exit 1

# Should fail with clear error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 8: Add with empty repository HEAD"
cd_empty_repo

# Try to add to HEAD in empty repo (no commits yet)
output=$(git perf add --commit HEAD -m error_test 100.0 2>&1) && exit 1

# Should fail because HEAD doesn't exist yet
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 9: Report from empty repository"
cd_empty_repo

# Try to report from empty repository
output=$(git perf report HEAD -o - 2>&1) && exit 1

# Should fail with clear message about missing HEAD
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 10: Add with HEAD~N beyond repository history"
cd_empty_repo
create_commit

# Only one commit exists, try HEAD~5
output=$(git perf add --commit HEAD~5 -m error_test 100.0 2>&1) && exit 1

# Should fail because HEAD~5 doesn't exist
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 11: Invalid HEAD~format"
cd_temp_repo

# Try with invalid HEAD~format
output=$(git perf add --commit "HEAD~invalid" -m error_test 100.0 2>&1) && exit 1

# Should fail with resolution error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 12: Add with abbreviated SHA that doesn't exist"
cd_temp_repo

# Try with short SHA that doesn't exist
output=$(git perf add --commit abcdef1 -m error_test 100.0 2>&1) && exit 1

# Should fail with resolution error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

echo "Test 13: Measure with HEAD^ in single-commit repo"
cd_empty_repo
create_commit

# Try HEAD^ when only one commit exists
output=$(git perf measure --commit "HEAD^" -m error_test -- echo "test" 2>&1) && exit 1

# Should fail because parent doesn't exist
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

# Tests 14 and 15 are skipped - they test edge cases with no/insufficient data
# which is a separate concern from committish validation

echo "All committish error tests passed!"
exit 0
