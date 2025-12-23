#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Missing arguments"
cd_temp_repo
assert_failure output git perf measure -m test-measure
assert_matches "$output" ".*following (required )?arguments.*" "Missing 'following arguments' in output"

test_section "Non-existing command"
cd_temp_repo
assert_failure git perf measure -m test-measure -- does-not-exist

test_section "Valid command, repeated measurements"
cd_temp_repo
git perf measure -m test-measure -n 5 -- true
num_measurements=$(git perf report -o - | wc -l)
# CSV now includes header row, so 5 measurements + 1 header = 6 lines
assert_equals "$num_measurements" "6" "Expected 6 lines (5 measurements + 1 header)"

test_section "Measurements in nanoseconds"
cd_temp_repo
git perf measure -m test-measure -- bash -c 'sleep 0.1'
# Skip header row (first line) and get the timestamp from first data row
val=$(git perf report -o - | tail -n +2 | cut -f4 | head -n 1)
# Check if value is at least 10^8 (0.1 seconds in nanoseconds should be around 10^8)
result=$(echo "${val} >= 10^(9-1)" | bc)
assert_equals "$result" "1" "Measure should be in nanosecond precision (0.1s sleep + overhead should be >= 10^8 ns)"

test_section "Measurement with padding spaces (argparse)"
cd_temp_repo
git perf add -m test-measure  0.5
# Skip header row and get value (field 5) from first data row
val=$(git perf report -o - | tail -n +2 | cut -f5 | head -n 1)
assert_equals "$val" "0.5" "Expected measurement value 0.5"

test_section "Measurement with padding spaces (quoted)"
cd_temp_repo
assert_failure output git perf add -m test-measure " 0.5 "
assert_matches "$output" ".*invalid float.*" "Error message should mention problem with float literal value"

test_stats
exit 0
