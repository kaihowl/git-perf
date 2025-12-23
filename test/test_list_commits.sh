#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Check git perf list-commits functionality"

test_section "Test with empty repository (no measurements)"
cd_empty_repo
assert_success_with_output output git perf list-commits
assert_equals "$output" "" "Expected empty output for repository without measurements"
popd

test_section "Test with single measurement"
cd_empty_repo
create_commit
current_commit=$(git rev-parse HEAD)
git perf add -m test 1.0
assert_success_with_output output git perf list-commits
assert_contains "$output" "$current_commit" "Expected current commit in list-commits output"
popd

test_section "Test with multiple measurements"
cd_empty_repo
create_commit
first_commit=$(git rev-parse HEAD)
git perf add -m test 1.0

create_commit
second_commit=$(git rev-parse HEAD)
git perf add -m test 2.0

assert_success_with_output output git perf list-commits
assert_contains "$output" "$first_commit" "Expected first commit in list-commits output"
assert_contains "$output" "$second_commit" "Expected second commit in list-commits output"

# Verify output is one commit per line
line_count=$(echo "$output" | wc -l)
assert_equals "$line_count" "2" "Expected 2 lines of output"
popd

test_section "Test after removing measurements (requires remote for publish/remove)"
pushd "$(mktemp -d)"
mkdir bare_repo
pushd bare_repo
bare_repo=$(pwd)
git init --bare
popd

git clone "$bare_repo" work_repo
pushd work_repo

create_commit
current_commit=$(git rev-parse HEAD)
git perf add -m test 1.0
git perf push

# Verify measurement was added
assert_success_with_output output git perf list-commits
assert_contains "$output" "$current_commit" "Expected commit to have measurement before removal"

# Wait a moment to ensure timestamp difference
sleep 2

# Remove measurements older than 0 days (removes all measurements)
git perf remove --older-than 0d

# Verify measurement was removed
assert_success_with_output output git perf list-commits
assert_equals "$output" "" "Expected empty output after removing all measurements"
popd
popd

test_stats
exit 0
