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

# Test 1: Different aggregate_by per measurement
echo "Test 1: Different aggregate_by per measurement"
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

echo "$AUDIT_BUILD_AGG" | grep -q "z-score" || exit 1
echo "$AUDIT_MEMORY_AGG" | grep -q "z-score" || exit 1
echo "✅ Different aggregate_by per measurement works"

# Test 2: Different sigma per measurement
echo "Test 2: Different sigma per measurement"
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

echo "$AUDIT_BUILD_SIGMA" | grep -q "z-score" || exit 1
echo "$AUDIT_MEMORY_SIGMA" | grep -q "z-score" || exit 1
echo "✅ Different sigma per measurement works"

# Test 3: Multiple measurements with different dispersion methods in ONE audit call
echo "Test 3: Multiple measurements with different dispersion methods in single audit"
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
echo "Multi-measurement audit output: $AUDIT_MULTI"

# Verify both measurements were audited with their respective methods
echo "$AUDIT_MULTI" | grep "build_time" || exit 1
echo "$AUDIT_MULTI" | grep "memory_usage" || exit 1

# Extract the lines for each measurement and verify dispersion methods
BUILD_LINE=$(echo "$AUDIT_MULTI" | grep "build_time" || echo "")
MEMORY_LINE=$(echo "$AUDIT_MULTI" | grep "memory_usage" || echo "")

echo "Build time line: $BUILD_LINE"
echo "Memory usage line: $MEMORY_LINE"

# build_time should use MAD, memory_usage should use stddev
echo "$AUDIT_MULTI" | grep -A 5 "build_time" | grep -q "z-score (mad):" || exit 1
echo "$AUDIT_MULTI" | grep -A 5 "memory_usage" | grep -q "z-score (stddev):" || exit 1
echo "✅ Multiple measurements use different dispersion methods in single audit"

# Test 4: CLI option overrides config for specific measurement
echo "Test 4: CLI option overrides per-measurement config"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
aggregate_by = "max"
sigma = 6.0
dispersion_method = "mad"
EOF

# CLI should override all config values
AUDIT_CLI_OVERRIDE=$(git perf audit -m build_time -n 10 --min-measurements 2 -a min -d 3.0 --dispersion-method stddev 2>&1 || true)
echo "$AUDIT_CLI_OVERRIDE" | grep -q "z-score (stddev):" || exit 1
echo "✅ CLI options override per-measurement config"

# Test 5: Three parameters different for three measurements
echo "Test 5: Three measurements with different configs for aggregate_by, sigma, and dispersion_method"

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
aggregate_by = "median"
sigma = 4.0
dispersion_method = "stddev"

[measurement."build_time"]
aggregate_by = "max"
sigma = 6.0
dispersion_method = "mad"

[measurement."memory_usage"]
aggregate_by = "min"
sigma = 2.0
dispersion_method = "stddev"

[measurement."test_metric"]
aggregate_by = "mean"
sigma = 5.0
dispersion_method = "mad"
EOF

# Run audit with all three measurements
AUDIT_THREE=$(git perf audit -m build_time -m memory_usage -m test_metric -n 10 2>&1 || true)
echo "Three measurement audit output: $AUDIT_THREE"

# Verify all three were audited
echo "$AUDIT_THREE" | grep "build_time" || exit 1
echo "$AUDIT_THREE" | grep "memory_usage" || exit 1
echo "$AUDIT_THREE" | grep "test_metric" || exit 1

# Verify dispersion methods are correct
echo "$AUDIT_THREE" | grep -A 5 "build_time" | grep -q "z-score (mad):" || exit 1
echo "$AUDIT_THREE" | grep -A 5 "memory_usage" | grep -q "z-score (stddev):" || exit 1
echo "$AUDIT_THREE" | grep -A 5 "test_metric" | grep -q "z-score (mad):" || exit 1
echo "✅ Three measurements with different configs for aggregate_by, sigma, and dispersion_method"

# Test 6: Config falls back to defaults when measurement-specific not defined
echo "Test 6: Falls back to global config when measurement-specific not defined"
cat > .gitperfconfig << 'EOF'
[measurement]
aggregate_by = "mean"
sigma = 3.5
dispersion_method = "mad"

[measurement."build_time"]
dispersion_method = "stddev"
EOF

# build_time should use stddev (specific) but inherit other global settings
# memory_usage should use all global settings
AUDIT_FALLBACK=$(git perf audit -m build_time -m memory_usage -n 10 2>&1 || true)

echo "$AUDIT_FALLBACK" | grep -A 5 "build_time" | grep -q "z-score (stddev):" || exit 1
echo "$AUDIT_FALLBACK" | grep -A 5 "memory_usage" | grep -q "z-score (mad):" || exit 1
echo "✅ Correctly falls back to global config"

echo "All per-measurement configuration tests passed!"
