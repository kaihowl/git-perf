#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test audit with --filter argument"

# Setup: Create a repo with multiple measurement types
cd_empty_repo

# Create commits with different measurement types
# Commit 1 (HEAD~3)
create_commit
git perf add -m bench_cpu 100
git perf add -m bench_memory 200
git perf add -m test_unit 50
git perf add -m timer 10

# Commit 2 (HEAD~2)
create_commit
git perf add -m bench_cpu 105
git perf add -m bench_memory 210
git perf add -m test_unit 52
git perf add -m timer 11

# Commit 3 (HEAD~1)
create_commit
git perf add -m bench_cpu 110
git perf add -m bench_memory 220
git perf add -m test_unit 54
git perf add -m timer 12

# Commit 4 (HEAD)
create_commit
git perf add -m bench_cpu 115
git perf add -m bench_memory 230
git perf add -m test_unit 56
git perf add -m timer 13

test_section "Test 1: Filter with regex pattern matching bench_* measurements"
# Test pure filter behavior without requiring -m
output=$(git perf audit --filter "bench_.*" -d 10 2>&1)
assert_contains "$output" "bench_cpu" "Should audit bench_cpu"
assert_contains "$output" "bench_memory" "Should audit bench_memory"
if echo "$output" | grep -q "test_unit"; then
  test_section "FAIL: test_unit should not be audited with bench_.* filter"
  exit 1
fi
if echo "$output" | grep -q "'timer'"; then
  test_section "FAIL: timer should not be audited with bench_.* filter"
  exit 1
fi
test_section "PASS: Filter correctly matched bench_* measurements"

test_section "Test 2: Combine -m and --filter (OR behavior)"
output=$(git perf audit -m timer --filter "bench_cpu" -d 10 2>&1)
assert_contains "$output" "timer" "Should audit explicit measurement 'timer'"
assert_contains "$output" "bench_cpu" "Should audit filtered measurement 'bench_cpu'"
if echo "$output" | grep -q "bench_memory"; then
  test_section "FAIL: bench_memory should not be audited (not in -m or filter)"
  exit 1
fi
if echo "$output" | grep -q "test_unit"; then
  test_section "FAIL: test_unit should not be audited (not in -m or filter)"
  exit 1
fi
test_section "PASS: Combined -m and --filter work with OR behavior"

test_section "Test 3: Filter matches no measurements (should error)"
if output=$(git perf audit --filter "nonexistent.*" -d 10 2>&1); then
  test_section "FAIL: Should fail when filter matches no measurements"
  test_section "Output: $output"
  exit 1
fi
assert_contains "$output" "No measurements found matching the provided patterns" "Should error with appropriate message"
test_section "PASS: Correctly errors when filter matches nothing"

test_section "Test 4: Invalid regex pattern (should error)"
if output=$(git perf audit --filter "[invalid" -d 10 2>&1); then
  test_section "FAIL: Should fail with invalid regex pattern"
  test_section "Output: $output"
  exit 1
fi
assert_contains "$output" "Invalid regex pattern" "Should error about invalid regex"
test_section "PASS: Correctly errors on invalid regex"

test_section "Test 5: Filter with selectors"
# Add measurements with selectors
cd_empty_repo
create_commit
git perf add -m bench_cpu 100 -k os=linux
git perf add -m bench_cpu 150 -k os=mac
git perf add -m test_unit 50 -k os=linux

create_commit
git perf add -m bench_cpu 105 -k os=linux
git perf add -m bench_cpu 155 -k os=mac
git perf add -m test_unit 52 -k os=linux

create_commit
git perf add -m bench_cpu 110 -k os=linux
git perf add -m bench_cpu 160 -k os=mac
git perf add -m test_unit 54 -k os=linux

create_commit
git perf add -m bench_cpu 115 -k os=linux
git perf add -m bench_cpu 165 -k os=mac
git perf add -m test_unit 56 -k os=linux

output=$(git perf audit --filter "bench_.*" -s os=linux -d 10 2>&1)
assert_contains "$output" "bench_cpu" "Should audit bench_cpu with os=linux"
if echo "$output" | grep -q "test_unit"; then
  test_section "FAIL: test_unit should not match bench_.* filter"
  exit 1
fi
test_section "PASS: Filter works correctly with selectors"

test_section "Test 6: Multiple filter patterns (OR behavior)"
cd_empty_repo
create_commit
git perf add -m bench_cpu 100
git perf add -m test_unit 50
git perf add -m other_metric 25

create_commit
git perf add -m bench_cpu 105
git perf add -m test_unit 52
git perf add -m other_metric 26

create_commit
git perf add -m bench_cpu 110
git perf add -m test_unit 54
git perf add -m other_metric 27

create_commit
git perf add -m bench_cpu 115
git perf add -m test_unit 56
git perf add -m other_metric 28

output=$(git perf audit --filter "bench_.*" --filter "test_.*" -d 10 2>&1)
assert_contains "$output" "bench_cpu" "Should audit bench_cpu"
assert_contains "$output" "test_unit" "Should audit test_unit"
if echo "$output" | grep -q "other_metric"; then
  test_section "FAIL: other_metric should not match filters"
  exit 1
fi
test_section "PASS: Multiple filter patterns work with OR behavior"

test_section "Test 7: Filter with strict sigma to verify actual audit logic runs"
cd_empty_repo
create_commit
git perf add -m bench_fast 10

create_commit
git perf add -m bench_fast 11

create_commit
git perf add -m bench_fast 12

# HEAD with outlier value
create_commit
git perf add -m bench_fast 100

# Should fail with low sigma
if output=$(git perf audit --filter "bench_.*" -d 0.5 2>&1); then
  test_section "FAIL: Audit should fail with outlier value and strict sigma"
  test_section "Output: $output"
  exit 1
fi
assert_contains "$output" "❌ 'bench_fast'" "Should show failure for bench_fast"
assert_contains "$output" "differs significantly" "Should indicate significant difference"
test_section "PASS: Filter + audit logic correctly detects outliers"

test_section "Test 8: Filter combined with explicit measurement (overlapping)"
cd_empty_repo
create_commit
git perf add -m my_benchmark 100

create_commit
git perf add -m my_benchmark 102

create_commit
git perf add -m my_benchmark 104

create_commit
git perf add -m my_benchmark 106

# Both -m and --filter specify the same measurement (dedup behavior)
output=$(git perf audit -m my_benchmark --filter "my_.*" -d 10 2>&1)
assert_contains "$output" "✅ 'my_benchmark'" "Should successfully audit measurement"
# Should only appear once in output (not duplicated)
if [ "$(echo "$output" | grep -c "'my_benchmark'")" -gt 2 ]; then
  test_section "FAIL: Measurement should not be duplicated when specified in both -m and --filter"
  exit 1
fi
test_section "PASS: Overlapping -m and --filter deduplicate correctly"

test_section "All audit filter tests passed successfully"
test_stats
exit 0
