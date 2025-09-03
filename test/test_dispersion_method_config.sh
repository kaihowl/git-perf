#!/bin/bash

set -e
set -x

# Configuration integration test for dispersion method functionality
# This test verifies that configuration files properly override default dispersion methods

echo "Testing dispersion method configuration integration..."

# Use the existing test infrastructure
script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Set PATH to use the built git-perf binary
export PATH="$(cd "$script_dir/.." && pwd)/target/debug:$PATH"

# Create a temporary repository with multiple commits
cd_temp_repo

echo "Created repository with 4 commits"

# Add performance measurements across different commits
echo "Adding performance measurements..."

# Add measurements to different commits to form the tail
# First commit: Add baseline measurements (normal performance)
git perf add -m build_time 100

# Second commit: Add more baseline measurements
git checkout master && git checkout HEAD~2
git perf add -m build_time 105

# Third commit: Add outlier measurement
git checkout master && git checkout HEAD~1
git perf add -m build_time 200

# Head commit: Add current measurement
git checkout master
git perf add -m build_time 110

echo "Added 4 measurements across 4 commits"

# Test 1: Default behavior (no config, no CLI option) should use stddev
echo "Test 1: Default behavior uses stddev"
AUDIT_DEFAULT=$(git perf audit -m build_time 2>&1 || true)
echo "$AUDIT_DEFAULT" | grep -q "z-score (stddev):" || exit 1
echo "✅ Default behavior correctly uses stddev"

# Test 2: CLI option overrides default
echo "Test 2: CLI option overrides default"
AUDIT_CLI_MAD=$(git perf audit -m build_time --dispersion-method mad 2>&1 || true)
echo "$AUDIT_CLI_MAD" | grep -q "z-score (mad):" || exit 1
echo "✅ CLI option correctly overrides default"

# Test 3: Global configuration overrides default
echo "Test 3: Global configuration overrides default"
cat > .gitperfconfig << 'EOF'
[audit.global]
dispersion_method = "mad"
EOF

# Verify the configuration file exists and show its contents
echo "Configuration file contents:"
cat .gitperfconfig
echo "Current working directory: $(pwd)"
echo "Configuration file exists: $(ls -la .gitperfconfig)"

# Run the audit command from the git repository directory
AUDIT_GLOBAL_MAD=$(git perf audit -m build_time 2>&1 || true)
echo "Audit output: $AUDIT_GLOBAL_MAD"
echo "$AUDIT_GLOBAL_MAD" | grep -q "z-score (mad):" || exit 1
echo "✅ Global configuration correctly overrides default"

# Test 4: Measurement-specific configuration overrides global
echo "Test 4: Measurement-specific configuration overrides global"
cat > .gitperfconfig << 'EOF'
[audit.global]
dispersion_method = "mad"

[audit.measurement."build_time"]
dispersion_method = "stddev"
EOF

# Run the audit command from the git repository directory
AUDIT_MEASUREMENT_STDDEV=$(git perf audit -m build_time 2>&1 || true)
echo "$AUDIT_MEASUREMENT_STDDEV" | grep -q "z-score (stddev):" || exit 1
echo "✅ Measurement-specific configuration correctly overrides global"

# Test 5: CLI option overrides configuration
echo "Test 5: CLI option overrides configuration"
AUDIT_CLI_OVERRIDE=$(git perf audit -m build_time --dispersion-method mad 2>&1 || true)
echo "$AUDIT_CLI_OVERRIDE" | grep -q "z-score (mad):" || exit 1
echo "✅ CLI option correctly overrides configuration"

# Test 6: Other measurements use global configuration
echo "Test 6: Other measurements use global configuration"
# Add a different measurement with enough data points
git checkout master
git perf add -m memory_usage 50

# Add more memory_usage measurements to different commits
git checkout HEAD~2
git perf add -m memory_usage 55

git checkout HEAD~1
git perf add -m memory_usage 60

git checkout master
git perf add -m memory_usage 45

# Set global config to MAD
cat > .gitperfconfig << 'EOF'
[audit.global]
dispersion_method = "mad"
EOF

AUDIT_OTHER_MEASUREMENT=$(git perf audit -m memory_usage 2>&1 || true)
echo "$AUDIT_OTHER_MEASUREMENT" | grep -q "z-score (mad):" || exit 1
echo "✅ Other measurements correctly use global configuration"

# Test 7: Invalid configuration falls back to default
echo "Test 7: Invalid configuration falls back to default"
cat > .gitperfconfig << 'EOF'
[audit.global]
dispersion_method = "invalid_value"
EOF

AUDIT_INVALID_CONFIG=$(git perf audit -m build_time 2>&1 || true)
echo "$AUDIT_INVALID_CONFIG" | grep -q "z-score (stddev):" || exit 1
echo "✅ Invalid configuration correctly falls back to default"

# Test 8: Malformed TOML falls back to default
echo "Test 8: Malformed TOML falls back to default"
cat > .gitperfconfig << 'EOF'
[audit.global
dispersion_method = "mad"
EOF

AUDIT_MALFORMED_TOML=$(git perf audit -m build_time 2>&1 || true)
echo "$AUDIT_MALFORMED_TOML" | grep -q "z-score (stddev):" || exit 1
echo "✅ Malformed TOML correctly falls back to default"

# Test 9: Empty configuration falls back to default
echo "Test 9: Empty configuration falls back to default"
echo "" > .gitperfconfig

AUDIT_EMPTY_CONFIG=$(git perf audit -m build_time 2>&1 || true)
echo "$AUDIT_EMPTY_CONFIG" | grep -q "z-score (stddev):" || exit 1
echo "✅ Empty configuration correctly falls back to default"

# Test 10: Verify that different dispersion methods produce different results
echo "Test 10: Verify dispersion method differences"
# Set global config to MAD
cat > .gitperfconfig << 'EOF'
[audit.global]
dispersion_method = "mad"
EOF

# Run audit with MAD (from config)
AUDIT_CONFIG_MAD=$(git perf audit -m build_time 2>&1 || true)
Z_SCORE_CONFIG_MAD=$(echo "$AUDIT_CONFIG_MAD" | grep "z-score (mad):" | sed 's/.*z-score (mad):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

# Run audit with stddev (CLI override)
AUDIT_CLI_STDDEV=$(git perf audit -m build_time --dispersion-method stddev 2>&1 || true)
Z_SCORE_CLI_STDDEV=$(echo "$AUDIT_CLI_STDDEV" | grep "z-score (stddev):" | sed 's/.*z-score (stddev):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

echo "Config MAD z-score: $Z_SCORE_CONFIG_MAD"
echo "CLI stddev z-score: $Z_SCORE_CLI_STDDEV"

# Verify z-scores are different
[ "$Z_SCORE_CONFIG_MAD" != "$Z_SCORE_CLI_STDDEV" ] || exit 1
echo "✅ Different dispersion methods produce different z-scores"

echo "All configuration integration tests passed!"
