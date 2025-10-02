#!/bin/bash

set -e
set -x

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

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" my-first-checkout
pushd my-first-checkout

echo "Setup: Create two commits with measurements"
create_commit
git perf add -m test-measure-1 10.0
git push
git perf push

create_commit
git perf add -m test-measure-2 20.0
git push
git perf push

echo "Initial state: should have 2 notes"
initial_notes=$(count_perf_notes)
[[ $initial_notes -eq 2 ]] || { echo "Expected 2 notes, got $initial_notes"; exit 1; }

# Now make the first commit unreachable by resetting
git reset --hard HEAD~1
git push --force origin master:master

# Make sure first commit is truly unreachable
git reflog expire --expire=all --all
git prune --expire=now

echo "After making commit unreachable: should still have 2 notes (one orphaned)"
after_unreachable=$(count_perf_notes)
[[ $after_unreachable -eq 2 ]] || { echo "Expected 2 notes after making commit unreachable, got $after_unreachable"; exit 1; }

echo "Test 1: Call prune directly (should remove orphaned note)"
git perf prune

after_direct_prune=$(count_perf_notes)
[[ $after_direct_prune -eq 1 ]] || { echo "Expected 1 note after direct prune, got $after_direct_prune"; exit 1; }

# Now test the --no-prune flag with a fresh scenario
echo "Setup for --no-prune test: Create another commit and make it orphaned"
create_commit
git perf add -m test-measure-3 30.0
git push
git perf push

create_commit
git perf add -m test-measure-4 40.0
git push
git perf push

echo "Should have 3 notes now"
before_test=$(count_perf_notes)
[[ $before_test -eq 3 ]] || { echo "Expected 3 notes, got $before_test"; exit 1; }

# Make one commit unreachable again
git reset --hard HEAD~1
git push --force origin master:master
git reflog expire --expire=all --all
git prune --expire=now

echo "Test 2: Remove with --no-prune (should NOT prune orphaned note)"
export FAKETIME='-30d'
git perf remove --older-than 25d --no-prune
unset FAKETIME

after_no_prune=$(count_perf_notes)
# Should still have 3 notes (measurements removed, but orphaned note NOT pruned)
[[ $after_no_prune -eq 3 ]] || { echo "Expected 3 notes after --no-prune, got $after_no_prune"; exit 1; }

echo "Test 3: Remove with default behavior (should prune orphaned note)"
export FAKETIME='-30d'
git perf remove --older-than 25d
unset FAKETIME

after_default_remove=$(count_perf_notes)
# Should now have 2 notes (orphaned note was pruned)
[[ $after_default_remove -eq 2 ]] || { echo "Expected 2 notes after default remove, got $after_default_remove"; exit 1; }

echo "SUCCESS: --no-prune flag correctly skips pruning"
echo "SUCCESS: default behavior correctly prunes orphaned measurements"
echo "SUCCESS: direct prune command works correctly"

popd

exit 0
