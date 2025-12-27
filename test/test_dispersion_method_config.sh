#!/bin/bash

export TEST_TRACE=0

# Configuration integration test for dispersion method functionality
# This test verifies that configuration files properly override default dispersion methods

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Set PATH to use the built git-perf binary
export PATH="$(cd "$script_dir/.." && pwd)/target/debug:$PATH"

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

# Head commit: Add current measurement
git checkout master
git perf add -m build_time 110

test_section "Default behavior uses stddev"
assert_success_with_output AUDIT_DEFAULT git perf audit -m build_time
assert_contains "$AUDIT_DEFAULT" "z-score (stddev):"

test_section "CLI option overrides default"
assert_success_with_output AUDIT_CLI_MAD git perf audit -m build_time --dispersion-method mad
assert_contains "$AUDIT_CLI_MAD" "z-score (mad):"

test_section "Global configuration overrides default"
cat > .gitperfconfig << 'EOF'
[measurement]
dispersion_method = "mad"
EOF

# Run the audit command from the git repository directory
assert_success_with_output AUDIT_GLOBAL_MAD git perf audit -m build_time
assert_contains "$AUDIT_GLOBAL_MAD" "z-score (mad):"

test_section "Measurement-specific configuration overrides global"
cat > .gitperfconfig << 'EOF'
[measurement]
dispersion_method = "mad"

[measurement."build_time"]
dispersion_method = "stddev"
EOF

# Run the audit command from the git repository directory
assert_success_with_output AUDIT_MEASUREMENT_STDDEV git perf audit -m build_time
assert_contains "$AUDIT_MEASUREMENT_STDDEV" "z-score (stddev):"

test_section "CLI option overrides configuration"
assert_success_with_output AUDIT_CLI_OVERRIDE git perf audit -m build_time --dispersion-method mad
assert_contains "$AUDIT_CLI_OVERRIDE" "z-score (mad):"

test_section "Other measurements use global configuration"
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
[measurement]
dispersion_method = "mad"
EOF

# This audit may fail due to significant deviation, but we're testing that MAD dispersion is used
assert_failure_with_output AUDIT_OTHER_MEASUREMENT git perf audit -m memory_usage
assert_contains "$AUDIT_OTHER_MEASUREMENT" "z-score (mad):"

test_section "Invalid configuration falls back to default"
cat > .gitperfconfig << 'EOF'
[audit."*"]
dispersion_method = "invalid_value"
EOF

assert_success_with_output AUDIT_INVALID_CONFIG git perf audit -m build_time
assert_contains "$AUDIT_INVALID_CONFIG" "z-score (stddev):"

test_section "Malformed TOML falls back to default"
cat > .gitperfconfig << 'EOF'
[audit.*
dispersion_method = "mad"
EOF

assert_success_with_output AUDIT_MALFORMED_TOML git perf audit -m build_time
assert_contains "$AUDIT_MALFORMED_TOML" "z-score (stddev):"

test_section "Empty configuration falls back to default"
echo "" > .gitperfconfig

assert_success_with_output AUDIT_EMPTY_CONFIG git perf audit -m build_time
assert_contains "$AUDIT_EMPTY_CONFIG" "z-score (stddev):"

test_section "Verify dispersion method differences"
# Set global config to MAD
cat > .gitperfconfig << 'EOF'
[measurement]
dispersion_method = "mad"
EOF

# Run audit with MAD (from config)
assert_success_with_output AUDIT_CONFIG_MAD git perf audit -m build_time
Z_SCORE_CONFIG_MAD=$(echo "$AUDIT_CONFIG_MAD" | grep "z-score (mad):" | sed 's/.*z-score (mad):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

# Run audit with stddev (CLI override)
assert_success_with_output AUDIT_CLI_STDDEV git perf audit -m build_time --dispersion-method stddev
Z_SCORE_CLI_STDDEV=$(echo "$AUDIT_CLI_STDDEV" | grep "z-score (stddev):" | sed 's/.*z-score (stddev):[[:space:]]*↓[[:space:]]*\([0-9.]*\).*/\1/')

# Verify z-scores are different
assert_not_equals "$Z_SCORE_CONFIG_MAD" "$Z_SCORE_CLI_STDDEV" "Different dispersion methods should produce different z-scores"

test_stats
exit 0
