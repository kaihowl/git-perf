#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Test case for issue #100: Separator changes within epoch"

# This tests the scenario where measurements in earlier commits have a separator key,
# but later commits (within the same epoch) don't have that key.

cd_empty_repo
create_commit

# First commit: measurements with os separator
git perf add -m timer 1.0 -k os=ubuntu
git perf add -m timer 0.9 -k os=ubuntu

create_commit

# Second commit: measurements with os separator
git perf add -m timer 2.0 -k os=ubuntu
git perf add -m timer 1.9 -k os=ubuntu

create_commit

# Third commit: measurements WITHOUT os separator (same epoch)
# This simulates a change in measurement structure within the same epoch
# These measurements will be silently excluded from the separated report
git perf add -m timer 3.0
git perf add -m timer 2.9

create_commit

# Fourth commit: measurements WITHOUT os separator
git perf add -m timer 4.0
git perf add -m timer 3.9

test_section "Generate report with separator that exists in some commits"

# Current behavior: measurements without the separator key are silently excluded
# Issue #100 asks: should there be a default bucket for measurements without the separator key?
assert_success git perf report -o separated_result.html -s os

# Verify the report was created (it should succeed because at least some measurements have the 'os' key)
assert_file_exists "separated_result.html" "Expected HTML file 'separated_result.html' was not created"

# Check that separated report contains measurements with the separator
assert_success_with_output separated_content cat separated_result.html
assert_contains "$separated_content" "ubuntu" "Separated HTML missing 'ubuntu' group label"
assert_contains "$separated_content" "timer" "Separated HTML missing 'timer' measurement name"

test_section "Verify report without separator includes all measurements"

assert_success git perf report -o all_result.html
assert_file_exists "all_result.html" "Expected HTML file 'all_result.html' was not created"

# Verify the unseparated report contains all measurements
assert_success_with_output html_content cat all_result.html
assert_contains "$html_content" "timer" "HTML file missing measurement name 'timer'"

test_section "Test separator key that doesn't exist in any measurements"

assert_failure_with_output output git perf report -s does-not-exist
assert_contains "$output" "invalid separator" "No error for separator key that doesn't exist in any measurements"

test_stats
exit 0
