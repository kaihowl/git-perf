#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Setup bare repository and working repos"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" myworkrepo
pushd myworkrepo
myworkrepo=$(pwd)

touch a
git add a
git commit -m 'first commit'

git push

popd

git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

test_section "Init git perf in two repos independently"

pushd "$repo1"

git perf add -m echo 0.5

git perf push

popd

pushd "$repo2"

git perf add -m echo 0.5

assert_success output git perf push
assert_contains "$output" "retrying" "Output is missing 'retrying'"

popd

test_section "Check number of measurements from myworkrepo"

pushd "$myworkrepo"

git perf pull
assert_success report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# CSV now includes header row, so 2 measurements + 1 header = 3 lines
assert_equals "$num_measurements" "3" "Expected two measurements (3 lines with header)"

popd

test_stats
exit 0
