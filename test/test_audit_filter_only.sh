#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Test audit with filter-only (no -m required)"

# This test demonstrates the new feature where --filter can be used without -m
# Previously, -m was always required, forcing users to provide dummy values

cd_empty_repo

# Create commits with various measurement types
create_commit
git perf add -m perf_cpu 100
git perf add -m perf_memory 200
git perf add -m bench_throughput 50
git perf add -m bench_latency 25
git perf add -m test_coverage 85

create_commit
git perf add -m perf_cpu 105
git perf add -m perf_memory 210
git perf add -m bench_throughput 52
git perf add -m bench_latency 26
git perf add -m test_coverage 86

create_commit
git perf add -m perf_cpu 110
git perf add -m perf_memory 220
git perf add -m bench_throughput 54
git perf add -m bench_latency 27
git perf add -m test_coverage 87

create_commit
git perf add -m perf_cpu 115
git perf add -m perf_memory 230
git perf add -m bench_throughput 56
git perf add -m bench_latency 28
git perf add -m test_coverage 88

echo "Test 1: Audit all measurements with 'perf_' prefix using filter-only"
output=$(git perf audit --filter "^perf_" -d 10 2>&1)
assert_output_contains "$output" "perf_cpu" "Should audit perf_cpu"
assert_output_contains "$output" "perf_memory" "Should audit perf_memory"
if echo "$output" | grep -q "bench_"; then
  echo "FAIL: bench_ measurements should not be audited"
  exit 1
fi
if echo "$output" | grep -q "test_"; then
  echo "FAIL: test_ measurements should not be audited"
  exit 1
fi
echo "PASS: Filter-only successfully audited perf_* measurements"

echo "Test 2: Audit all measurements with 'bench_' prefix using filter-only"
output=$(git perf audit --filter "^bench_" -d 10 2>&1)
assert_output_contains "$output" "bench_throughput" "Should audit bench_throughput"
assert_output_contains "$output" "bench_latency" "Should audit bench_latency"
if echo "$output" | grep -q "perf_"; then
  echo "FAIL: perf_ measurements should not be audited"
  exit 1
fi
echo "PASS: Filter-only successfully audited bench_* measurements"

echo "Test 3: Audit with unanchored pattern (matches substring)"
output=$(git perf audit --filter "through" -d 10 2>&1)
assert_output_contains "$output" "bench_throughput" "Should match measurements containing 'through'"
echo "PASS: Unanchored pattern works correctly"

echo "Test 4: Error when neither -m nor --filter provided"
if output=$(git perf audit -d 10 2>&1); then
  echo "FAIL: Should error when neither -m nor --filter provided"
  exit 1
fi
assert_output_contains "$output" "required arguments" "Should indicate missing required arguments"
echo "PASS: Correctly errors when both -m and --filter are missing"

echo "All filter-only tests passed successfully"
exit 0
