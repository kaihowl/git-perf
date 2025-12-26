#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Test bump-epoch with multiple -m flags
cd_empty_repo
create_commit
git perf add -m metric1 100
git perf add -m metric2 200
git perf add -m metric3 300

# Bump epochs for multiple measurements with multiple -m flags
git perf bump-epoch -m metric1 -m metric2 -m metric3

# Verify that all three measurements have epochs in the config
grep -q 'metric1' .gitperfconfig
grep -q 'metric2' .gitperfconfig
grep -q 'metric3' .gitperfconfig

# Verify the config file has proper structure
grep -q '\[measurement.metric1\]' .gitperfconfig
grep -q '\[measurement.metric2\]' .gitperfconfig
grep -q '\[measurement.metric3\]' .gitperfconfig

# Test that we can still use a single -m flag (backwards compatibility)
git perf bump-epoch -m metric1

test_stats
exit 0
