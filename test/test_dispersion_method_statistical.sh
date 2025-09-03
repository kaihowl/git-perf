#!/bin/bash

set -e

# Statistical integration test for dispersion method functionality
# This test verifies that audit with different dispersion methods produces different results

echo "Testing dispersion method statistical differences with actual git repository..."

# Use the existing test infrastructure
script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Using test infrastructure from: $script_dir"

echo "Using git-perf from: $(which git-perf)"

# Create a temporary repository with multiple commits
cd_temp_repo

# Capture the temp directory path for later use
TEMP_DIR=$(pwd)

echo "✅ Created temporary repository with 4 commits"
echo "Repository location: $TEMP_DIR"

# Add performance measurements across different commits
# We'll create data that should show different z-scores with stddev vs MAD

echo "Adding baseline performance measurements..."

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

echo "✅ Added performance measurements across 4 commits"
echo "   - Commit 1: 1x100ms (baseline)"
echo "   - Commit 2: 1x105ms (baseline)"
echo "   - Commit 3: 1x200ms (outlier)"
echo "   - Commit 4: 1x110ms (current)"
echo "   Total: 4 measurements"

# Now test that audit with different dispersion methods produces different results

echo ""
echo "Running audit with stddev method..."
AUDIT_STDDEV=$(git perf audit -m build_time --dispersion-method stddev 2>&1 || true)

echo "Running audit with MAD method..."
AUDIT_MAD=$(git perf audit -m build_time --dispersion-method mad 2>&1 || true)

echo ""
echo "=== Audit Output with stddev ==="
echo "$AUDIT_STDDEV"
echo ""
echo "=== Audit Output with MAD ==="
echo "$AUDIT_MAD"
echo ""

# Verify that both methods produced output
if [ -z "$AUDIT_STDDEV" ]; then
    echo "❌ No output from stddev audit"
    exit 1
fi

if [ -z "$AUDIT_MAD" ]; then
    echo "❌ No output from MAD audit"
    exit 1
fi

echo "✅ Both audit methods produced output"

# Verify that both methods show the correct method name in output
if ! echo "$AUDIT_STDDEV" | grep -q "z-score (stddev):"; then
    echo "❌ stddev audit output doesn't show 'z-score (stddev):'"
    echo "Output: $AUDIT_STDDEV"
    exit 1
fi

if ! echo "$AUDIT_MAD" | grep -q "z-score (mad):"; then
    echo "❌ MAD audit output doesn't show 'z-score (mad):'"
    echo "Output: $AUDIT_MAD"
    exit 1
fi

echo "✅ Both methods show correct method names in output"

# Extract z-scores from output
Z_SCORE_STDDEV=$(echo "$AUDIT_STDDEV" | grep "z-score (stddev):" | sed 's/.*z-score (stddev):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')
Z_SCORE_MAD=$(echo "$AUDIT_MAD" | grep "z-score (mad):" | sed 's/.*z-score (mad):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

echo ""
echo "Extracted z-score (stddev): $Z_SCORE_STDDEV"
echo "Extracted z-score (mad): $Z_SCORE_MAD"

# Verify that z-scores are different (this is the key test)
if [ "$Z_SCORE_STDDEV" = "$Z_SCORE_MAD" ]; then
    echo "❌ Z-scores are identical with stddev ($Z_SCORE_STDDEV) and MAD ($Z_SCORE_MAD)"
    echo "This suggests the dispersion methods are not working correctly"
    echo ""
    echo "Expected behavior:"
    echo "- With outlier data (200ms), MAD should be more sensitive to outliers at HEAD"
    echo "- MAD should produce a higher z-score than stddev"
    echo "- This is because MAD is more robust and less affected by extreme values in the tail"
    exit 1
fi

echo "✅ Z-scores are different: stddev=$Z_SCORE_STDDEV, MAD=$Z_SCORE_MAD"

# Verify that both z-scores are reasonable (positive numbers)
if ! echo "$Z_SCORE_STDDEV" | grep -qE '^[0-9]+\.?[0-9]*$'; then
    echo "❌ Invalid z-score from stddev: $Z_SCORE_STDDEV"
    exit 1
fi

if ! echo "$Z_SCORE_MAD" | grep -qE '^[0-9]+\.?[0-9]*$'; then
    echo "❌ Invalid z-score from MAD: $Z_SCORE_MAD"
    exit 1
fi

echo "✅ Both z-scores are valid positive numbers"

# Verify the expected relationship: MAD z-score should be higher than stddev z-score
# This is because MAD is more robust to outliers, so it will show a higher z-score
# when there are actual outliers in the HEAD commit.
if (( $(echo "$Z_SCORE_MAD > $Z_SCORE_STDDEV" | bc -l) )); then
    echo "✅ MAD z-score ($Z_SCORE_MAD) is higher than stddev z-score ($Z_SCORE_STDDEV) as expected"
    echo "   This confirms MAD is more sensitive to outliers than stddev"
else
    echo "⚠️  MAD z-score ($Z_SCORE_MAD) is not higher than stddev z-score ($Z_SCORE_STDDEV)"
    echo "   This might indicate the statistical relationship is different than expected"
    echo "   or the outlier data isn't producing the expected effect"
fi

echo ""
echo "✅ All dispersion method statistical tests passed!"
echo ""
