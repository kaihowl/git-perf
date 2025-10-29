#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test audit with --filter argument"

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

echo "Test 1: Filter with regex pattern matching bench_* measurements (with dummy -m)"
# Note: CLI currently requires -m, but filter can expand the set
# Use a non-existent -m to test pure filter behavior
output=$(git perf audit -m nonexistent --filter "bench_.*" -d 10 2>&1)
assert_output_contains "$output" "bench_cpu" "Should audit bench_cpu"
assert_output_contains "$output" "bench_memory" "Should audit bench_memory"
if echo "$output" | grep -q "test_unit"; then
  echo "FAIL: test_unit should not be audited with bench_.* filter"
  exit 1
fi
if echo "$output" | grep -q "'timer'"; then
  echo "FAIL: timer should not be audited with bench_.* filter"
  exit 1
fi
echo "PASS: Filter correctly matched bench_* measurements"

echo "Test 2: Combine -m and --filter (OR behavior)"
output=$(git perf audit -m timer --filter "bench_cpu" -d 10 2>&1)
assert_output_contains "$output" "timer" "Should audit explicit measurement 'timer'"
assert_output_contains "$output" "bench_cpu" "Should audit filtered measurement 'bench_cpu'"
if echo "$output" | grep -q "bench_memory"; then
  echo "FAIL: bench_memory should not be audited (not in -m or filter)"
  exit 1
fi
if echo "$output" | grep -q "test_unit"; then
  echo "FAIL: test_unit should not be audited (not in -m or filter)"
  exit 1
fi
echo "PASS: Combined -m and --filter work with OR behavior"

echo "Test 3: Filter matches no measurements (should error)"
if output=$(git perf audit -m nonexistent_dummy --filter "nonexistent.*" -d 10 2>&1); then
  echo "FAIL: Should fail when filter matches no measurements"
  echo "Output: $output"
  exit 1
fi
assert_output_contains "$output" "No measurements found matching the provided patterns" "Should error with appropriate message"
echo "PASS: Correctly errors when filter matches nothing"

echo "Test 4: Invalid regex pattern (should error)"
if output=$(git perf audit -m dummy --filter "[invalid" -d 10 2>&1); then
  echo "FAIL: Should fail with invalid regex pattern"
  echo "Output: $output"
  exit 1
fi
assert_output_contains "$output" "Invalid regex pattern" "Should error about invalid regex"
echo "PASS: Correctly errors on invalid regex"

echo "Test 5: Filter with selectors"
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

output=$(git perf audit -m dummy --filter "bench_.*" -s os=linux -d 10 2>&1)
assert_output_contains "$output" "bench_cpu" "Should audit bench_cpu with os=linux"
if echo "$output" | grep -q "test_unit"; then
  echo "FAIL: test_unit should not match bench_.* filter"
  exit 1
fi
echo "PASS: Filter works correctly with selectors"

echo "Test 6: Multiple filter patterns (OR behavior)"
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

output=$(git perf audit -m dummy --filter "bench_.*" --filter "test_.*" -d 10 2>&1)
assert_output_contains "$output" "bench_cpu" "Should audit bench_cpu"
assert_output_contains "$output" "test_unit" "Should audit test_unit"
if echo "$output" | grep -q "other_metric"; then
  echo "FAIL: other_metric should not match filters"
  exit 1
fi
echo "PASS: Multiple filter patterns work with OR behavior"

echo "Test 7: Filter with strict sigma to verify actual audit logic runs"
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
if output=$(git perf audit -m dummy --filter "bench_.*" -d 0.5 2>&1); then
  echo "FAIL: Audit should fail with outlier value and strict sigma"
  echo "Output: $output"
  exit 1
fi
assert_output_contains "$output" "❌ 'bench_fast'" "Should show failure for bench_fast"
assert_output_contains "$output" "differs significantly" "Should indicate significant difference"
echo "PASS: Filter + audit logic correctly detects outliers"

echo "Test 8: Filter combined with explicit measurement (overlapping)"
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
assert_output_contains "$output" "✅ 'my_benchmark'" "Should successfully audit measurement"
# Should only appear once in output (not duplicated)
if [ "$(echo "$output" | grep -c "'my_benchmark'")" -gt 2 ]; then
  echo "FAIL: Measurement should not be duplicated when specified in both -m and --filter"
  exit 1
fi
echo "PASS: Overlapping -m and --filter deduplicate correctly"

echo "All audit filter tests passed successfully"
exit 0
