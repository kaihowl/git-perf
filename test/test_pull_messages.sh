#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Pull in repo without a remote
cd_empty_repo

assert_failure output="$(git perf pull 2>&1 1>/dev/null)"
assert_contains "$output" "No upstream found" "Missing 'No upstream found' from output"

echo Pull from remote without measurements

cd "$(mktemp -d)"
root=$(pwd)

mkdir orig
cd orig
orig=$(pwd)

git init --bare

cd "$(mktemp -d)"
git clone "$orig" myworkrepo

cd myworkrepo

touch a
git add a
git commit -m 'first commit'

git push

assert_failure output="$(git perf pull 2>&1 1>/dev/null)"
assert_contains "$output" "Remote repository is empty or has never been pushed to" "Missing 'Remote repository is empty or has never been pushed to' in output"

cd "$root"
git clone "$orig" repo1
repo1=$(pwd)/repo1

cd "$repo1"

git perf add -m test-measure 12
git perf push

output=$(git perf pull 2>/dev/null) || exit 1
assert_contains "$output" "Already up to date" "Missing 'Already up to date' in output"