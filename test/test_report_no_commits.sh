#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Error message when no commits exist in cloned repository"

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
# The error occurs when trying to resolve HEAD in an empty repository
assert_contains "$output" "Failed to resolve commit" "Missing 'Failed to resolve commit' error in output"

test_stats
exit 0