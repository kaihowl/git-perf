#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

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

exit 0
