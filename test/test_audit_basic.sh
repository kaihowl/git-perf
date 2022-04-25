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

exit 0
