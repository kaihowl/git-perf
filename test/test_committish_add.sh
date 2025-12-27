#!/bin/bash

# Test committish support for write operations (add, measure)
# Tests that measurements can be added to specific commits using --commit flag

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test 1: Add to specific commit by SHA"
cd_temp_repo
commit1=$(git rev-parse HEAD~2)
commit2=$(git rev-parse HEAD~1)
head=$(git rev-parse HEAD)

# Add measurement to HEAD~2 using --commit flag
git perf add --commit "$commit1" -m test_metric 100.5

# Verify measurement appears in report from commit1
output=$(git perf report "$commit1" -o -)
assert_output_contains "$output" "test_metric" "Measurement should be in report from commit1"
assert_output_contains "$output" "100.5" "Value should be in report from commit1"

# Verify commit1 hash is in the output
assert_output_contains "$output" "$commit1" "Commit1 hash should be in report"

echo "Test 2: Add to HEAD~N format"
cd_temp_repo
target=$(git rev-parse HEAD~2)

# Add using HEAD~N format
git perf add --commit HEAD~2 -m test_metric2 200.5

# Verify measurement is accessible from that commit
output=$(git perf report HEAD~2 -o -)
assert_output_contains "$output" "test_metric2" "Measurement should be in report from HEAD~2"
assert_output_contains "$output" "200.5" "Value should be in report from HEAD~2"

echo "Test 3: Add to branch name"
cd_empty_repo
create_commit

# Create feature branch and add a commit to it
git checkout -b feature-branch
create_commit
feature_commit=$(git rev-parse HEAD)

# Switch back to master and create another commit
git checkout master
create_commit

# Add measurement to feature branch (not current HEAD)
git perf add --commit feature-branch -m test_metric3 300.5

# Verify measurement appears in report from feature branch
output=$(git perf report feature-branch -o -)
assert_output_contains "$output" "test_metric3" "Measurement should be in report from feature-branch"
assert_output_contains "$output" "300.5" "Value should be in report from feature-branch"

echo "Test 4: Add to tag"
cd_temp_repo
tagged_commit=$(git rev-parse HEAD~1)
git tag v1.0 "$tagged_commit"

# Add measurement using tag name
git perf add --commit v1.0 -m test_metric4 400.5

# Verify measurement appears in report from tag
output=$(git perf report v1.0 -o -)
assert_output_contains "$output" "test_metric4" "Measurement should be in report from tag"
assert_output_contains "$output" "400.5" "Value should be in report from tag"

echo "Test 5: Default behavior without --commit flag"
cd_empty_repo
create_commit
head=$(git rev-parse HEAD)

# Add without --commit flag (should default to HEAD)
git perf add -m default_test 42.0

# Verify measurement appears in report from HEAD
output=$(git perf report -o -)
assert_output_contains "$output" "default_test" "Measurement should be in report from HEAD"
assert_output_contains "$output" "42.0" "Value should be in report from HEAD"

echo "Test 6: Measure to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Measure to HEAD~1 using --commit flag
git perf measure --commit HEAD~1 -m measure_test -- echo "test"

# Verify measurement appears in report from HEAD~1
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "measure_test" "Measurement should be in report from HEAD~1"

echo "Test 7: Multiple measurements to same specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Add multiple measurements to the same commit
git perf add --commit HEAD~1 -m metric1 100.0
git perf add --commit HEAD~1 -m metric2 200.0

# Verify both measurements appear in report
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "metric1" "First measurement should be in report"
assert_output_contains "$output" "100.0" "First value should be in report"
assert_output_contains "$output" "metric2" "Second measurement should be in report"
assert_output_contains "$output" "200.0" "Second value should be in report"

echo "Test 8: Add with selectors to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Add measurement with key-value selector to specific commit
git perf add --commit HEAD~1 -m test_metric 100.0 -k os=linux

# Verify measurement with selector appears in report
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "test_metric" "Measurement should be in report"
# Note: CSV output may not show selectors in the same way, so just verify the measurement exists

echo "Test 9: Add multiple measurements with different selectors to same commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Add measurements with different selectors
git perf add --commit HEAD~1 -m metric1 100.0 -k os=linux
git perf add --commit HEAD~1 -m metric1 150.0 -k os=mac

# Verify both measurements appear (they have the same name but different selectors)
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "metric1" "Measurement should be in report"
assert_output_contains "$output" "100.0" "Linux value should be in report"
assert_output_contains "$output" "150.0" "Mac value should be in report"

echo "Test 10: Measure with repetitions to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Measure multiple times to specific commit
git perf measure --commit HEAD~1 -m repeat_test -n 3 -- true

# Verify measurements appear in report
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "repeat_test" "Measurement should be in report"

# Count occurrences of repeat_test in output (should be 3 data rows)
count=$(echo "$output" | grep -c "repeat_test" || true)
if [[ $count -ne 3 ]]; then
    echo "FAIL: Expected 3 measurements, found $count"
    echo "Output: $output"
    exit 1
fi

echo "All committish add tests passed!"
exit 0
