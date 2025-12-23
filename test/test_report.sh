#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Report command tests - basic functionality"

cd_empty_repo
create_commit
git perf add -m timer 1 -k os=ubuntu
git perf add -m timer 0.9 -k os=ubuntu
git perf add -m timer 1.2 -k os=mac
git perf add -m timer 1.1 -k os=mac
create_commit
git perf add -m timer 2.1 -k os=ubuntu
git perf add -m timer 2.2 -k os=ubuntu
git perf add -m timer 2.1 -k os=mac
git perf add -m timer 2.0 -k os=mac
create_commit
git perf add -m timer 3.1 -k os=ubuntu
git perf add -m timer 3.2 -k os=ubuntu
git perf add -m timer 3.3 -k os=mac
git perf add -m timer 3.4 -k os=mac
create_commit
git perf add -m timer 4 -k os=ubuntu
git perf add -m timer 4 -k os=ubuntu
git perf add -m timer 4.3 -k os=mac
git perf add -m timer 4.3 -k os=mac
git perf add -m timer2 2 -k os=mac

git perf report -o all_result.html
git perf report -o separated_result.html -s os
git perf report -o single_result.html -m timer
git perf report -o separated_single_result.html -m timer -s os

# Verify HTML files were created and contain measurement data
for html_file in all_result.html separated_result.html single_result.html separated_single_result.html; do
  assert_file_exists "$html_file" "Expected HTML file '$html_file' was not created"

  html_content=$(cat "$html_file")
  assert_contains "$html_content" "timer" "HTML file '$html_file' missing measurement name 'timer'"
done

# Verify separated output contains OS labels
separated_content=$(cat separated_result.html)
assert_contains "$separated_content" "ubuntu" "Separated HTML missing 'ubuntu' group label"
assert_contains "$separated_content" "mac" "Separated HTML missing 'mac' group label"

# Verify timer2 only appears in all_result (not filtered out)
all_content=$(cat all_result.html)
assert_contains "$all_content" "timer2" "All results HTML missing 'timer2' measurement"

# Verify timer2 is absent from filtered reports
single_content=$(cat single_result.html)
separated_single_content=$(cat separated_single_result.html)
assert_not_contains "$single_content" "timer2" "Single measurement HTML should not contain 'timer2'"
assert_not_contains "$separated_single_content" "timer2" "Separated single measurement HTML should not contain 'timer2'"

assert_failure output git perf report -m timer-does-not-exist
assert_contains "$output" "no performance measurements" "No warning for missing measurements"

assert_failure output git perf report -s does-not-exist
assert_contains "$output" "invalid separator" "No warning for invalid separator 'does-not-exist'"

test_stats
exit 0

