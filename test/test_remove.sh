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

function git_object_show_roots {
  PATTERN=$1

  while read -r OBJECT; do
      # Check if the object contains the specific pattern
      if git cat-file -p "$OBJECT" 2>/dev/null | grep -q "$PATTERN"; then
        git log --all --pretty=tformat:'%T %h %s' \
        | while read -r tree commit subject ; do
            if git ls-tree -r "$tree" | grep -q "$OBJECT" ; then
                echo "$commit" "$subject" still present
                git for-each-ref --contains "$commit"
            fi
        done
      fi
  done < <(git rev-list --objects --all | awk '{print $1}')

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

echo Add measurement on commit in the past
create_commit 
git perf add -m test-measure-one 10.0
num_measurements=$(git perf report -o - | wc -l)
# Exactly one measurement should be present
[[ ${num_measurements} -eq 1 ]] || exit 1

# Only published measurements can be expired
git perf push

# TODO(kaihowl) specify >= or > precisely
# These tests will become flaky if this is incorrectly set as we remove excactly on 7 day boundaries
# If the test runs quickly enough, the offset between commits and the invocation of removal will be exactly 7 days
echo "Remove measurements on commits older than 7 days"
git perf remove --older-than 7d || bash -i
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

# Only published measurements can be expired
git perf push

# Pre-state
git reflog expire --expire=all --all
git prune --expire=now
prev_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
prev_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

git for-each-ref
echo "Remove older than 7 days measurements"
git perf remove --older-than 7d
num_measurements=$(git perf report -o - | wc -l)
# One measurement should still be there
[[ ${num_measurements} -eq 1 ]] || exit 1
# The measurement should be 20.0
git perf report -o - | grep '20\.0'

if git_objects_contain test-measure-one; then
  echo "Unexpectedly still found test-measure-one in the git objects"
  echo "This should have been already pruned"
  git_object_show_roots test-measure-one
  git log refs/notes/perf-v3 --oneline
  exit 1
fi

if ! [[ $((cur_objects + cur_in_pack)) -lt $((prev_objects + prev_in_pack)) ]]; then
  echo "The number of objects now ($cur_objects + $cur_in_pack)
  is not less than previously ($prev_objects + $prev_in_pack)"
  echo "Drop compaction has not worked"
  exit 1
fi

# -- 7 days ago
export FAKETIME='-7d'

# Pre-state
git reflog expire --expire=all --all
git prune --expire=now
prev_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
prev_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

echo "Remove older than 7 days measurements"
git perf remove --older-than 7d

num_measurements=$(git perf report -o - | wc -l)
# No measurement should be there
[[ ${num_measurements} -eq 0 ]] || exit 1

git reflog expire --expire=all --all
git prune --expire=now
cur_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
cur_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

if git_objects_contain test-measure-one; then
  echo "Unexpectedly still found test-measure-one in the git objects"
  echo "This should have been already pruned"
  git_object_show_roots test-measure-one
  git log refs/notes/perf-v3 --oneline
  exit 1
fi

if git_objects_contain test-measure-two; then
  echo "Unexpectedly still found test-measure-two in the git objects"
  echo "This should have been already pruned"
  git_object_show_roots test-measure-two
  exit 1
fi

if ! [[ $((cur_objects + cur_in_pack)) -lt $((prev_objects + prev_in_pack)) ]]; then
  echo "The number of objects now ($cur_objects + $cur_in_pack)
  is not less than previously ($prev_objects + $prev_in_pack)"
  echo "Drop compaction has not worked"
  exit 1
fi

echo "Add a commit with a newer measurement"
create_commit
git perf add -m test-measure-three 30.0

git push
git perf push

num_measurements=$(git perf report -o - | wc -l)
# One measurement should be there
# TODO(kaihowl) clean up of write branches needed
[[ ${num_measurements} -eq 1 ]] || exit 1

popd

exit 0
