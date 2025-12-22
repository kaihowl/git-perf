#!/bin/bash

# Disable verbose tracing (recommended for new tests)
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Output into broken pipe"

cd_empty_repo
create_commit
assert_success git perf add -m test 2
create_commit
assert_success git perf add -m test 4
create_commit

# Test that git perf report handles broken pipe gracefully (piping to 'true')
git perf report -o - | true
assert_equals "${PIPESTATUS[0]}" "0" "git-perf should handle broken pipe gracefully"

test_stats
exit 0
