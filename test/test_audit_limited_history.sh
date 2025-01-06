#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
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
git perf add -m timer 3
git checkout master
git perf audit -m timer
echo Two historical measurements available
git checkout HEAD~2
git perf add -m timer 3.5
git checkout master
git perf audit -m timer

cd_temp_repo
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 3
git checkout master
git perf audit -m timer && exit 1

echo Only measurements for different value available
cd_temp_repo
git checkout HEAD~1
git perf add -m othertimer 3
git checkout master
git perf add -m othertimer 3
git perf audit -m timer && exit 1
echo New measurement for HEAD but only historical measurements for different measurements
git perf add -m timer 3
git perf audit -m timer

echo New measurement not acceptable, but min_measurements not reached, therefore accept
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 2
git checkout master
git perf add -m timer 3
git perf audit -m timer --min-measurements 1 && exit 1
git perf audit -m timer --min-measurements 2

exit 0
