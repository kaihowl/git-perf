#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Test that audit fails with no -m
cd_temp_repo
if git perf audit 2>&1 | grep -q 'required'; then
  echo 'PASS: audit fails with no -m as required.'
else
  echo 'FAIL: audit did not fail as required when no -m is given.'
  exit 1
fi

# Reset for the rest of the tests
cd_temp_repo

# Create measurements for two different metrics
git checkout HEAD~3
git perf add -m timer 1
git perf add -m memory 100

git checkout master && git checkout HEAD~2
git perf add -m timer 2
git perf add -m memory 110

git checkout master && git checkout HEAD~1
git perf add -m timer 3
git perf add -m memory 120

# head commit
git checkout master
git perf add -m timer 4
git perf add -m memory 130

# Test auditing multiple metrics at once
git perf audit -m timer -m memory -d 4
git perf audit -m timer -m memory -d 3
git perf audit -m timer -m memory -d 2

# Test that it fails when one metric is outside the acceptable range
git checkout master
create_commit
git perf add -m timer 10
git perf add -m memory 130
if output=$(git perf audit -m timer -m memory -d 2 2>&1); then
  echo "FAIL: audit did not fail when one metric is outside the acceptable range"
  echo "$output"
  exit 1
fi
echo "$output" | grep -q "❌ 'timer'" || exit 1
echo "$output" | grep -q "One or more measurements failed audit" || exit 1
echo "PASS: audit failed when one metric is outside the acceptable range"

# Test that multiple failing metrics are all reported
git reset --hard HEAD~1
create_commit
git perf add -m timer 15  # This will also be outside the acceptable range
git perf add -m memory 200  # This will also be outside the acceptable range
if output=$(git perf audit -m timer -m memory -d 2 2>&1); then
  echo "FAIL: audit did not fail when multiple metrics are outside the acceptable range"
  echo "$output"
  exit 1
fi
echo "$output" | grep -q "❌ 'timer'" || exit 1
echo "$output" | grep -q "❌ 'memory'" || exit 1
echo "$output" | grep -q "One or more measurements failed audit" || exit 1
echo "PASS: audit failed when multiple metrics are outside the acceptable range"

# Test with only one metric (backward compatibility)
cd_temp_repo
git perf add -m timer 4
git perf audit -m timer -d 4

# Test with three metrics
git checkout master
git perf add -m timer 4
git perf add -m memory 130
git perf add -m cpu 50

git checkout HEAD~1
git perf add -m timer 3
git perf add -m memory 120
git perf add -m cpu 45

git checkout master
git perf audit -m timer -m memory -m cpu -d 4

echo Multiple metrics audit tests completed successfully
exit 0 