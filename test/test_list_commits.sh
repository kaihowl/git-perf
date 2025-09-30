#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Check git perf list-commits functionality

# Test with empty repository (no measurements)
cd_empty_repo
output=$(git perf list-commits)
if [ -n "$output" ]; then
  echo "Expected empty output for repository without measurements"
  exit 1
fi
popd

# Test with single measurement
cd_empty_repo
create_commit
git perf measure -m test -v 1.0
git perf publish
current_commit=$(git rev-parse HEAD)
output=$(git perf list-commits)
assert_output_contains "$output" "$current_commit" "Expected current commit in list-commits output"
popd

# Test with multiple measurements
cd_empty_repo
create_commit
git perf measure -m test -v 1.0
git perf publish
first_commit=$(git rev-parse HEAD)

create_commit
git perf measure -m test -v 2.0
git perf publish
second_commit=$(git rev-parse HEAD)

output=$(git perf list-commits)
assert_output_contains "$output" "$first_commit" "Expected first commit in list-commits output"
assert_output_contains "$output" "$second_commit" "Expected second commit in list-commits output"

# Verify output is one commit per line
line_count=$(echo "$output" | wc -l)
if [ "$line_count" -ne 2 ]; then
  echo "Expected 2 lines of output, got $line_count"
  exit 1
fi
popd

# Test that unpublished measurements are not listed
cd_empty_repo
create_commit
git perf measure -m test -v 1.0
# Don't publish
current_commit=$(git rev-parse HEAD)
output=$(git perf list-commits)
if [ -n "$output" ]; then
  echo "Expected empty output for unpublished measurements"
  exit 1
fi
popd

# Test after removing measurements
cd_empty_repo
create_commit
git perf measure -m test -v 1.0
git perf publish
current_commit=$(git rev-parse HEAD)

# Wait a moment to ensure timestamp difference
sleep 2

# Remove measurements older than now
git perf remove --older-than now
git perf publish

output=$(git perf list-commits)
if [ -n "$output" ]; then
  echo "Expected empty output after removing all measurements"
  exit 1
fi
popd

echo "All tests passed!"
