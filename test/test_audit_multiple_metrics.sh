#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test that audit fails with no -m"
cd_temp_repo
assert_failure output git perf audit
assert_contains "$output" "required"

test_section "Reset for the rest of the tests"
cd_temp_repo

test_section "Create measurements for two different metrics"
assert_success git checkout HEAD~3
assert_success git perf add -m timer 1
assert_success git perf add -m memory 100

assert_success git checkout master
assert_success git checkout HEAD~2
assert_success git perf add -m timer 2
assert_success git perf add -m memory 110

assert_success git checkout master
assert_success git checkout HEAD~1
assert_success git perf add -m timer 3
assert_success git perf add -m memory 120

test_section "Head commit"
assert_success git checkout master
assert_success git perf add -m timer 4
assert_success git perf add -m memory 130

test_section "Test auditing multiple metrics at once"
assert_success git perf audit -m timer -m memory -d 4
assert_success git perf audit -m timer -m memory -d 3
assert_success git perf audit -m timer -m memory -d 2

test_section "Test that it fails when one metric is outside the acceptable range"
assert_success git checkout master
assert_success create_commit
assert_success git perf add -m timer 10
assert_success git perf add -m memory 130
assert_failure output git perf audit -m timer -m memory -d 2
assert_contains "$output" "❌ 'timer'"
assert_contains "$output" "One or more measurements failed audit"

test_section "Test that multiple failing metrics are all reported"
assert_success git reset --hard HEAD~1
assert_success create_commit
assert_success git perf add -m timer 15
assert_success git perf add -m memory 200
assert_failure output git perf audit -m timer -m memory -d 2
assert_contains "$output" "❌ 'timer'"
assert_contains "$output" "❌ 'memory'"
assert_contains "$output" "One or more measurements failed audit"

test_section "Test with only one metric (backward compatibility)"
cd_temp_repo
assert_success git perf add -m timer 4
assert_success git perf audit -m timer -d 4

test_section "Test with three metrics"
assert_success git checkout master
assert_success git perf add -m timer 4
assert_success git perf add -m memory 130
assert_success git perf add -m cpu 50

assert_success git checkout HEAD~1
assert_success git perf add -m timer 3
assert_success git perf add -m memory 120
assert_success git perf add -m cpu 45

assert_success git checkout master
assert_success git perf audit -m timer -m memory -m cpu -d 4

test_stats
exit 0 