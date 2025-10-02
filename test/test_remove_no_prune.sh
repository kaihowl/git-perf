#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check that no git objects in the entire repo contains the given pattern
function git_objects_contain {
  PATTERN=$1
  # Iterate over all git objects in the repository
  while read -r OBJECT; do
      # Check if the object contains the specific pattern
      if git cat-file -p "$OBJECT" 2>/dev/null | grep -q "$PATTERN"; then
          echo "The pattern '$PATTERN' was found in the git object '$OBJECT'."
          return 0
      fi
  done < <(git rev-list --objects --all | awk '{print $1}')

  echo "Success: No git objects contain the pattern '$PATTERN'."
  return 1
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

echo "Add measurement on commit in the past"
create_commit
git perf add -m test-measure-old 10.0
num_measurements=$(git perf report -o - | wc -l)
# Exactly one measurement should be present
[[ ${num_measurements} -eq 1 ]] || exit 1

# Only published measurements can be expired
git perf push

# --- 14 days ago
export FAKETIME='-14d'

echo "Add a commit with a newer measurement"
create_commit
git perf add -m test-measure-new 20.0
num_measurements=$(git perf report -o - | wc -l)
# Two measurements should be there
[[ ${num_measurements} -eq 2 ]] || exit 1

# Only published measurements can be expired
git perf push

# Now remove the old commit from git history (simulating a rebase/force push)
git reset --hard HEAD~1
git push --force

# Run git prune to make the old commit unreachable
git reflog expire --expire=all --all
git prune --expire=now

echo "Test 1: Remove with --no-prune flag (should NOT prune orphaned measurements)"
git perf remove --older-than 20d --no-prune

num_measurements=$(git perf report -o - | wc -l)
# One measurement should still be there (test-measure-new)
[[ ${num_measurements} -eq 1 ]] || exit 1

# The orphaned measurement (test-measure-old) should STILL be in git objects
# because we used --no-prune
if ! git_objects_contain test-measure-old; then
  echo "ERROR: test-measure-old was pruned even with --no-prune flag!"
  exit 1
fi

echo "Test 2: Now remove with default behavior (should prune orphaned measurements)"
# First, let's remove measurements again, this time without --no-prune
git perf remove --older-than 20d

num_measurements=$(git perf report -o - | wc -l)
# Still one measurement (test-measure-new)
[[ ${num_measurements} -eq 1 ]] || exit 1

# Now the orphaned measurement (test-measure-old) should be GONE
# because default behavior includes pruning
git reflog expire --expire=all --all
git prune --expire=now

if git_objects_contain test-measure-old; then
  echo "ERROR: test-measure-old was NOT pruned with default behavior!"
  echo "This means automatic pruning is not working"
  exit 1
fi

echo "SUCCESS: --no-prune flag works correctly"
echo "SUCCESS: default pruning behavior works correctly"

popd

exit 0
