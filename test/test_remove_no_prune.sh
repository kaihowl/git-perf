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

# --- 28 days ago
export FAKETIME='-28d'

echo "Setup: Create old commit with measurement"
create_commit
old_commit=$(git rev-parse HEAD)
git perf add -m test-measure-old 10.0

# Must push to create the remote perf branch
git push
git perf push

# --- Now (present)
unset FAKETIME

echo "Setup: Create newer commit with measurement"
create_commit
git perf add -m test-measure-new 20.0
git push
git perf push

# Now make the old commit unreachable by resetting and force pushing
git reset --hard HEAD~1
git push --force origin master:master

# Make sure old commit is truly unreachable
git reflog expire --expire=all --all
git prune --expire=now

echo "Initial state: should have 2 notes (one orphaned for unreachable commit)"
initial_notes=$(count_perf_notes)
[[ $initial_notes -eq 2 ]] || { echo "Expected 2 notes, got $initial_notes"; exit 1; }

echo "Test 1: Remove with --no-prune (should keep orphaned note)"
git perf remove --older-than 20d --no-prune

after_no_prune=$(count_perf_notes)
# Should still have 2 notes even though one measurement was removed
# (the note for the unreachable commit is orphaned but not pruned)
[[ $after_no_prune -eq 2 ]] || { echo "Expected 2 notes after --no-prune, got $after_no_prune"; exit 1; }

echo "Test 2: Remove with default behavior (should prune orphaned note)"
git perf remove --older-than 20d

after_prune=$(count_perf_notes)
# Should now have only 1 note (orphaned one was pruned)
[[ $after_prune -eq 1 ]] || { echo "Expected 1 note after default remove, got $after_prune"; exit 1; }

echo "SUCCESS: --no-prune flag correctly skips pruning"
echo "SUCCESS: default behavior correctly prunes orphaned measurements"

popd

exit 0
