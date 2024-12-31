#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

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

echo Add measurement on commit in the past
create_commit 
git perf add -m test-measure-one 10.0
num_measurements=$(git perf report -o - | wc -l)
# Exactly one measurement should be present
[[ ${num_measurements} -eq 1 ]] || exit 1


# TODO(kaihowl) specify >= or > precisely
echo "Remove measurements on commits older than 7 days"
git perf remove --older-than 10d
num_measurements=$(git perf report -o - | wc -l)
# Nothing should have been removed
[[ ${num_measurements} -eq 1 ]] || exit 1

# --- 14 days ago
export FAKETIME='-14d'

echo "Add a commit with a newer measurement"
create_commit
git perf add -m test-measure-two 20.0
num_measurements=$(git perf report -o - | wc -l)
# Two measurements should be there
[[ ${num_measurements} -eq 2 ]] || exit 1

echo "Remove older than 7 days measurements"
git perf remove --older-than 7d
num_measurements=$(git perf report -o - | wc -l)
# One measurement should still be there
[[ ${num_measurements} -eq 1 ]] || exit 1
# The measurement should be 20.0
git perf report -o - | grep '20\.0'

# -- NOW
export FAKETIME=

echo "Remove older than 7 days measurements"
git perf remove --older-than 7d

num_measurements=$(git perf report -o - | wc -l)
# No measurement should be there
[[ ${num_measurements} -eq 0 ]] || exit 1

echo "Add a commit with a newer measurement"
create_commit
git perf add -m test-measure-three 30.0

git push
git perf push

num_measurements=$(git perf report -o - | wc -l)
# One measurement should be there
[[ ${num_measurements} -eq 1 ]] || exit 1

popd

# TODO check that we reach the state of 7 days prior

# Checkout repo on second checkout with earlier notes state
git clone "$orig" my-second-checkout
pushd my-second-checkout
zsh -i 
git perf pull
popd

pushd my-first-checkout
echo "Manual implementation of drop compaction"
prev_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
prev_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

# TODO(kaihowl) move all into rust impl
REFS_NOTES_BRANCH=refs/notes/perf-v3
# Checkout git perf branch (TODO(kaihowl) handle this without the checkout)
# Go back 7 days on branch (TODO(kaihowl) check that this works without reflog)
prev_head=$(git rev-parse "$REFS_NOTES_BRANCH")
cutoff_head=$(git rev-parse "$REFS_NOTES_BRANCH@{7 days ago}")
# Check that the commits is a different one than before
if [[ $prev_head = "$cutoff_head" ]]; then
  echo "cutoff head after checkout did not change"
  exit 1
fi
# TODO(kaihowl) debug command
git reflog "$REFS_NOTES_BRANCH"

# Make orphan checkout / new temp branch
new_history=$(git commit-tree -m 'cutoff history' "$(git rev-parse "$cutoff_head^{tree}")")
# Rebase remaining history on top of new parent
# TODO(kaihowl) check that this works with merges in between and still has the correct state
for commit in $(git rev-list --reverse "${cutoff_head}..${prev_head}"); do
  # TODO(kaihowl) fix the message?
  new_history=$(git commit-tree -m 'reapply' -p "$new_history" "$commit^{tree}")
done
# Install new history as notes branch
git update-ref "$REFS_NOTES_BRANCH" "$new_history"
# Delete temp branch
# Prune
# TODO maybe expire the reflog first / more specifically?
git reflog expire --expire=all "$REFS_NOTES_BRANCH"
git prune --expire=now

cur_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
cur_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

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

if git_objects_contain test-measure-one; then
  echo "Unexpectedly still found test-measure-one in the git objects"
  echo "This should have been already pruned"
  exit 1
fi

if ! git_objects_contain test-measure-two; then
  echo "Expected to find no longer visible but not yet garbage collected test-measure-two in the repo"
  exit 1
fi

if ! [[ $((cur_objects + cur_in_pack)) -lt $((prev_objects + prev_in_pack)) ]]; then
  echo "The number of objects now ($cur_objects + $cur_in_pack)
  is not less than previously ($prev_objects + $prev_in_pack)"
  echo "Drop compaction has not worked"
  exit 1
fi

# TODO(kaihowl) remove once no checkout is needed above anymore
num_measurements=$(git perf report -o - | wc -l)
# One measurement should be there
[[ ${num_measurements} -eq 1 ]] || exit 1

git perf push

popd

echo "Add no longer shared history measurement from second checkout"
pushd my-second-checkout

git perf add -m second-measurement 103.0
# TODO(kaihowl) what happens here?
git perf push

# TODO(kaihowl) humpty dumpty implementation:
# Merge unrelated histories after all
# What happens if a checkout did not consume a compaction commit, adds new measurements and replays the commits to the remote?
# Could we establish "a set of unpublished measurements" with an upstream ref?
# These would always be replayed on whatever is upstream, even with unrelated histories.
# I.e., create an empty tree, replay the non-published changes, merge the result with the (maybe unrelated) upstream history?

git perf report -o -

git log "$REFS_NOTES_BRANCH"

exit 0
