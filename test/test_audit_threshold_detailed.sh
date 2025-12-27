#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Use the locally compiled binary"
export PATH="$(cargo metadata --format-version=1 | jq -r '.target_directory')/debug:$PATH"

test_section "Test the minimum relative deviation threshold feature with more extreme values"
cd_empty_repo

test_section "Create a config file with threshold settings"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
min_relative_deviation = 10.0
EOF

test_section "Verify config file exists and has correct content"
assert_file_exists .gitperfconfig

test_section "Create some commits with measurements that have very low variance"
assert_success create_commit
assert_success git perf add -m build_time 1000
assert_success create_commit
assert_success git perf add -m build_time 1001
assert_success create_commit
assert_success git perf add -m build_time 1002

test_section "Add a measurement that should fail audit due to high z-score but pass due to low relative deviation"
assert_success create_commit
assert_success git perf add -m build_time 1050

test_section "This should show the threshold note if the relative deviation is below 10%"
assert_success git perf audit -m build_time

test_stats
exit 0
