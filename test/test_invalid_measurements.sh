#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)

# shellcheck source=test/common.sh
source "$script_dir/common.sh"

epoch=42

test_section "Add invalid measurements"

test_section "Empty measurement"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "\n"
assert_success_with_output output git perf report
assert_contains "$output" "too few items" "Missing 'too few items' in output"

test_section "Measurement with just date"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$(date +%s)"
assert_success_with_output output git perf report
assert_contains "$output" "too few items" "Missing 'too few items' in output"

test_section "Measurement without date"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$RANDOMkey=value"
assert_success_with_output output git perf report
assert_contains "$output" "skipping" "Missing 'skipping' in output"

test_section "Measurement without kvs"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOM"
assert_success_with_output output git perf report
# Expect a warning about too few items being skipped
assert_contains "$output" "skipping" "Expected warning about skipping record with too few items"

test_section "Measurement with invalid kvs"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMtestotherteststuff"
assert_success_with_output output git perf report
assert_not_equals "$output" "" "There should be output in stderr"

test_section "Measurement valid but with too many separators"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=value"
assert_success_with_output output git perf report
# Expect a warning about too few items due to empty separators creating invalid structure
assert_contains "$output" "skipping" "Expected warning about invalid measurement structure"

test_section "Duplicate kvs"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=valuekey=valuekey=valuekey=value"
assert_success_with_output output git perf report
assert_contains "$output" "Duplicate entries for key key with same value" "Expected warning about 'Duplicate entries for key key with same value' in the output"

# Verify warning is only printed once
warning_count=$(echo "$output" | grep -c "Duplicate entries for key key with same value")
assert_equals "$warning_count" "1" "Expected warning to appear exactly once"

test_section "Conflicting kvs"
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=valuekey=value2"
assert_success_with_output output git perf report
assert_contains "$output" "Conflicting values" "Expected warning about 'Conflicting values' in the output"

test_stats
exit 0
