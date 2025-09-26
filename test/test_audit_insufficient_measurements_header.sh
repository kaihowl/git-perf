#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test audit insufficient measurements header format"
cd_temp_repo

# Add a single measurement (insufficient for default min_measurements=10)
git perf add -m test-metric 100

# Run audit and capture output - this should show the skip message with header
output=$(git perf audit -m test-metric --min-measurements 10 2>&1) || true

echo "Audit output: $output"

# Verify the output contains the header with measurement name
if [[ $output == *"⏭️ 'test-metric'"* ]]; then
    echo "✅ Header with measurement name found"
else
    echo "❌ Header with measurement name NOT found"
    echo "Expected: ⏭️ 'test-metric'"
    echo "Got: $output"
    exit 1
fi

# Verify the output contains the skip message
if [[ $output == *"Only 1 measurement found"* ]]; then
    echo "✅ Skip message found"
else
    echo "❌ Skip message NOT found"
    exit 1
fi

# Verify the output contains the threshold information
if [[ $output == *"Less than requested min_measurements of 10"* ]]; then
    echo "✅ Threshold information found"
else
    echo "❌ Threshold information NOT found"
    exit 1
fi

echo "Test passed: Audit insufficient measurements header format is correct"
exit 0