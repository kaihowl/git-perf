#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Basic audit tests

cd_temp_repo
git checkout HEAD~3
git perf add -m timer 1
git checkout - && git checkout HEAD~2
git perf add -m timer 2
git checkout - && git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf add -m timer 4
# mean: 2, std: 1
git perf audit -m timer -d 4
git perf audit -m timer -d 3
git perf audit -m timer -d 2
git perf audit -m timer -d 1.9999 && exit 1
git perf audit -m timer -d 1 && exit 1

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

echo Check audit with different measurements available
cd_temp_repo
echo No measurements available
git perf audit -m timer && exit 1
echo Only HEAD measurement available
git perf add -m timer 3
git perf audit -m timer
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 4
git checkout -
git perf audit -m timer
echo Two historical measurements available
git checkout HEAD~2
git perf add -m timer 3.5
git checkout -
git perf audit -m timer

cd_temp_repo
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf audit -m timer && exit 1

echo Only measurements for different value available
cd_temp_repo
git checkout HEAD~1
git perf add -m othertimer 3
git checkout -
git perf add -m othertimer 3
git perf audit -m timer && exit 1
echo New measurement for HEAD but only historical measurements for different measurements
git perf add -m timer 3
git perf audit -m timer

echo Only single historical measurement available, should accept new measurement
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf add -m timer 4
git perf audit -m timer

echo Two historical measurements available, and acceptable new measurement
cd_temp_repo
git checkout HEAD~2
git perf add -m timer 3
git checkout -
git checkout HEAD~1
git perf add -m timer 4
git checkout -
git perf add -m timer 5
git perf audit -m timer

echo New measure with selector, only historical measurements with a different selector
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv otherselector=test
git checkout -
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

echo New measure with selector, only historical measurements with the same selector but different value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv myselector=other
git checkout -
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

echo New non-matching measures, only historical measurements with matching key and value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv myselector=test
git checkout -
git perf add -m timer 4
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv otherselector=test
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv myselector=other
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

exit 0
