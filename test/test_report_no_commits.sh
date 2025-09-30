#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test error message when no commits exist in cloned repository"

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
# This test validates the improved error message added to address the TODO
# about better messaging when git push is missing before shallow clone operations
output=$(git perf report 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "No commits found in repository" "Missing 'No commits found in repository' in output"
assert_output_contains "$output" "Ensure commits exist and were pushed to the remote" "Missing guidance about pushing to remote in output"

echo "Test passed: Correct error message shown for repository with no commits"

exit 0