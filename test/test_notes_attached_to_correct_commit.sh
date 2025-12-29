#!/bin/bash

# Test to verify that git notes are attached to the correct commit
# This ensures measurements are stored on the intended commit, not HEAD or other commits
# This test focuses on direct verification of git notes attachment

# Disable verbose tracing - our assertions provide better output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

test_section "Verify notes are attached to the correct commit"

cd_empty_repo

# Create a series of commits
create_commit
first_commit=$(git rev-parse HEAD)

create_commit
second_commit=$(git rev-parse HEAD)

create_commit
third_commit=$(git rev-parse HEAD)

# Add measurements to specific commits
git perf add -m test_metric 100 --commit "$first_commit"
git perf add -m test_metric 200 --commit "$second_commit"
git perf add -m test_metric 300 --commit "$third_commit"

test_section "Verify measurements are retrievable from correct commits"

# Verify measurements can be reported from each commit (-n 1 limits to just that commit)
first_report=$(git perf report "$first_commit" -o - -n 1)
assert_contains "$first_report" "test_metric" "Report for first commit should show test_metric"
assert_matches "$first_report" $'[[:space:]]100\\.0[[:space:]]' "Report for first commit should show value 100"

second_report=$(git perf report "$second_commit" -o - -n 1)
assert_contains "$second_report" "test_metric" "Report for second commit should show test_metric"
assert_matches "$second_report" $'[[:space:]]200\\.0[[:space:]]' "Report for second commit should show value 200"

third_report=$(git perf report "$third_commit" -o - -n 1)
assert_contains "$third_report" "test_metric" "Report for third commit should show test_metric"
assert_matches "$third_report" $'[[:space:]]300\\.0[[:space:]]' "Report for third commit should show value 300"

test_section "Verify cross-contamination does not occur"

# Ensure first commit doesn't have second or third commit's values
assert_not_contains "$first_report" "200.0" "First commit should not have second commit's value"
assert_not_contains "$first_report" "300.0" "First commit should not have third commit's value"

# Ensure second commit doesn't have first or third commit's values
assert_not_contains "$second_report" "100.0" "Second commit should not have first commit's value"
assert_not_contains "$second_report" "300.0" "Second commit should not have third commit's value"

# Ensure third commit doesn't have first or second commit's values
assert_not_contains "$third_report" "100.0" "Third commit should not have first commit's value"
assert_not_contains "$third_report" "200.0" "Third commit should not have second commit's value"

test_section "Verify adding to HEAD vs specific commit"

# Create another commit and make it HEAD
create_commit
head_commit=$(git rev-parse HEAD)

# Add measurement to HEAD using default (no --commit flag)
git perf add -m default_metric 999

# Verify the measurement is on HEAD
head_report=$(git perf report "$head_commit" -o - -n 1)
assert_contains "$head_report" "default_metric" "HEAD commit should have default_metric"
assert_matches "$head_report" $'[[:space:]]999\\.0[[:space:]]' "HEAD commit should have value 999"

# Verify previous commits don't have this new measurement
first_report_check=$(git perf report "$first_commit" -o - -n 1)
assert_not_contains "$first_report_check" "default_metric" "First commit should not have default_metric"
assert_not_contains "$first_report_check" "999.0" "First commit should not have value 999"

test_section "Verify multiple measurements on same commit"

# Add multiple different metrics to first commit
git perf add -m metric_a 1111 --commit "$first_commit"
git perf add -m metric_b 2222 --commit "$first_commit"

# Verify all measurements are on the same commit via report command
first_report_multi=$(git perf report "$first_commit" -o - -n 1)
assert_contains "$first_report_multi" "test_metric" "First commit should still have test_metric"
assert_matches "$first_report_multi" $'[[:space:]]100\\.0[[:space:]]' "First commit should still have value 100"
assert_contains "$first_report_multi" "metric_a" "First commit should have metric_a"
assert_matches "$first_report_multi" $'[[:space:]]1111\\.0[[:space:]]' "First commit should have value 1111"
assert_contains "$first_report_multi" "metric_b" "First commit should have metric_b"
assert_matches "$first_report_multi" $'[[:space:]]2222\\.0[[:space:]]' "First commit should have value 2222"

# Verify these new metrics didn't leak to other commits
second_report_check=$(git perf report "$second_commit" -o - -n 1)
assert_not_contains "$second_report_check" "metric_a" "Second commit should not have metric_a"
assert_not_contains "$second_report_check" "1111.0" "Second commit should not have value 1111"
assert_not_contains "$second_report_check" "metric_b" "Second commit should not have metric_b"
assert_not_contains "$second_report_check" "2222.0" "Second commit should not have value 2222"

test_section "Verify commits with measurements are tracked"

# Verify list-commits shows all commits with measurements
commits_with_measurements=$(git perf list-commits)
assert_contains "$commits_with_measurements" "$first_commit" "First commit should be in list-commits"
assert_contains "$commits_with_measurements" "$second_commit" "Second commit should be in list-commits"
assert_contains "$commits_with_measurements" "$third_commit" "Third commit should be in list-commits"
assert_contains "$commits_with_measurements" "$head_commit" "HEAD commit should be in list-commits"

# Count commits with measurements (should be exactly 4)
commit_count=$(echo "$commits_with_measurements" | wc -l)
assert_equals "$commit_count" "4" "Should have exactly 4 commits with measurements"

test_stats
exit 0
