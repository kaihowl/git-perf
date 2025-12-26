#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git perf size functionality

test_section "Test 1: Empty repository (no measurements)"
cd_empty_repo
output=$(git perf size 2>&1)
assert_contains "$output" "0" "Expected 0 commits with measurements"
assert_contains "$output" "Total measurement data size" "Expected size info"
popd

test_section "Test 2: Single commit with measurements"
cd_empty_repo
create_commit
git perf add -m test-measure-one 10.0
git perf add -m test-measure-two 20.0

output=$(git perf size)
assert_contains "$output" "1" "Expected 1 commit with measurements"
assert_contains "$output" "Number of commits with measurements" "Expected commit count info"
popd

test_section "Test 3: Multiple commits with measurements"
cd_empty_repo
create_commit
git perf add -m test-measure-one 10.0
git perf add -m test-measure-two 20.0

create_commit
git perf add -m test-measure-one 15.0
git perf add -m test-measure-three 25.0

output=$(git perf size)
assert_contains "$output" "2" "Expected 2 commits"
assert_contains "$output" "measurement" "Expected measurement info"
popd

test_section "Test 4: Detailed output shows measurement names"
cd_empty_repo
create_commit
git perf add -m test-measure-one 10.0
git perf add -m test-measure-two 20.0

create_commit
git perf add -m test-measure-one 15.0
git perf add -m test-measure-three 25.0

output=$(git perf size --detailed)
assert_contains "$output" "test-measure-one" "Expected measurement breakdown"
assert_contains "$output" "test-measure-two" "Expected measurement breakdown"
assert_contains "$output" "test-measure-three" "Expected measurement breakdown"
assert_contains "$output" "occurrences" "Expected occurrence count"
popd

test_section "Test 5: Bytes format shows numeric values"
cd_empty_repo
create_commit
git perf add -m test-measure 42.0

output=$(git perf size --format bytes)
assert_contains "$output" "Total measurement data size" "Expected size label"
# Should contain numeric bytes value (at least 2 digits)
assert_matches "$output" "[0-9][0-9]+"
popd

test_section "Test 6: Disk size flag works"
cd_empty_repo
create_commit
git perf add -m test-measure 42.0

output=$(git perf size --disk-size)
assert_contains "$output" "Total measurement data size" "Expected size info"
# Should succeed without errors
popd

test_section "Test 7: Include objects flag shows repository stats"
cd_empty_repo
create_commit
git perf add -m test-measure 42.0

output=$(git perf size --include-objects)
assert_contains "$output" "Repository Statistics" "Expected repo stats section"
assert_contains "$output" "objects" "Expected objects info"
popd

test_section "Test 8: Detailed + bytes format combination"
cd_empty_repo
create_commit
git perf add -m test-measure-one 10.0
git perf add -m test-measure-two 20.0

output=$(git perf size --detailed --format bytes)
assert_contains "$output" "test-measure-one" "Expected measurement breakdown"
assert_matches "$output" "[0-9][0-9]+"
popd

test_section "Test 9: Size changes after adding more measurements"
cd_empty_repo
create_commit
git perf add -m test-measure 10.0

output1=$(git perf size --format bytes)
# Extract the size value
size1=$(echo "$output1" | grep "Total measurement data size" | grep -o '[0-9][0-9]*' | head -1)

create_commit
git perf add -m test-measure 20.0

output2=$(git perf size --format bytes)
size2=$(echo "$output2" | grep "Total measurement data size" | grep -o '[0-9][0-9]*' | head -1)

# Second size should be larger than first
if [ "$size2" -le "$size1" ]; then
    test_section "Expected size to increase after adding measurements"
    test_section "Size 1: $size1"
    test_section "Size 2: $size2"
    exit 1
fi
popd

test_section "Test 10: Detailed breakdown shows correct occurrence counts"
cd_empty_repo
create_commit
git perf add -m repeated-measure 10.0

create_commit
git perf add -m repeated-measure 20.0

create_commit
git perf add -m repeated-measure 30.0

output=$(git perf size --detailed)
assert_contains "$output" "repeated-measure" "Expected measurement name"
assert_contains "$output" "3 occurrences" "Expected 3 occurrences"
popd

test_section "All size tests passed!"
test_stats
exit 0
