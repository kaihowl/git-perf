#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test audit with filter-only (no -m required)"

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

test_section "Audit all measurements with 'perf_' prefix using filter-only"
assert_success output git perf audit --filter "^perf_" -d 10
assert_contains "$output" "perf_cpu" "Should audit perf_cpu"
assert_contains "$output" "perf_memory" "Should audit perf_memory"
assert_not_contains "$output" "bench_"
assert_not_contains "$output" "test_"

test_section "Audit all measurements with 'bench_' prefix using filter-only"
assert_success output git perf audit --filter "^bench_" -d 10
assert_contains "$output" "bench_throughput" "Should audit bench_throughput"
assert_contains "$output" "bench_latency" "Should audit bench_latency"
assert_not_contains "$output" "perf_"

test_section "Audit with unanchored pattern (matches substring)"
assert_success output git perf audit --filter "through" -d 10
assert_contains "$output" "bench_throughput" "Should match measurements containing 'through'"

test_section "Error when neither -m nor --filter provided"
assert_failure output git perf audit -d 10
assert_contains "$output" "required arguments" "Should indicate missing required arguments"

test_stats
exit 0
