#!/bin/bash
# Integration tests for git perf config command

set -e
set -x

# Source common test utilities
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Config command tests - no config"
cd_temp_repo
# Should work even without config
git perf config --list

echo "Config command tests - basic list"
cd_temp_repo
# Create a basic config
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
unit = "ms"

[measurement.test_time]
epoch = "87654321"
unit = "s"
EOF
# List config
output=$(git perf config --list)
# Should show both measurements
echo "$output" | grep -q "build_time"
echo "$output" | grep -q "test_time"
echo "$output" | grep -q "12345678"
echo "$output" | grep -q "87654321"

echo "Config command tests - detailed list"
cd_temp_repo
# Create a config with various settings
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
min_relative_deviation = 5.0
dispersion_method = "mad"
min_measurements = 10
aggregate_by = "median"
sigma = 2.0
unit = "ms"
EOF
# List with detailed flag
output=$(git perf config --list --detailed)
# Should show all settings
echo "$output" | grep -q "epoch.*12345678"
echo "$output" | grep -q "min_relative_deviation.*5"
echo "$output" | grep -q "dispersion_method"
echo "$output" | grep -q "min_measurements.*10"
echo "$output" | grep -q "aggregate_by.*median"
echo "$output" | grep -q "sigma.*2"
echo "$output" | grep -q "unit.*ms"

echo "Config command tests - JSON output"
cd_temp_repo
# Create a basic config
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
unit = "ms"
EOF
# List as JSON
output=$(git perf config --list --format json)
# Should be valid JSON
echo "$output" | jq . > /dev/null
# Should contain expected fields
echo "$output" | jq -e '.git_context' > /dev/null
echo "$output" | jq -e '.config_sources' > /dev/null
echo "$output" | jq -e '.global_settings' > /dev/null
echo "$output" | jq -e '.measurements' > /dev/null
echo "$output" | jq -e '.measurements.build_time' > /dev/null

echo "Config command tests - measurement filter"
cd_temp_repo
# Create config with multiple measurements
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"

[measurement.test_time]
epoch = "87654321"
EOF
# Filter for specific measurement
output=$(git perf config --list --measurement build_time)
# Should show build_time
echo "$output" | grep -q "build_time"
# Should NOT show test_time
! echo "$output" | grep -q "test_time"

echo "Config command tests - validate valid config"
cd_temp_repo
# Create valid config
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
min_relative_deviation = 5.0
min_measurements = 10
sigma = 3.0
unit = "ms"
EOF
# Validate should pass (exit 0)
git perf config --list --validate

echo "Config command tests - validate missing epoch"
cd_temp_repo
# Create config without epoch
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
unit = "ms"
EOF
# Validate should fail (exit non-zero)
if git perf config --list --validate 2>&1; then
    echo "Expected validation to fail due to missing epoch"
    exit 1
fi
# Should show validation error
output=$(git perf config --list --validate 2>&1 || true)
echo "$output" | grep -q "No epoch configured"

echo "Config command tests - validate invalid sigma"
cd_temp_repo
# Create config with invalid sigma
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
sigma = -1.0
EOF
# Validate should fail
if git perf config --list --validate 2>&1; then
    echo "Expected validation to fail due to invalid sigma"
    exit 1
fi
# Should show validation error
output=$(git perf config --list --validate 2>&1 || true)
echo "$output" | grep -q "Invalid sigma value"

echo "Config command tests - validate invalid min_measurements"
cd_temp_repo
# Create config with invalid min_measurements
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
min_measurements = 1
EOF
# Validate should fail
if git perf config --list --validate 2>&1; then
    echo "Expected validation to fail due to invalid min_measurements"
    exit 1
fi
# Should show validation error
output=$(git perf config --list --validate 2>&1 || true)
echo "$output" | grep -q "Invalid min_measurements"

echo "Config command tests - validate multiple issues"
cd_temp_repo
# Create config with multiple issues
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
sigma = -1.0
min_relative_deviation = -5.0
min_measurements = 1
EOF
# Validate should fail
if git perf config --list --validate 2>&1; then
    echo "Expected validation to fail due to multiple issues"
    exit 1
fi
# Should show multiple validation errors
output=$(git perf config --list --validate 2>&1 || true)
echo "$output" | grep -q "Invalid sigma value"
echo "$output" | grep -q "Invalid min_relative_deviation"
echo "$output" | grep -q "Invalid min_measurements"

echo "Config command tests - git context"
cd_temp_repo
# Create a config
cat > .gitperfconfig << 'EOF'
[measurement.build_time]
epoch = "12345678"
EOF
# Output should show git context
output=$(git perf config --list)
echo "$output" | grep -q "Git Context:"
echo "$output" | grep -q "Branch:"
echo "$output" | grep -q "Repository:"

echo "All config command tests passed!"
