#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Basic audit tests"

cd_temp_repo
# mean: 2, std: 1
git checkout HEAD~3
git perf add -m timer 1
git checkout master && git checkout HEAD~2
git perf add -m timer 2
git checkout master && git checkout HEAD~1
git perf add -m timer 3
# head commit
git checkout master
git perf add -m timer 4
# measure
assert_success git perf audit -m timer -d 4
assert_success git perf audit -m timer -d 3
assert_success git perf audit -m timer -d 2
assert_failure git perf audit -m timer -d 1.9999
assert_failure git perf audit -m timer -d 1

test_section "Initial measurements with too few data points"
cd_empty_repo
# mean: 15, std: 5
create_commit
git perf add -m timer 10
create_commit
git perf add -m timer 15
# Add second measurement. Due to "min" and "group by commit": No effect.
# But: Should also not be counted for min-measurements.
git perf add -m timer 15
create_commit
git perf add -m timer 20
# head commit
create_commit
git perf add -m timer 30
# measure
assert_success git perf audit -m timer -d 3
assert_failure git perf audit -m timer -d 2
assert_success git perf audit -m timer -d 2 --min-measurements 10
assert_success git perf audit -m timer -d 2 --min-measurements 4
assert_failure git perf audit -m timer -d 2 --min-measurements 3

test_section "Stable measurements with zero stddev"
cd_empty_repo
create_commit
git perf add -m timer 3
assert_success git perf audit -m timer
create_commit
git perf add -m timer 3
assert_success git perf audit -m timer
create_commit
git perf add -m timer 3
assert_success git perf audit -m timer
create_commit
git perf add -m timer 4
assert_failure git perf audit -m timer

test_stats
exit 0
