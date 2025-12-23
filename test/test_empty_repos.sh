#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "New repo, error out without crash"

cd_empty_repo

assert_failure output git perf add -m 'test' 23
assert_contains "$output" "Missing HEAD" "Missing 'Missing HEAD' in output"

test_section "Empty repo with upstream"
cd "$(mktemp -d)"
root=$(pwd)

mkdir orig
cd orig
orig=$(pwd)

git init --bare

cd "$(mktemp -d)"
git clone "$orig" myworkrepo

cd myworkrepo

assert_failure output git perf audit -m non-existent
assert_contains "$output" "No commit at HEAD" "Missing 'No Commit at HEAD' in output"

touch a
git add a
git commit -m 'first commit'

git push

assert_failure output git perf report
assert_contains "$output" "No performance measurements found" "Missing 'No performance measurements found' in output"

assert_failure output git perf push
assert_contains "$output" "This repo does not have any measurements" "Missing 'This repo does not have any measurements' in output"

assert_failure output git perf audit -m non-existent
assert_contains "$output" "No measurement for HEAD" "Missing 'No measurement for HEAD' in output"

test_stats
exit 0
