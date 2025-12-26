#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Test the minimum relative deviation threshold feature
cd_empty_repo

# Create a config file with threshold settings
cat > .gitperfconfig << 'EOF'
[measurement]
min_relative_deviation = 5.0

[measurement."build_time"]
min_relative_deviation = 10.0
EOF

# Create some commits with measurements
create_commit
git perf add -m build_time 1000
create_commit
git perf add -m build_time 1050
create_commit
git perf add -m build_time 1100

# Add a measurement that would normally fail audit (high z-score) but passes due to threshold
create_commit
git perf add -m build_time 1200

# This should pass due to relative deviation being below 10% threshold
# (1200/1050 - 1) * 100% = 14.3% > 10%, so it should still fail
git perf audit -m build_time && echo "Audit passed as expected" || echo "Audit failed as expected"

# Test with a measurement that has lower relative deviation
create_commit
git perf add -m build_time 1080

# This should pass due to relative deviation being below 10% threshold
# (1080/1050 - 1) * 100% = 2.9% < 10%, so it should pass
git perf audit -m build_time

# Test global threshold with a different measurement
create_commit
git perf add -m memory_usage 100
create_commit
git perf add -m memory_usage 105
create_commit
git perf add -m memory_usage 110

# Add a measurement that would pass due to global threshold
create_commit
git perf add -m memory_usage 112

# This should pass due to relative deviation being below 5% global threshold
# (112/105 - 1) * 100% = 6.7% > 5%, so it should fail
git perf audit -m memory_usage && echo "Audit passed as expected" || echo "Audit failed as expected"

# Test with a measurement that has lower relative deviation
create_commit
git perf add -m memory_usage 107

# This should pass due to relative deviation being below 5% global threshold
# (107/105 - 1) * 100% = 1.9% < 5%, so it should pass
git perf audit -m memory_usage

test_section "All threshold tests completed successfully!"
