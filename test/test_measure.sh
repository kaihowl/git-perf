#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Missing arguments
cd_temp_repo
assert_failure_with_output output git perf measure -m test-measure
re=".*following (required )?arguments.*"
assert_matches "$output" "$re"

echo Non-existing command
cd_temp_repo
assert_failure git perf measure -m test-measure -- does-not-exist

echo Valid command, repeated measurements
cd_temp_repo
git perf measure -m test-measure -n 5 -- true
num_measurements=$(git perf report -o - | wc -l)
# CSV now includes header row, so 5 measurements + 1 header = 6 lines
[[ ${num_measurements} -eq 6 ]] || exit 1

echo Measurements in nanoseconds
cd_temp_repo
git perf measure -m test-measure -- bash -c 'sleep 0.1'
# Skip header row (first line) and get the timestamp from first data row
val=$(git perf report -o - | tail -n +2 | cut -f4 | head -n 1)
if [[ 1 -eq "$(echo "${val} < 10^(9-1)" | bc)" ]]; then
    test_section "Measure is not in nanosecond precision"
    test_section "0.1 seconds of sleep + fork + etc. overhead is currently $val"
    exit 1
fi

test_section "Measurement with padding spaces (argparse)"
cd_temp_repo
git perf add -m test-measure  0.5
# Skip header row and get value (field 5) from first data row
val=$(git perf report -o - | tail -n +2 | cut -f5 | head -n 1)
if [[ $val != 0.5 ]]; then
  echo "Unexpected measurement of val '${val}'. Expected 0.5 instead."
  exit 1
fi

test_section "Measurement with padding spaces (quoted)"
cd_temp_repo
assert_failure_with_output output git perf add -m test-measure " 0.5 "
re=".*invalid float.*"
assert_matches "$output" "re"

test_stats
exit 0
