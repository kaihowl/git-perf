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

test_section "Prune cleans up orphan staging refs from dead processes"

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --update-head-ok origin master:master

git perf add -m test-orphan-cleanup 5
git perf push

# Simulate a crashed process: spawn a subshell just to capture its PID, then let it exit.
# By the time we use the PID, the process is already dead.
dead_pid=$(bash -c 'echo $$')
dead_pid_hex=$(printf '%08x' "$dead_pid")

current_oid=$(git rev-parse HEAD)
git update-ref "refs/notes/perf-v3-add-${dead_pid_hex}-deadbeef" "$current_oid"
git update-ref "refs/notes/perf-v3-merge-${dead_pid_hex}-deadbeef" "$current_oid"

orphan_count=$(git for-each-ref 'refs/notes/perf-v3-add-*' 'refs/notes/perf-v3-merge-*' | wc -l | tr -d '[:space:]')
assert_equals "$orphan_count" "2" "Expected 2 orphan staging refs before prune"

git perf prune

orphan_count=$(git for-each-ref 'refs/notes/perf-v3-add-*' 'refs/notes/perf-v3-merge-*' | wc -l | tr -d '[:space:]')
assert_equals "$orphan_count" "0" "Expected 0 orphan staging refs after prune"

popd

test_section "Prune preserves staging refs from live processes"

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --update-head-ok origin master:master

git perf add -m test-live-ref 5
git perf push

# Create a staging ref with the current process's PID — this process is still alive
live_pid=$$
live_pid_hex=$(printf '%08x' "$live_pid")
current_oid=$(git rev-parse HEAD)
live_ref="refs/notes/perf-v3-add-${live_pid_hex}-cafebabe"
git update-ref "$live_ref" "$current_oid"

git perf prune

live_count=$(git for-each-ref "$live_ref" | wc -l | tr -d '[:space:]')
assert_equals "$live_count" "1" "Expected prune to preserve staging ref from live process (PID $$)"

# Clean up the ref we created
git update-ref -d "$live_ref"

popd

test_section "Prune preserves old-format staging refs (no PID prefix)"

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --update-head-ok origin master:master

git perf add -m test-old-format 5
git perf push

current_oid=$(git rev-parse HEAD)
# Old format: 8 hex chars, no dash — PID cannot be extracted, must not be deleted
old_ref="refs/notes/perf-v3-add-deadbeef"
git update-ref "$old_ref" "$current_oid"

git perf prune

old_count=$(git for-each-ref "$old_ref" | wc -l | tr -d '[:space:]')
assert_equals "$old_count" "1" "Expected prune to preserve old-format staging ref (no PID)"

# Clean up
git update-ref -d "$old_ref"

popd

test_stats
exit 0
