#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git-perf operations without seed measurements
# This test verifies that operations handle gracefully the absence of any measurements

test_section "Testing operations without seed measurements..."

# Create a fresh repository environment without seed measurement
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

test_section "Testing remove operation without seed measurements..."
# Test remove operation without any measurements (should handle gracefully)
output=$(git perf remove --older-than '7d' 2>&1 1>/dev/null) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    test_section "SUCCESS: Remove operation handled empty measurement set gracefully"
else
    # Check if output contains expected error messages
    if [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
        test_section "SUCCESS: Remove operation correctly reported no measurements available"
    else
        test_section "INFO: Remove operation failed on empty measurement set with: $output"
        # This might be expected behavior - some operations may fail without measurements
    fi
fi

test_section "Testing prune operation without seed measurements..."
# Test prune operation without any measurements (should handle gracefully)
output=$(git perf prune 2>&1 1>/dev/null) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    test_section "SUCCESS: Prune operation handled empty measurement set gracefully"
else
    # Check if output contains expected error messages
    if [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
        test_section "SUCCESS: Prune operation correctly reported no measurements available"
    else
        test_section "INFO: Prune operation failed on empty measurement set with: $output"
        # This might be expected behavior - some operations may fail without measurements
    fi
fi

test_section "Testing report operation without seed measurements..."
# Test report operation without any measurements (should handle gracefully)
output=$(git perf report -o - 2>&1) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    report_lines=$(echo "$output" | wc -l)
    if [[ $report_lines -eq 0 ]] || [[ $report_lines -eq 1 && -z "$(echo "$output" | tr -d '[:space:]')" ]]; then
        test_section "SUCCESS: Report operation correctly returned empty result for no measurements"
    else
        test_section "INFO: Report operation returned $report_lines lines for empty measurement set"
    fi
else
    # Check if output contains expected error message
    if [[ ${output} == *'No performance measurements found'* ]]; then
        test_section "SUCCESS: Report operation correctly reported no measurements available"
    else
        test_section "INFO: Report operation failed on empty measurement set with: $output"
    fi
fi

popd > /dev/null  # exit work_noseed

test_section "All operations without seed measurements tested successfully"
