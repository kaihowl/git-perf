#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git perf size functionality

test_section "Empty repository (no measurements)"
cd_empty_repo
assert_success output git perf size
assert_contains "$output" "0"
assert_contains "$output" "Total measurement data size"
popd

test_section "Single commit with measurements"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure-one 10.0
assert_success git perf add -m test-measure-two 20.0

assert_success output git perf size
assert_contains "$output" "1"
assert_contains "$output" "Number of commits with measurements"
popd

test_section "Multiple commits with measurements"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure-one 10.0
assert_success git perf add -m test-measure-two 20.0

create_commit
assert_success git perf add -m test-measure-one 15.0
assert_success git perf add -m test-measure-three 25.0

assert_success output git perf size
assert_contains "$output" "2"
assert_contains "$output" "measurement"
popd

test_section "Detailed output shows measurement names"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure-one 10.0
assert_success git perf add -m test-measure-two 20.0

create_commit
assert_success git perf add -m test-measure-one 15.0
assert_success git perf add -m test-measure-three 25.0

assert_success output git perf size --detailed
assert_contains "$output" "test-measure-one"
assert_contains "$output" "test-measure-two"
assert_contains "$output" "test-measure-three"
assert_contains "$output" "occurrences"
popd

test_section "Bytes format shows numeric values"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure 42.0

assert_success output git perf size --format bytes
assert_contains "$output" "Total measurement data size"
assert_matches "$output" "[0-9][0-9]+"
popd

test_section "Disk size flag works"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure 42.0

assert_success output git perf size --disk-size
assert_contains "$output" "Total measurement data size"
popd

test_section "Include objects flag shows repository stats"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure 42.0

assert_success output git perf size --include-objects
assert_contains "$output" "Repository Statistics"
assert_contains "$output" "objects"
popd

test_section "Detailed + bytes format combination"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure-one 10.0
assert_success git perf add -m test-measure-two 20.0

assert_success output git perf size --detailed --format bytes
assert_contains "$output" "test-measure-one"
assert_matches "$output" "[0-9][0-9]+"
popd

test_section "Size changes after adding more measurements"
cd_empty_repo
create_commit
assert_success git perf add -m test-measure 10.0

assert_success output1 git perf size --format bytes
size1=$(echo "$output1" | grep "Total measurement data size" | grep -o '[0-9][0-9]*' | head -1)

create_commit
assert_success git perf add -m test-measure 20.0

assert_success output2 git perf size --format bytes
size2=$(echo "$output2" | grep "Total measurement data size" | grep -o '[0-9][0-9]*' | head -1)

assert_true '[[ $size2 -gt $size1 ]]' "Size should increase after adding measurements"
popd

test_section "Detailed breakdown shows correct occurrence counts"
cd_empty_repo
create_commit
assert_success git perf add -m repeated-measure 10.0

create_commit
assert_success git perf add -m repeated-measure 20.0

create_commit
assert_success git perf add -m repeated-measure 30.0

assert_success output git perf size --detailed
assert_contains "$output" "repeated-measure"
assert_contains "$output" "3 occurrences"
popd

test_stats
exit 0
