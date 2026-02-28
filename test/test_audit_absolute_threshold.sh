#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test the minimum absolute deviation threshold feature"
cd_empty_repo

test_section "Create a config file with absolute threshold settings"
cat > .gitperfconfig << 'EOF'
[measurement]
min_absolute_deviation = 5.0

[measurement."build_time"]
min_absolute_deviation = 100.0
EOF

test_section "Create some commits with very low variance measurements"
create_commit
git perf add -m build_time 1000
create_commit
git perf add -m build_time 1001
create_commit
git perf add -m build_time 1002

test_section "Add a measurement that exceeds sigma but absolute deviation is below threshold"
create_commit
git perf add -m build_time 1050

test_section "Audit should pass (absolute deviation 48 < threshold 100)"
# |1050 - 1001| = 49 < 100, so it should pass despite possibly exceeding sigma
assert_success git perf audit -m build_time

test_section "Test with global threshold and a different measurement"
create_commit
git perf add -m latency 10.0
create_commit
git perf add -m latency 10.1
create_commit
git perf add -m latency 10.2

test_section "Add a measurement with small absolute deviation"
create_commit
git perf add -m latency 14.0

test_section "Audit should pass (absolute deviation 3.8 < global threshold 5.0)"
# |14.0 - 10.1| = 3.9 < 5.0, so it should pass
assert_success git perf audit -m latency

test_section "Test that large absolute deviation still fails"
create_commit
git perf add -m latency 10.0
create_commit
git perf add -m latency 10.1
create_commit
git perf add -m latency 10.2
create_commit
git perf add -m latency 10.0
create_commit
git perf add -m latency 10.1
create_commit
git perf add -m latency 100.0

test_section "Audit should fail (absolute deviation far above threshold)"
assert_failure git perf audit -m latency

test_stats
exit 0
