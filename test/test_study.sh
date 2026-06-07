#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "study: grouped measurements produce recommendations"
cd_empty_repo
create_commit
# Simulate 10 independent runner instances each contributing 1 measurement
# with a group key. Values are close together (low CoV ≈ 2-3%).
for i in $(seq 1 10); do
  git perf add -m bench::my_op --key-value group="$i" $((100 + i))
done

assert_success_with_output output git perf study -m bench::my_op
assert_contains "$output" "Between-group CoV"
assert_contains "$output" "dispersion_method"
assert_contains "$output" "sigma"
assert_contains "$output" "min_relative_deviation"
assert_contains "$output" "max_cov"
assert_contains "$output" "[measurement.\"bench::my_op\"]"

test_section "study: fallback to raw CoV when no group key"
cd_empty_repo
create_commit
for i in $(seq 1 5); do
  git perf add -m raw_op $((200 + i * 5))
done

assert_success_with_output output git perf study -m raw_op
assert_contains "$output" "Overall CoV (no group key found)"
assert_contains "$output" "dispersion_method"

test_section "study: --max-cov passes when CoV is low"
cd_empty_repo
create_commit
# Very tight data: all values within 1% of each other → low CoV
for i in $(seq 1 5); do
  git perf add -m stable_op --key-value group="$i" 1000
done

assert_success git perf study -m stable_op --max-cov 50

test_section "study: --max-cov fails when CoV exceeds threshold"
cd_empty_repo
create_commit
# Wide spread: values range 50..150 → CoV ≈ 25%+ → should fail at threshold 5
for i in 50 75 100 125 150; do
  git perf add -m noisy_op --key-value group="$i" "$i"
done

assert_failure git perf study -m noisy_op --max-cov 5

test_section "study: fails with clear message when no measurements"
cd_empty_repo
create_commit

assert_failure_with_output output git perf study -m nonexistent
assert_contains "$output" "No measurements found"

test_section "study: fails with clear message when too few groups"
cd_empty_repo
create_commit
# Only 2 groups: below the 3-group minimum
git perf add -m few_op --key-value group=1 100
git perf add -m few_op --key-value group=2 110

assert_failure_with_output output git perf study -m few_op
assert_contains "$output" "at least 3"
