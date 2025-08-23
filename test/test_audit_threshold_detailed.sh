#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Use the locally compiled binary
export PATH="$(cargo metadata --format-version=1 | jq -r '.target_directory')/debug:$PATH"

# Test the minimum relative deviation threshold feature with more extreme values
cd_empty_repo

# Create a config file with threshold settings
cat > .gitperfconfig << 'EOF'
[audit.measurement."build_time"]
min_relative_deviation = 10.0
EOF

# Debug: Check if config file exists and has correct content
echo "Current directory: $(pwd)"
echo "Config file content:"
cat .gitperfconfig

# Create some commits with measurements that have very low variance
create_commit
git perf add -m build_time 1000
create_commit
git perf add -m build_time 1001
create_commit
git perf add -m build_time 1002

# Add a measurement that should fail audit due to high z-score
# but pass due to low relative deviation (should be around 5%)
create_commit
git perf add -m build_time 1050

# This should show the threshold note if the relative deviation is below 10%
echo "Testing audit with high z-score but low relative deviation:"
git perf audit -m build_time

echo "All detailed threshold tests completed!"
