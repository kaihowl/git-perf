#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test audit insufficient measurements header format"
cd_temp_repo

# Add measurements to multiple commits to create some history, but still insufficient for min_measurements=10
git perf add -m test-metric 100

# Go to previous commit and add measurement there
git checkout HEAD~1
git perf add -m test-metric 95

# Go to another previous commit and add measurement
git checkout HEAD~1
git perf add -m test-metric 98

# Return to master for audit
git checkout master

# Run audit and capture output - this should show the skip message with header (3 measurements < 10)
output=$(git perf audit -m test-metric --min-measurements 10 2>&1) || true

test_section "Audit output: $output"

# Verify the output contains the header with measurement name
assert_contains "$output" "⏭️ 'test-metric'" "Header with measurement name NOT found"
echo "✅ Header with measurement name found"

# Verify the output contains the skip message (should be 2 tail measurements + 1 head = 3 total, but only 2 in tail)
assert_contains "$output" "Only 2 historical measurements" "Skip message NOT found"
echo "✅ Skip message found"

# Verify the output contains the threshold information
assert_contains "$output" "Less than requested min_measurements of 10" "Threshold information NOT found"
echo "✅ Threshold information found"

# Verify the output contains a sparkline with range (check for range format and sparkline characters)
if echo "$output" | grep -q "\[.*%.*–.*%\].*[▁▂▃▄▅▆▇█]"; then
    echo "✅ Sparkline with range found in skip message"
else
    echo "❌ ERROR: Sparkline with range NOT found in skip message"
    exit 1
fi

test_section "Test passed: Audit insufficient measurements header format is correct"
test_stats
exit 0
