#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Audit with separate-by: basic grouping by single key"

cd_empty_repo

# Create measurements for different OS values
# We'll add both os=linux and os=macos at each commit
# Group 1 (os=linux): mean: 10, std: 0
# Group 2 (os=macos): mean: 20, std: 0
create_commit
git perf add -m timer --key-value os=linux 10
git perf add -m timer --key-value os=macos 20
create_commit
git perf add -m timer --key-value os=linux 10
git perf add -m timer --key-value os=macos 20
create_commit
git perf add -m timer --key-value os=linux 10
git perf add -m timer --key-value os=macos 20

# Both groups should pass with their own baselines
assert_success_with_output output git perf audit -m timer -S os
assert_contains "$output" "Auditing measurement \"timer\" (os=linux):"
assert_contains "$output" "Auditing measurement \"timer\" (os=macos):"
assert_contains "$output" "Overall: PASSED"

# Test that groups are audited independently
# Add a failing measurement to one group (linux), but keep macos stable
create_commit
git perf add -m timer --key-value os=linux 50
git perf add -m timer --key-value os=macos 20

# Linux group should fail, but macos should still pass
assert_failure_with_output output git perf audit -m timer -S os
assert_contains "$output" "Auditing measurement \"timer\" (os=linux):"
assert_contains "$output" "Auditing measurement \"timer\" (os=macos):"
assert_contains "$output" "Overall: FAILED"

test_section "Audit with separate-by: multiple dimensions"

cd_empty_repo

# Create measurements for different os/arch combinations
# Add all three groups at each commit
create_commit
git perf add -m bench --key-value os=linux --key-value arch=x64 100
git perf add -m bench --key-value os=linux --key-value arch=arm64 200
git perf add -m bench --key-value os=macos --key-value arch=arm64 300
create_commit
git perf add -m bench --key-value os=linux --key-value arch=x64 100
git perf add -m bench --key-value os=linux --key-value arch=arm64 200
git perf add -m bench --key-value os=macos --key-value arch=arm64 300
create_commit
git perf add -m bench --key-value os=linux --key-value arch=x64 100
git perf add -m bench --key-value os=linux --key-value arch=arm64 200
git perf add -m bench --key-value os=macos --key-value arch=arm64 300

# All three groups should pass
assert_success_with_output output git perf audit -m bench -S os -S arch
assert_contains "$output" "Auditing measurement \"bench\" (os=linux/arch=arm64):"
assert_contains "$output" "Auditing measurement \"bench\" (os=linux/arch=x64):"
assert_contains "$output" "Auditing measurement \"bench\" (os=macos/arch=arm64):"
assert_contains "$output" "Overall: PASSED (3/3 groups passed)"

test_section "Audit with separate-by: combined with selectors"

cd_empty_repo

# Create measurements with multiple keys
# Add both prod and dev measurements at each commit
create_commit
git perf add -m test --key-value os=linux --key-value env=prod 10
git perf add -m test --key-value os=linux --key-value env=dev 20
create_commit
git perf add -m test --key-value os=linux --key-value env=prod 10
git perf add -m test --key-value os=linux --key-value env=dev 20
create_commit
git perf add -m test --key-value os=linux --key-value env=prod 10
git perf add -m test --key-value os=linux --key-value env=dev 20

# Use selectors to pre-filter to only prod, then separate by os
assert_success_with_output output git perf audit -m test --selectors env=prod -S os
assert_contains "$output" "Auditing measurement \"test\" (os=linux):"
# Should only show one group since we filtered to env=prod first
assert_not_contains "$output" "env=dev"

test_section "Audit with separate-by: error when key doesn't exist"

cd_empty_repo

# Create measurements without the separate-by key
create_commit
git perf add -m timer 10
create_commit
git perf add -m timer 20

# Should fail when trying to separate by non-existent key
assert_failure_with_output output git perf audit -m timer -S os
assert_contains "$output" "no measurements have all required keys"

test_section "Audit without separate-by: no summary line printed"

cd_empty_repo

# Create stable measurements without any key-value pairs
create_commit
git perf add -m timer 10
create_commit
git perf add -m timer 10
create_commit
git perf add -m timer 10

# Without -S flag, no "Overall:" summary should appear in the output
assert_success_with_output output git perf audit -m timer
assert_not_contains "$output" "Overall:"

test_stats
exit 0
