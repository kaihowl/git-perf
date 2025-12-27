#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo New repo, error out without crash
cd_empty_repo

output=$(git perf add -m 'test' 23 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "Missing HEAD" "Missing 'Missing HEAD' in output"

echo Empty repo with upstream
cd "$(mktemp -d)"
root=$(pwd)

mkdir orig
cd orig
orig=$(pwd)

git init --bare

cd "$(mktemp -d)"
git clone "$orig" myworkrepo

cd myworkrepo

output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "Failed to resolve commit 'HEAD'" "Missing 'Failed to resolve commit' in output"

touch a
git add a
git commit -m 'first commit'

git push

output=$(git perf report 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "No performance measurements found" "Missing 'No performance measurements found' in output"

output=$(git perf push 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "This repo does not have any measurements" "Missing 'This repo does not have any measurements' in output"

output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "No measurement for HEAD" "Missing 'No measurement for HEAD' in output"
