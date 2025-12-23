#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test bump-epoch with multiple -m flags"

cd_empty_repo
create_commit
git perf add -m metric1 100
git perf add -m metric2 200
git perf add -m metric3 300

# Bump epochs for multiple measurements with multiple -m flags
assert_success git perf bump-epoch -m metric1 -m metric2 -m metric3

# Verify that all three measurements have epochs in the config
assert_success config cat .gitperfconfig
assert_contains "$config" "metric1"
assert_contains "$config" "metric2"
assert_contains "$config" "metric3"

# Verify the config file has proper structure
assert_contains "$config" "[measurement.metric1]"
assert_contains "$config" "[measurement.metric2]"
assert_contains "$config" "[measurement.metric3]"

test_section "Test backwards compatibility with single -m flag"

# Test that we can still use a single -m flag (backwards compatibility)
assert_success git perf bump-epoch -m metric1

test_stats
exit 0
