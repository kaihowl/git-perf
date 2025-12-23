#!/bin/bash

export TEST_TRACE=0

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

test_section "Setup repository with measurements"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" my-first-checkout
pushd my-first-checkout

test_section "Add measurement 28 days ago"

# --- 28 days ago
export FAKETIME='-28d'

create_commit
git perf add -m test-measure-one 10.0
assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# Exactly one measurement should be present (plus header row = 2 lines)
assert_equals "$num_measurements" "2" "Expected 1 measurement plus header"

# Only published measurements can be expired
git perf push

test_section "Test remove with 7 day threshold (should keep measurement)"

# Note: --older-than uses <= (inclusive), so measurements at exactly 7 days will be removed
# These tests will become flaky if run at exactly the 7 day boundary
git perf remove --older-than 7d
assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# Nothing should have been removed (1 measurement + header = 2 lines)
assert_equals "$num_measurements" "2" "Expected 1 measurement still present"

test_section "Add newer measurement 14 days ago"

# --- 14 days ago
export FAKETIME='-14d'

create_commit
git perf add -m test-measure-two 20.0
assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# Two measurements should be there (plus header = 3 lines)
assert_equals "$num_measurements" "3" "Expected 2 measurements plus header"

# Only published measurements can be expired
git perf push

# Pre-state
git reflog expire --expire=all --all
git prune --expire=now
prev_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
prev_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

git for-each-ref

test_section "Remove measurements older than 7 days"

git perf remove --older-than 7d
assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# One measurement should still be there (plus header = 2 lines)
assert_equals "$num_measurements" "2" "Expected 1 measurement after removal"
# The measurement should be 20.0
assert_contains "$report" "20.0" "Expected to find the 20.0 measurement"

if git_objects_contain test-measure-one; then
  echo "Unexpectedly still found test-measure-one in the git objects"
  echo "This should have been already pruned"
  git_object_show_roots test-measure-one
  git log refs/notes/perf-v3 --oneline
  exit 1
fi

cur_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
cur_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

if ! [[ $((cur_objects + cur_in_pack)) -lt $((prev_objects + prev_in_pack)) ]]; then
  echo "The number of objects now ($cur_objects + $cur_in_pack)
  is not less than previously ($prev_objects + $prev_in_pack)"
  echo "Drop compaction has not worked"
  exit 1
fi

test_section "Remove all remaining measurements (7 days ago)"

# -- 7 days ago
export FAKETIME='-7d'

# Pre-state
git reflog expire --expire=all --all
git prune --expire=now
prev_objects=$(git count-objects -v | awk '/count:/ { print $2 }')
prev_in_pack=$(git count-objects -v | awk '/in-pack:/ { print $2 }')

git perf remove --older-than 7d

assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# No measurement should be there
assert_equals "$num_measurements" "0" "Expected no measurements after final removal"

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

test_section "Add new measurement and verify cleanup"

create_commit
git perf add -m test-measure-three 30.0

git push
git perf push

assert_success_with_output report git perf report -o -
num_measurements=$(echo "$report" | wc -l)
# One measurement should be there (plus header = 2 lines)
assert_equals "$num_measurements" "2" "Expected 1 measurement after adding new one"

# Verify that temporary write branches are cleaned up after push
ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)
assert_equals "$ref_count" "1" "Expected only the permanent git perf ref after push"

popd

test_stats
exit 0
