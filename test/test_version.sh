#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)

# shellcheck source=test/common.sh
source "$script_dir/common.sh"

assert_failure_with_output output git perf
assert_contains "$output" "--help" "No warning for missing arguments"

output=$(git perf --version)
assert_matches "$output" "^(git-perf )?[0-9]+\.[0-9]+\.[0-9]+$"

# Git version too old
export PATH=${script_dir}/fake_git_2.40.0:$PATH
assert_failure git-perf add -m test 12

# Git version just right
export PATH=${script_dir}/fake_git_2.43.0:$PATH
git-perf add -m test 12

test_stats
exit 0
