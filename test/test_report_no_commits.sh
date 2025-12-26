#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test error message when no commits exist in cloned repository"

cd "$(mktemp -d)"

# Create a bare remote repository
mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

# Clone the empty repository (simulates shallow clone scenario where commits weren't pushed)
git clone "file://$orig" test_repo
cd test_repo

# Try to run report on empty repository - should fail with helpful message
assert_failure_with_output output git perf report
assert_contains "$output" "No commits found in repository" "Missing 'No commits found in repository' in output"
assert_contains "$output" "Ensure commits exist and were pushed to the remote" "Missing guidance about pushing to remote in output"

test_section "Test passed: Correct error message shown for repository with no commits"

test_stats
exit 0