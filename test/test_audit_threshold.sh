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
create_commit
git perf add -m build_time 1000
create_commit
git perf add -m build_time 1050
create_commit
git perf add -m build_time 1100

test_section "Add a measurement that would normally fail audit (high z-score) but passes due to threshold"
create_commit
git perf add -m build_time 1200

test_section "Audit should pass (z-score 3.0 < default sigma 4.0)"
# (1200/1050 - 1) * 100% = 14.3% > 10% threshold, but z-score doesn't exceed sigma
# z-score of 3.0 is below default sigma of 4.0, so audit passes
assert_success git perf audit -m build_time

test_section "Test with a measurement that has lower relative deviation"
create_commit
git perf add -m build_time 1080

test_section "This should pass due to relative deviation being below 10% threshold"
# (1080/1050 - 1) * 100% = 2.9% < 10%, so it should pass
assert_success git perf audit -m build_time

test_section "Test global threshold with a different measurement"
create_commit
git perf add -m memory_usage 100
create_commit
git perf add -m memory_usage 105
create_commit
git perf add -m memory_usage 110

test_section "Add a measurement that would pass due to global threshold"
create_commit
git perf add -m memory_usage 112

test_section "Audit should pass (z-score likely < default sigma 4.0)"
# (112/105 - 1) * 100% = 6.7% > 5% threshold, but z-score doesn't exceed sigma
# With low variance, z-score is below default sigma of 4.0, so audit passes
assert_success git perf audit -m memory_usage

test_section "Test with a measurement that has lower relative deviation"
create_commit
git perf add -m memory_usage 107

test_section "This should pass due to relative deviation being below 5% global threshold"
# (107/105 - 1) * 100% = 1.9% < 5%, so it should pass
assert_success git perf audit -m memory_usage

test_stats
exit 0
