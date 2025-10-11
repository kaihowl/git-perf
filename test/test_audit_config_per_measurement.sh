#!/bin/bash

set -e
set -x

# Configuration integration test for per-measurement audit parameter functionality
# This test verifies that different measurements can have different config values
# for min_measurements, aggregate_by, and sigma

echo "Testing per-measurement audit configuration..."

# Use the existing test infrastructure
script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Set PATH to use the built git-perf binary
export PATH="$(cd "$script_dir/.." && pwd)/target/debug:$PATH"

# Create a temporary repository with multiple commits
cd_temp_repo
# Add more commits to have enough measurements
create_commit
create_commit

echo "Created repository with 6 commits"

# Add performance measurements across different commits for TWO different metrics
echo "Adding performance measurements for build_time and memory_usage..."

# Add measurements to each commit (we have 6 commits from cd_temp_repo)
# Start from oldest and work towards HEAD
for i in {5..0}; do
    if [ $i -eq 0 ]; then
        git checkout master
    else
        git checkout master && git checkout HEAD~$i 2>/dev/null || git checkout master
    fi
    git perf add -m build_time $((100 + i * 5))
    git perf add -m memory_usage $((50 + i * 2))
done
git checkout master

echo "Added measurements for build_time and memory_usage across 6 commits"

# Test 1: Different min_measurements per measurement
echo "Test 1: Different min_measurements per measurement"
cat > .gitperfconfig << 'EOF'
[measurement]
min_measurements = 3

[measurement."build_time"]
min_measurements = 5

[measurement."memory_usage"]
min_measurements = 2
EOF

# We have 6 measurements total
# build_time requires 5, memory_usage requires 2 (both should have enough)
AUDIT_BUILD=$(git perf audit -m build_time -n 10 2>&1 || true)
assert_output_contains "$AUDIT_BUILD" "z-score"
assert_output_contains "$AUDIT_BUILD" "Tail:"

AUDIT_MEMORY=$(git perf audit -m memory_usage -n 10 2>&1 || true)
assert_output_contains "$AUDIT_MEMORY" "z-score"
assert_output_contains "$AUDIT_MEMORY" "Tail:"

# Test with insufficient measurements: build_time needs 5 but only get 2 (HEAD + 1 tail with n=1)
# This should skip the test
AUDIT_BUILD_INSUFFICIENT=$(git perf audit -m build_time -n 1 2>&1)
assert_output_contains "$AUDIT_BUILD_INSUFFICIENT" "min_measurements of 5"
assert_output_contains "$AUDIT_BUILD_INSUFFICIENT" "⏭️"
echo "✅ Different min_measurements per measurement works"

# Test 2: Different aggregate_by per measurement
echo "Test 2: Different aggregate_by per measurement"
cat > .gitperfconfig << 'EOF'
[measurement]
aggregate_by = "median"

[measurement."build_time"]
aggregate_by = "max"

[measurement."memory_usage"]
aggregate_by = "min"
EOF

# Run audits - we can't directly observe aggregate_by in output,
# but we verify the commands run without error
AUDIT_BUILD_AGG=$(git perf audit -m build_time -n 10 2>&1 || true)
AUDIT_MEMORY_AGG=$(git perf audit -m memory_usage -n 10 2>&1 || true)

assert_output_contains "$AUDIT_BUILD_AGG" "z-score"
assert_output_contains "$AUDIT_MEMORY_AGG" "z-score"
echo "✅ Different aggregate_by per measurement works"

# Test 3: Different sigma per measurement
echo "Test 3: Different sigma per measurement"
cat > .gitperfconfig << 'EOF'
[measurement]
sigma = 4.0

[measurement."build_time"]
sigma = 6.0

[measurement."memory_usage"]
sigma = 2.0
EOF

# Run audits - sigma affects pass/fail thresholds
AUDIT_BUILD_SIGMA=$(git perf audit -m build_time -n 10 2>&1 || true)
AUDIT_MEMORY_SIGMA=$(git perf audit -m memory_usage -n 10 2>&1 || true)

assert_output_contains "$AUDIT_BUILD_SIGMA" "z-score"
assert_output_contains "$AUDIT_MEMORY_SIGMA" "z-score"
echo "✅ Different sigma per measurement works"

# Test 4: Multiple measurements with different dispersion methods in ONE audit call
echo "Test 4: Multiple measurements with different dispersion methods in single audit"
cat > .gitperfconfig << 'EOF'
[measurement]
dispersion_method = "stddev"

[measurement."build_time"]
dispersion_method = "mad"

[measurement."memory_usage"]
dispersion_method = "stddev"
EOF

# Run audit with BOTH measurements at once
AUDIT_MULTI=$(git perf audit -m build_time -m memory_usage -n 10 2>&1 || true)

# Verify both measurements were audited with their respective methods
assert_output_contains "$AUDIT_MULTI" "build_time"
assert_output_contains "$AUDIT_MULTI" "memory_usage"

# build_time should use MAD, memory_usage should use stddev
assert_output_contains "$AUDIT_MULTI" "z-score (mad):"
assert_output_contains "$AUDIT_MULTI" "z-score (stddev):"
echo "✅ Multiple measurements use different dispersion methods in single audit"

# Test 5: CLI option overrides config for specific measurement
echo "Test 5: CLI option overrides per-measurement config"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
min_measurements = 10
aggregate_by = "max"
sigma = 6.0
dispersion_method = "mad"

[measurement."memory_usage"]
min_measurements = 8
dispersion_method = "mad"
EOF

# CLI should override all config values
AUDIT_CLI_OVERRIDE=$(git perf audit -m build_time -n 10 --min-measurements 2 -a min -d 3.0 --dispersion-method stddev 2>&1 || true)
assert_output_contains "$AUDIT_CLI_OVERRIDE" "z-score (stddev):"

# CRITICAL: CLI --min-measurements should apply to ALL measurements
# Config says build_time needs 10 and memory_usage needs 8, but CLI says 2 for all
AUDIT_CLI_MIN_ALL=$(git perf audit -m build_time -m memory_usage -n 3 --min-measurements 2 2>&1 || true)
# Both should succeed with only 3 measurements because CLI overrides config for ALL
assert_output_contains "$AUDIT_CLI_MIN_ALL" "build_time"
assert_output_contains "$AUDIT_CLI_MIN_ALL" "memory_usage"
assert_output_contains "$AUDIT_CLI_MIN_ALL" "z-score"
echo "✅ CLI options override per-measurement config"

# Test 6: All four parameters different for three measurements
echo "Test 6: Three measurements with different configs for all parameters"

# Add a third measurement
for i in {5..0}; do
    if [ $i -eq 0 ]; then
        git checkout master
    else
        git checkout master && git checkout HEAD~$i 2>/dev/null || git checkout master
    fi
    git perf add -m test_metric $((200 + i * 10))
done
git checkout master

cat > .gitperfconfig << 'EOF'
[measurement]
min_measurements = 3
aggregate_by = "median"
sigma = 4.0
dispersion_method = "stddev"

[measurement."build_time"]
min_measurements = 5
aggregate_by = "max"
sigma = 6.0
dispersion_method = "mad"

[measurement."memory_usage"]
min_measurements = 2
aggregate_by = "min"
sigma = 2.0
dispersion_method = "stddev"

[measurement."test_metric"]
min_measurements = 4
aggregate_by = "mean"
sigma = 5.0
dispersion_method = "mad"
EOF

# Run audit with all three measurements
AUDIT_THREE=$(git perf audit -m build_time -m memory_usage -m test_metric -n 10 2>&1 || true)

# Verify all three were audited
assert_output_contains "$AUDIT_THREE" "build_time"
assert_output_contains "$AUDIT_THREE" "memory_usage"
assert_output_contains "$AUDIT_THREE" "test_metric"

# Verify dispersion methods are correct
assert_output_contains "$AUDIT_THREE" "z-score (mad):"
assert_output_contains "$AUDIT_THREE" "z-score (stddev):"
echo "✅ Three measurements with different configs for all parameters"

# Test 7: Config falls back to defaults when measurement-specific not defined
echo "Test 7: Falls back to global config when measurement-specific not defined"
cat > .gitperfconfig << 'EOF'
[measurement]
min_measurements = 4
aggregate_by = "mean"
sigma = 3.5
dispersion_method = "mad"

[measurement."build_time"]
dispersion_method = "stddev"
EOF

# build_time should use stddev (specific) but inherit other global settings
# memory_usage should use all global settings
AUDIT_FALLBACK=$(git perf audit -m build_time -m memory_usage -n 10 2>&1 || true)

assert_output_contains "$AUDIT_FALLBACK" "z-score (stddev):"
assert_output_contains "$AUDIT_FALLBACK" "z-score (mad):"
echo "✅ Correctly falls back to global config"

# Test 8: Warning when max_count < min_measurements (config)
echo "Test 8: Warning when max_count < min_measurements from config"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
min_measurements = 10

[measurement."memory_usage"]
min_measurements = 15
EOF

# Test with max_count=3, which is less than both config values
# This should produce warnings for both measurements
AUDIT_WARNING=$(git perf audit -m build_time -m memory_usage -n 3 2>&1 || true)

# Verify warnings appear for both measurements
assert_output_contains "$AUDIT_WARNING" "Warning: --max_count (3) is less than min_measurements (10)"
assert_output_contains "$AUDIT_WARNING" "measurement 'build_time'"
assert_output_contains "$AUDIT_WARNING" "Warning: --max_count (3) is less than min_measurements (15)"
assert_output_contains "$AUDIT_WARNING" "measurement 'memory_usage'"
assert_output_contains "$AUDIT_WARNING" "limits available historical data"
echo "✅ Warning displayed when max_count < config min_measurements"

# Test 9: No warning when max_count >= min_measurements
echo "Test 9: No warning when max_count >= min_measurements"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
min_measurements = 3
EOF

# Test with max_count=5, which is >= config min_measurements (3)
# This should NOT produce a warning
AUDIT_NO_WARNING=$(git perf audit -m build_time -n 5 2>&1 || true)

# Verify no warning appears
if echo "$AUDIT_NO_WARNING" | grep -q "Warning.*max_count"; then
    echo "❌ Unexpected warning when max_count >= min_measurements"
    echo "Output was: $AUDIT_NO_WARNING"
    exit 1
fi
echo "✅ No warning when max_count >= min_measurements"

# Test 10: No warning when CLI provides both max_count and min_measurements
# (CLI validation should prevent invalid combinations)
echo "Test 10: CLI validation prevents invalid max_count/min_measurements combination"

# This should fail with CLI validation error, not reach our warning
AUDIT_CLI_INVALID=$(git perf audit -m build_time -n 3 --min-measurements 5 2>&1 || true)

# Should have CLI validation error, not our runtime warning
assert_output_contains "$AUDIT_CLI_INVALID" "minimal number of measurements"
assert_output_contains "$AUDIT_CLI_INVALID" "cannot be more than"

# Should NOT have our runtime warning (because CLI validation prevented execution)
if echo "$AUDIT_CLI_INVALID" | grep -q "limits available historical data"; then
    echo "❌ Runtime warning appeared when CLI should have prevented execution"
    echo "Output was: $AUDIT_CLI_INVALID"
    exit 1
fi
echo "✅ CLI validation prevents invalid combinations before runtime"

echo "All per-measurement configuration tests passed!"
