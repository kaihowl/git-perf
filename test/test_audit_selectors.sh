#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "New measure with selector, only historical measurements with a different selector"
cd_temp_repo
assert_success git checkout HEAD~1
assert_success git perf add -m timer 4 -k otherselector=test
assert_success git checkout master
assert_success git perf add -m timer 4 -k myselector=test
assert_success git perf audit -m timer -s myselector=test

test_section "New measure with selector, only historical measurements with the same selector but different value"
cd_temp_repo
assert_success git checkout HEAD~1
assert_success git perf add -m timer 4 -k myselector=other
assert_success git checkout master
assert_success git perf add -m timer 4 -k myselector=test
assert_success git perf audit -m timer -s myselector=test

test_section "New non-matching measures, only historical measurements with matching key and value"
cd_temp_repo
assert_success git checkout HEAD~1
assert_success git perf add -m timer 4 -k myselector=test
assert_success git checkout master
assert_success git perf add -m timer 4
assert_failure git perf audit -m timer -s myselector=test
assert_success git perf add -m timer 4 -k otherselector=test
assert_failure git perf audit -m timer -s myselector=test
assert_success git perf add -m timer 4 -k myselector=other
assert_failure git perf audit -m timer -s myselector=test
assert_success git perf add -m timer 4 -k myselector=test
assert_success git perf audit -m timer -s myselector=test

test_stats
exit 0
