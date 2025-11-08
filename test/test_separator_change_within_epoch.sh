#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Test case for issue #100: What happens if separators change inside the same epoch?
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

# Generate report with separator that exists in some commits but not others
# Current behavior: measurements without the separator key are silently excluded
# Issue #100 asks: should there be a default bucket for measurements without the separator key?
git perf report -o separated_result.html -s os

# Verify the report was created (it should succeed because at least some measurements have the 'os' key)
if [[ ! -f "separated_result.html" ]]; then
  echo "Expected HTML file 'separated_result.html' was not created"
  exit 1
fi

# Check that separated report contains measurements with the separator
separated_content=$(cat separated_result.html)
assert_output_contains "$separated_content" "ubuntu" "Separated HTML missing 'ubuntu' group label"
assert_output_contains "$separated_content" "timer" "Separated HTML missing 'timer' measurement name"

# Verify that we can generate a report without the separator (should include all measurements)
git perf report -o all_result.html
if [[ ! -f "all_result.html" ]]; then
  echo "Expected HTML file 'all_result.html' was not created"
  exit 1
fi

# Verify the unseparated report contains all measurements
html_content=$(cat all_result.html)
assert_output_contains "$html_content" "timer" "HTML file missing measurement name 'timer'"

# Test the opposite scenario: separator key that doesn't exist in any measurements
output=$(git perf report -s does-not-exist 2>&1 1>/dev/null) && exit 1
assert_output_contains "$output" "invalid separator" "No error for separator key that doesn't exist in any measurements"

echo "Test passed: Separator change within epoch behaves as documented"
echo "Note: Measurements without the separator key are silently excluded from separated reports"
echo "This is the behavior questioned in issue #100"
