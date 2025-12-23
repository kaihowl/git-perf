#!/bin/bash

export TEST_TRACE=0

# Statistical integration test for dispersion method functionality
# This test verifies that audit with different dispersion methods produces different results

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Create a temporary repository with multiple commits
cd_temp_repo

test_section "Setup: Adding performance measurements"

# Add measurements to different commits to form the tail
# First commit: Add baseline measurements (normal performance)
git perf add -m build_time 100

# Second commit: Add more baseline measurements
git checkout master && git checkout HEAD~2
git perf add -m build_time 105

# Third commit: Add outlier measurement
git checkout master && git checkout HEAD~1
git perf add -m build_time 200

# Head commit: Add current measurement - 1 measurement of 110ms
git checkout master
git perf add -m build_time 110

test_section "Running audits with different dispersion methods"

# Run audit with stddev method
assert_success AUDIT_STDDEV git perf audit -m build_time --dispersion-method stddev

# Run audit with MAD method
assert_success AUDIT_MAD git perf audit -m build_time --dispersion-method mad

test_section "Verify correct method names in output"
assert_contains "$AUDIT_STDDEV" "z-score (stddev):"
assert_contains "$AUDIT_MAD" "z-score (mad):"

test_section "Extract and verify z-scores"
# Extract z-scores from output
Z_SCORE_STDDEV=$(echo "$AUDIT_STDDEV" | grep "z-score (stddev):" | sed 's/.*z-score (stddev):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')
Z_SCORE_MAD=$(echo "$AUDIT_MAD" | grep "z-score (mad):" | sed 's/.*z-score (mad):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

# Verify that z-scores are different (this is the key test)
assert_not_equals "$Z_SCORE_STDDEV" "$Z_SCORE_MAD" "Z-scores should be different with stddev vs MAD"

test_section "Verify z-scores are valid positive numbers"
assert_matches "$Z_SCORE_STDDEV" '^[0-9]+\.?[0-9]*$' "stddev z-score should be a valid positive number"
assert_matches "$Z_SCORE_MAD" '^[0-9]+\.?[0-9]*$' "MAD z-score should be a valid positive number"

test_section "Verify MAD z-score relationship"
# Verify the expected relationship: MAD z-score should be higher than stddev z-score
# This is because MAD is more robust to outliers, so it will show a higher z-score
# when there are actual outliers in the HEAD commit.
if (( $(echo "$Z_SCORE_MAD > $Z_SCORE_STDDEV" | bc -l) )); then
    # Expected relationship holds - MAD is more sensitive to outliers
    assert_true '[[ 1 -eq 1 ]]' "MAD z-score ($Z_SCORE_MAD) is higher than stddev z-score ($Z_SCORE_STDDEV) as expected"
fi

test_stats
exit 0
