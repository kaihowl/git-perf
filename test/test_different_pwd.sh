#!/bin/bash

# Disable verbose tracing (recommended for new tests)
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "New repo but current working directory different"

cd_empty_repo
create_commit
assert_success git perf add -m test-measure 10

work_dir=$(pwd)
cd /tmp

# Test that git-perf works when invoked with -C from a different directory
assert_success_with_output output git -C "$work_dir" perf report -o -
assert_contains "$output" "test-measure" "Failed to retrieve measurement from different working directory"

test_stats
exit 0
