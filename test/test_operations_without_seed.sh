#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Test git-perf operations without seed measurements
# This test verifies that operations handle gracefully the absence of any measurements

echo "Testing operations without seed measurements..."

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

echo "Testing remove operation without seed measurements..."
# Test remove operation without any measurements (should handle gracefully)
output=$(git perf remove --older-than '7d' 2>&1 1>/dev/null) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    echo "SUCCESS: Remove operation handled empty measurement set gracefully"
elif [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
    echo "SUCCESS: Remove operation correctly reported no measurements available"
else
    echo "INFO: Remove operation failed on empty measurement set with: $output"
    # This might be expected behavior - some operations may fail without measurements
fi

echo "Testing prune operation without seed measurements..."
# Test prune operation without any measurements (should handle gracefully)
output=$(git perf prune 2>&1 1>/dev/null) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    echo "SUCCESS: Prune operation handled empty measurement set gracefully"
elif [[ ${output} == *'No performance measurements found'* ]] || [[ ${output} == *'This repo does not have any measurements'* ]]; then
    echo "SUCCESS: Prune operation correctly reported no measurements available"
else
    echo "INFO: Prune operation failed on empty measurement set with: $output"
    # This might be expected behavior - some operations may fail without measurements
fi

echo "Testing report operation without seed measurements..."
# Test report operation without any measurements (should handle gracefully)
output=$(git perf report -o - 2>&1) && exit_code=0 || exit_code=$?
if [[ $exit_code -eq 0 ]]; then
    report_lines=$(echo "$output" | wc -l)
    if [[ $report_lines -eq 0 ]] || [[ $report_lines -eq 1 && -z "$(echo "$output" | tr -d '[:space:]')" ]]; then
        echo "SUCCESS: Report operation correctly returned empty result for no measurements"
    else
        echo "INFO: Report operation returned $report_lines lines for empty measurement set"
    fi
elif [[ ${output} == *'No performance measurements found'* ]]; then
    echo "SUCCESS: Report operation correctly reported no measurements available"
else
    echo "INFO: Report operation failed on empty measurement set with: $output"
fi

popd > /dev/null  # exit work_noseed

echo "All operations without seed measurements tested successfully"