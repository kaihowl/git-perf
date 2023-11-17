#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo New measure with selector, only historical measurements with a different selector
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -k otherselector=test
git checkout master
git perf add -m timer 4 -k myselector=test
git perf audit -m timer -s myselector=test

echo New measure with selector, only historical measurements with the same selector but different value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -k myselector=other
git checkout master
git perf add -m timer 4 -k myselector=test
git perf audit -m timer -s myselector=test

echo New non-matching measures, only historical measurements with matching key and value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -k myselector=test
git checkout master
git perf add -m timer 4
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -k otherselector=test
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -k myselector=other
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -k myselector=test
git log --format=fuller --notes=refs/notes/perf
git perf audit -m timer -s myselector=test

exit 0
