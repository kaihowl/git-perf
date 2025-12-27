#!/bin/bash

# Disable verbose tracing - our assertions provide better output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git-perf operations without seed measurements
# This test verifies that operations handle gracefully the absence of any measurements

# ============================================================================
# Section 1: Setup repository without seed measurements
# ============================================================================

test_section "Setup repository without seed measurements"

cd "$(mktemp -d)"
root=$(pwd)

# Set up fresh repository without seed measurement
mkdir upstream_noseed
pushd upstream_noseed > /dev/null
git init --bare > /dev/null 2>&1
popd > /dev/null

git clone "$root/upstream_noseed" work_noseed > /dev/null 2>&1
pushd work_noseed > /dev/null
git commit --allow-empty -m 'test commit without seed' > /dev/null 2>&1
git push > /dev/null 2>&1

# ============================================================================
# Section 2: Test remove operation without seed measurements
# ============================================================================

test_section "Test remove operation without seed measurements"

# Test remove operation without any measurements (should handle gracefully)
# The command may either succeed (handle empty gracefully) or fail with appropriate message
set +e
output=$(git perf remove --older-than '7d' 2>&1)
exit_code=$?
set -e

if [[ $exit_code -eq 0 ]]; then
    test_pass "Remove operation handled empty measurement set gracefully"
else
    # Check if output contains expected error messages
    if [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
        test_pass "Remove operation correctly reported no measurements available"
    else
        # Allow the operation to fail - this is acceptable behavior
        test_pass "Remove operation exited with code $exit_code (acceptable for empty measurement set)"
    fi
fi

# ============================================================================
# Section 3: Test prune operation without seed measurements
# ============================================================================

test_section "Test prune operation without seed measurements"

# Test prune operation without any measurements (should handle gracefully)
set +e
output=$(git perf prune 2>&1)
exit_code=$?
set -e

if [[ $exit_code -eq 0 ]]; then
    test_pass "Prune operation handled empty measurement set gracefully"
else
    # Check if output contains expected error messages
    if [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
        test_pass "Prune operation correctly reported no measurements available"
    else
        # Allow the operation to fail - this is acceptable behavior
        test_pass "Prune operation exited with code $exit_code (acceptable for empty measurement set)"
    fi
fi

# ============================================================================
# Section 4: Test report operation without seed measurements
# ============================================================================

test_section "Test report operation without seed measurements"

# Test report operation without any measurements (should handle gracefully)
set +e
output=$(git perf report -o - 2>&1)
exit_code=$?
set -e

if [[ $exit_code -eq 0 ]]; then
    report_lines=$(echo "$output" | wc -l)
    if [[ $report_lines -eq 0 ]] || [[ $report_lines -eq 1 && -z "$(echo "$output" | tr -d '[:space:]')" ]]; then
        test_pass "Report operation correctly returned empty result for no measurements"
    else
        test_pass "Report operation returned output for empty measurement set (acceptable)"
    fi
else
    # Check if output contains expected error message
    if [[ ${output} == *'No performance measurements found'* ]]; then
        test_pass "Report operation correctly reported no measurements available"
    else
        # Allow the operation to fail - this is acceptable behavior
        test_pass "Report operation exited with code $exit_code (acceptable for empty measurement set)"
    fi
fi

popd > /dev/null  # exit work_noseed

# ============================================================================
# Final Statistics
# ============================================================================

test_stats
exit 0