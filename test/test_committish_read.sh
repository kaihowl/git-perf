#!/bin/bash

# Test committish support for read operations (report, audit)
# Tests that reports and audits can start from specific commits using positional argument

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test 1: Report from specific commit by SHA"
cd_temp_repo

# Add measurements to multiple commits
git perf add -m test_metric 100.0
commit1=$(git rev-parse HEAD)

create_commit
git perf add -m test_metric 110.0
commit2=$(git rev-parse HEAD)

create_commit
git perf add -m test_metric 120.0
commit3=$(git rev-parse HEAD)

# Generate report from commit2 (should include commit2 and commit1, not commit3)
output=$(git perf report "$commit2" -o -)

# Verify commit2 data is included
assert_output_contains "$output" "110.0" "Report should include commit2 data"

# Verify commit1 data is included (ancestor)
assert_output_contains "$output" "100.0" "Report should include commit1 data"

# Verify commit3 data is NOT included (not an ancestor of commit2)
assert_output_not_contains "$output" "120.0" "Report should NOT include commit3 data"

echo "Test 2: Report from HEAD~N format"
cd_temp_repo

# Add measurements to commits
git perf add -m metric2 200.0
create_commit
git perf add -m metric2 210.0
create_commit
git perf add -m metric2 220.0

# Generate report from HEAD~1
output=$(git perf report HEAD~1 -o -)

# Verify HEAD~1 data is included
assert_output_contains "$output" "210.0" "Report should include HEAD~1 data"

# Verify HEAD~2 data is included
assert_output_contains "$output" "200.0" "Report should include HEAD~2 data"

# Verify HEAD data is NOT included
assert_output_not_contains "$output" "220.0" "Report should NOT include HEAD data when starting from HEAD~1"

echo "Test 3: Report from branch"
cd_empty_repo
create_commit
git perf add -m branch_metric 100.0

# Create feature branch
git checkout -b feature-branch
create_commit
git perf add -m branch_metric 200.0

# Go back to master and add more commits
git checkout master
create_commit
git perf add -m branch_metric 300.0

# Report from feature branch
output=$(git perf report feature-branch -o -)

# Should include feature branch data
assert_output_contains "$output" "200.0" "Report should include feature branch data"

# Should include common ancestor data
assert_output_contains "$output" "100.0" "Report should include common ancestor data"

# Should NOT include master-only data
assert_output_not_contains "$output" "300.0" "Report should NOT include master-only data"

echo "Test 4: Report from tag"
cd_temp_repo

git perf add -m tag_metric 100.0
create_commit
git perf add -m tag_metric 200.0
git tag v1.0

create_commit
git perf add -m tag_metric 300.0

# Report from tag
output=$(git perf report v1.0 -o -)

# Should include tagged commit and ancestors
assert_output_contains "$output" "200.0" "Report should include tagged commit data"
assert_output_contains "$output" "100.0" "Report should include ancestor data"

# Should NOT include commits after tag
assert_output_not_contains "$output" "300.0" "Report should NOT include commits after tag"

echo "Test 5: Audit from specific commit"
cd_temp_repo

# Create baseline measurements
git perf add -m audit_metric 100.0
create_commit
git perf add -m audit_metric 102.0
create_commit
git perf add -m audit_metric 101.0
commit_to_audit=$(git rev-parse HEAD)

# Audit the specific commit
output=$(git perf audit "$commit_to_audit" -m audit_metric -n 3)

# Verify the audit ran
assert_output_contains "$output" "audit_metric" "Audit output should contain metric name"

echo "Test 6: Audit from HEAD~N"
cd_temp_repo

# Create measurement history
git perf add -m metric6 100.0
create_commit
git perf add -m metric6 105.0
create_commit
git perf add -m metric6 103.0

# Audit HEAD~0 (HEAD) with history
output=$(git perf audit HEAD -m metric6 -n 3)

# Verify audit ran successfully
assert_output_contains "$output" "metric6" "Audit should include metric name"

echo "Test 7: Report with limited history from specific commit"
cd_temp_repo

# Create multiple commits with measurements
for i in {1..6}; do
    git perf add -m limited_metric "$((100 + i)).0"
    if [[ $i -lt 6 ]]; then
        create_commit
    fi
done

target=$(git rev-parse HEAD~2)

# Report with -n 2 from HEAD~2 (should only show 2 commits)
output=$(git perf report "$target" -o - -n 2)

# Count data rows (excluding CSV header)
row_count=$(echo "$output" | tail -n +2 | wc -l)

# Should have exactly 2 rows
if [[ $row_count -ne 2 ]]; then
    echo "FAIL: Expected 2 data rows, got $row_count"
    echo "Output: $output"
    exit 1
fi

echo "Test 8: Default behavior without committish (should use HEAD)"
cd_temp_repo

# Add measurements
git perf add -m default_report 100.0
create_commit
git perf add -m default_report 200.0

# Report without committish should use HEAD
output=$(git perf report -o -)

# Should include all data up to HEAD
assert_output_contains "$output" "100.0" "Default report should include all data"
assert_output_contains "$output" "200.0" "Default report should include HEAD data"

echo "Test 9: Report from specific commit with measurement filter"
cd_temp_repo

# Add different measurements
git perf add -m metric_a 100.0
git perf add -m metric_b 200.0
create_commit

git perf add -m metric_a 110.0
git perf add -m metric_b 210.0
target=$(git rev-parse HEAD)

# Report from specific commit, filtering for metric_a
output=$(git perf report "$target" -o - -m metric_a)

# Should include metric_a
assert_output_contains "$output" "metric_a" "Report should include metric_a"

echo "Test 10: Audit specific commit with sufficient history"
cd_temp_repo

# Build up history of measurements
for i in {1..5}; do
    git perf add -m stable_metric "$((100 + i)).0"
    create_commit
done

# Add one more measurement
git perf add -m stable_metric "106.0"
target=$(git rev-parse HEAD)

# Audit this commit
output=$(git perf audit "$target" -m stable_metric -n 5)

# Should successfully audit
assert_output_contains "$output" "stable_metric" "Audit should run successfully"

echo "All committish read tests passed!"
exit 0
