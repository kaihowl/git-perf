#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test that audit fails with no -m"
cd_temp_repo
assert_failure_with_output output git perf audit
assert_contains "$output" "required"

test_section "Reset for the rest of the tests"
cd_temp_repo

test_section "Create measurements for two different metrics"
git checkout HEAD~3
git perf add -m timer 1
git perf add -m memory 100

git checkout master
git checkout HEAD~2
git perf add -m timer 2
git perf add -m memory 110

git checkout master
git checkout HEAD~1
git perf add -m timer 3
git perf add -m memory 120

test_section "Head commit"
git checkout master
git perf add -m timer 4
git perf add -m memory 130

test_section "Test auditing multiple metrics at once"
assert_success git perf audit -m timer -m memory -d 4
assert_success git perf audit -m timer -m memory -d 3
assert_success git perf audit -m timer -m memory -d 2

test_section "Test that it fails when one metric is outside the acceptable range"
git checkout master
create_commit
git perf add -m timer 10
git perf add -m memory 130
assert_failure_with_output output git perf audit -m timer -m memory -d 2
assert_contains "$output" "❌ 'timer'"
assert_contains "$output" "One or more measurements failed audit"

test_section "Test that multiple failing metrics are all reported"
git reset --hard HEAD~1
create_commit
git perf add -m timer 15
git perf add -m memory 200
assert_failure_with_output output git perf audit -m timer -m memory -d 2
assert_contains "$output" "❌ 'timer'"
assert_contains "$output" "❌ 'memory'"
assert_contains "$output" "One or more measurements failed audit"

test_section "Test with only one metric (backward compatibility)"
cd_temp_repo
git perf add -m timer 4
assert_success git perf audit -m timer -d 4

test_section "Test with three metrics"
git checkout master
git perf add -m timer 4
git perf add -m memory 130
git perf add -m cpu 50

git checkout HEAD~1
git perf add -m timer 3
git perf add -m memory 120
git perf add -m cpu 45

git checkout master
assert_success git perf audit -m timer -m memory -m cpu -d 4

test_stats
exit 0 