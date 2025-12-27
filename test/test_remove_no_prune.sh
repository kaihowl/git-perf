#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check how many notes exist in the perf branch
function count_perf_notes {
  git notes --ref=refs/notes/perf-v3 list 2>/dev/null | wc -l
}

if [[ $(uname -s) = Darwin ]]; then
  export DYLD_FORCE_FLAT_NAMESPACE=1
  export DYLD_INSERT_LIBRARIES=/opt/homebrew/lib/faketime/libfaketime.1.dylib
else
  export LD_PRELOAD=/usr/lib/x86_64-linux-gnu/faketime/libfaketime.so.1
fi

test_section "Setup repository with measurements"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" my-first-checkout
pushd my-first-checkout

test_section "Create two commits with measurements"

create_commit
git perf add -m test-measure-1 10.0
git push
git perf push

create_commit
git perf add -m test-measure-2 20.0
git push
git perf push

initial_notes=$(count_perf_notes)
assert_equals "$initial_notes" "2" "Initial state should have 2 notes"

test_section "Make first commit unreachable"

# Now make the first commit unreachable by resetting
git reset --hard HEAD~1
git push --force origin master:master

# Make sure first commit is truly unreachable
git reflog expire --expire=all --all
git prune --expire=now

after_unreachable=$(count_perf_notes)
assert_equals "$after_unreachable" "2" "Should still have 2 notes (one orphaned) after making commit unreachable"

test_section "Test 1: Call prune directly (should remove orphaned note)"

git perf prune

after_direct_prune=$(count_perf_notes)
assert_equals "$after_direct_prune" "1" "Expected 1 note after direct prune"

test_section "Setup for --no-prune test: Create another commit and make it orphaned"

create_commit
git perf add -m test-measure-3 30.0
git push
git perf push

create_commit
git perf add -m test-measure-4 40.0
git push
git perf push

before_test=$(count_perf_notes)
assert_equals "$before_test" "3" "Should have 3 notes now"

# Make one commit unreachable again
git reset --hard HEAD~1
git push --force origin master:master
git reflog expire --expire=all --all
git prune --expire=now

test_section "Test 2: Remove with --no-prune (should NOT prune orphaned note)"

export FAKETIME='-30d'
git perf remove --older-than 25d --no-prune
unset FAKETIME

after_no_prune=$(count_perf_notes)
# Should still have 3 notes (measurements removed, but orphaned note NOT pruned)
assert_equals "$after_no_prune" "3" "Expected 3 notes after --no-prune"

test_section "Test 3: Remove with default behavior (should prune orphaned note)"

export FAKETIME='-30d'
git perf remove --older-than 25d
unset FAKETIME

after_default_remove=$(count_perf_notes)
# Should now have 2 notes (orphaned note was pruned)
assert_equals "$after_default_remove" "2" "Expected 2 notes after default remove"

popd

test_stats
exit 0
