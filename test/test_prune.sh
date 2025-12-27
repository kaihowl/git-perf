#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Setup test repository"

# Refuse to run on a shallow clone
pushd "$(mktemp -d)"
repo=$(pwd)
git init --bare
popd

pushd "$(mktemp -d)"
git clone "$repo" work/
cd work/
create_commit
create_commit
create_commit
git push
popd

test_section "Test prune refuses to run on shallow clone"

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --no-tags --prune --progress --no-recurse-submodules --depth=1 --update-head-ok origin master:master
assert_failure_with_output output git perf prune
assert_contains "$output" "shallow" "No warning for 'shallow' clone"
popd

test_section "Test running git perf prune outside of a git repository"

pushd "$(mktemp -d)"
assert_failure_with_output output git perf prune
# Check for either expected error message
if [[ $output != *'not a git repository'* ]]; then
  assert_contains "$output" "fatal" "Expected error for running outside a git repo"
fi
popd

test_section "Normal prune operations on main repo"

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --update-head-ok origin master:master

git perf add -m test 5

# Must push once to create the remote perf branch
git perf push

git perf prune

nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
assert_equals "$nr_notes" "1" "Expected to have 1 note after initial prune"

git reset --hard HEAD~1
git push --force origin master:master

nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
assert_equals "$nr_notes" "1" "Expected to still have 1 note before gc"

git reflog expire --expire-unreachable=now --all
git prune --expire=now
git perf prune
nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
assert_equals "$nr_notes" "0" "Expected to have no notes after pruning unreachable commits"

popd

test_stats
exit 0
