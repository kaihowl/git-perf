#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Basic audit tests

cd_temp_repo
# mean: 2, std: 1
git checkout HEAD~3
git perf add -m timer 1
git checkout - && git checkout HEAD~2
git perf add -m timer 2
git checkout - && git checkout HEAD~1
git perf add -m timer 3
# head commit
git checkout -
git perf add -m timer 4
# measure
git perf audit -m timer -d 4
git perf audit -m timer -d 3
git perf audit -m timer -d 2
git perf audit -m timer -d 1.9999 && exit 1
git perf audit -m timer -d 1 && exit 1


echo Initial measurements with too few data points
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
git perf audit -m timer -d 3
git perf audit -m timer -d 2 && exit 1
git perf audit -m timer -d 2 --min-measurements 10
git perf audit -m timer -d 2 --min-measurements 4
git perf audit -m timer -d 2 --min-measurements 3 && exit 1


echo Stable measurements with zero stddev
cd_empty_repo
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 4
git perf audit -m timer && exit 1

exit 0
