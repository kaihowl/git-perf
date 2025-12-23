#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test the minimum relative deviation threshold feature"
cd_empty_repo

test_section "Create a config file with threshold settings"
cat > .gitperfconfig << 'EOF'
[measurement]
min_relative_deviation = 5.0

[measurement."build_time"]
min_relative_deviation = 10.0
EOF

test_section "Create some commits with measurements"
assert_success create_commit
assert_success git perf add -m build_time 1000
assert_success create_commit
assert_success git perf add -m build_time 1050
assert_success create_commit
assert_success git perf add -m build_time 1100

test_section "Add a measurement that would normally fail audit (high z-score) but passes due to threshold"
assert_success create_commit
assert_success git perf add -m build_time 1200

test_section "This should fail due to relative deviation being above 10% threshold"
# (1200/1050 - 1) * 100% = 14.3% > 10%, so it should still fail
assert_failure git perf audit -m build_time

test_section "Test with a measurement that has lower relative deviation"
assert_success create_commit
assert_success git perf add -m build_time 1080

test_section "This should pass due to relative deviation being below 10% threshold"
# (1080/1050 - 1) * 100% = 2.9% < 10%, so it should pass
assert_success git perf audit -m build_time

test_section "Test global threshold with a different measurement"
assert_success create_commit
assert_success git perf add -m memory_usage 100
assert_success create_commit
assert_success git perf add -m memory_usage 105
assert_success create_commit
assert_success git perf add -m memory_usage 110

test_section "Add a measurement that would pass due to global threshold"
assert_success create_commit
assert_success git perf add -m memory_usage 112

test_section "This should fail due to relative deviation being above 5% global threshold"
# (112/105 - 1) * 100% = 6.7% > 5%, so it should fail
assert_failure git perf audit -m memory_usage

test_section "Test with a measurement that has lower relative deviation"
assert_success create_commit
assert_success git perf add -m memory_usage 107

test_section "This should pass due to relative deviation being below 5% global threshold"
# (107/105 - 1) * 100% = 1.9% < 5%, so it should pass
assert_success git perf audit -m memory_usage

test_stats
exit 0
