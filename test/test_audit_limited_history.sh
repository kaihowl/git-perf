#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Check audit with different measurements available"

cd_temp_repo

test_section "No measurements available"
assert_failure git perf audit -m timer

test_section "Only HEAD measurement available"
assert_success git perf add -m timer 3
assert_success git perf audit -m timer

test_section "Only one historical measurement available"
assert_success git checkout HEAD~1
assert_success git perf add -m timer 3
assert_success git checkout master
assert_success git perf audit -m timer

test_section "Two historical measurements available"
assert_success git checkout HEAD~2
assert_success git perf add -m timer 3.5
assert_success git checkout master
assert_success git perf audit -m timer

cd_temp_repo

test_section "Only one historical measurement available - should fail"
assert_success git checkout HEAD~1
assert_success git perf add -m timer 3
assert_success git checkout master
assert_failure git perf audit -m timer

test_section "Only measurements for different value available"
cd_temp_repo
assert_success git checkout HEAD~1
assert_success git perf add -m othertimer 3
assert_success git checkout master
assert_success git perf add -m othertimer 3
assert_failure git perf audit -m timer

test_section "New measurement for HEAD but only historical measurements for different measurements"
assert_success git perf add -m timer 3
assert_success git perf audit -m timer

test_section "New measurement not acceptable, but min_measurements not reached, therefore accept"
cd_temp_repo
assert_success git checkout HEAD~1
assert_success git perf add -m timer 2
assert_success git checkout master
assert_success git perf add -m timer 3
assert_failure git perf audit -m timer --min-measurements 1
assert_success git perf audit -m timer --min-measurements 2

test_stats
exit 0
