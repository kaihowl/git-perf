#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Use the locally compiled binary"
export PATH="$(cargo metadata --format-version=1 | jq -r '.target_directory')/debug:$PATH"

test_section "Test the minimum absolute deviation threshold feature with note output"
cd_empty_repo

test_section "Create a config file with absolute threshold settings"
cat > .gitperfconfig << 'EOF'
[measurement."build_time"]
min_absolute_deviation = 100.0
EOF

test_section "Verify config file exists and has correct content"
assert_file_exists .gitperfconfig

test_section "Create some commits with measurements that have very low variance"
create_commit
git perf add -m build_time 1000
create_commit
git perf add -m build_time 1001
create_commit
git perf add -m build_time 1002

test_section "Add a measurement that should fail audit due to high z-score but pass due to low absolute deviation"
create_commit
git perf add -m build_time 1050

test_section "This should show the absolute threshold note and pass"
# |1050 - 1001| = 49 < 100 threshold
output=$(git perf audit -m build_time 2>&1)
assert_success git perf audit -m build_time
echo "$output" | grep -q "absolute deviation" && echo "PASS: threshold note present" || echo "FAIL: threshold note missing"

test_stats
exit 0
