#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Setup repository without seed measurements"

# Create a fresh repository environment without seed measurement
cd "$(mktemp -d)"
root=$(pwd)

# Set up fresh repository without seed measurement
mkdir upstream_noseed
pushd upstream_noseed > /dev/null
git init --bare > /dev/null 2>&1
popd > /dev/null

git clone "$root/upstream_noseed" work_noseed > /dev/null 2>&1
pushd work_noseed > /dev/null
git commit --allow-empty -m 'test commit without seed' > /dev/null 2>&1
git push > /dev/null 2>&1

test_section "Testing remove operation without seed measurements"

# Test remove operation without any measurements - should fail with expected error
assert_failure_with_output output git perf remove --older-than '7d'
assert_contains "$output" "Remote repository is empty" "Expected error about empty remote repository"

test_section "Testing prune operation without seed measurements"

# Test prune operation without any measurements - should fail with expected error
assert_failure_with_output output git perf prune
assert_contains "$output" "Remote repository is empty" "Expected error about empty remote repository"

test_section "Testing report operation without seed measurements"

# Test report operation without any measurements - should fail with expected error
assert_failure_with_output output git perf report -o -
assert_contains "$output" "No performance measurements found" "Expected error about no measurements"

popd > /dev/null  # exit work_noseed

test_stats
exit 0