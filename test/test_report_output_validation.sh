#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd_empty_repo

# Create test data with known values
create_commit
commit1=$(git rev-parse HEAD)
git perf add -m timer 1.5 -k os=ubuntu
git perf add -m timer 2.5 -k os=ubuntu

create_commit
commit2=$(git rev-parse HEAD)
git perf add -m timer 3.0 -k os=ubuntu
git perf add -m timer 4.0 -k os=ubuntu

create_commit
commit3=$(git rev-parse HEAD)
git perf add -m timer 5.0 -k os=mac
git perf add -m timer 6.0 -k os=mac

test_section "CSV output contains correct measurement values"
csv_output=$(git perf report -m timer -o - | grep -v '^[[:space:]]*$')

# Verify we have header + data lines (should be 1 header + 6 measurements = 7 lines)
line_count=$(echo "$csv_output" | grep -v '^[[:space:]]*$' | wc -l)
assert_equals "$line_count" "7" "Expected 7 lines in CSV (1 header + 6 measurements), got: $line_count"

# Verify specific values appear in output
# Normalize whitespace (convert tabs/spaces to single space) to avoid whitespace sensitivity
csv_normalized=$(echo "$csv_output" | tr -s '[:space:]' ' ')
assert_contains "$csv_normalized" " 1.5 " "Missing measurement value 1.5"
assert_contains "$csv_normalized" " 2.5 " "Missing measurement value 2.5"
assert_contains "$csv_normalized" " 3.0 " "Missing measurement value 3.0"
assert_contains "$csv_normalized" " 4.0 " "Missing measurement value 4.0"
assert_contains "$csv_normalized" " 5.0 " "Missing measurement value 5.0"
assert_contains "$csv_normalized" " 6.0 " "Missing measurement value 6.0"

# Verify measurement name appears in all lines
timer_count=$(echo "$csv_output" | grep -c "timer")
assert_equals "$timer_count" "6" "Expected 'timer' to appear 6 times, got: $timer_count"

test_section "CSV aggregation produces correct calculated values"
# Mean of [1.5, 2.5] = 2.0
# Mean of [3.0, 4.0] = 3.5
# Mean of [5.0, 6.0] = 5.5
agg_output=$(git perf report -m timer -a mean -o - | grep -v '^[[:space:]]*$')

line_count=$(echo "$agg_output" | grep -v '^[[:space:]]*$' | wc -l)
assert_equals "$line_count" "4" "Expected 4 lines (1 header + 3 aggregated), got: $line_count"

# Check for the mean values with whitespace normalization
agg_normalized=$(echo "$agg_output" | tr -s '[:space:]' ' ')
assert_contains "$agg_normalized" " 2.0" "Missing aggregated value 2.0 (mean of 1.5, 2.5)"
assert_contains "$agg_normalized" " 3.5" "Missing aggregated value 3.5 (mean of 3.0, 4.0)"
assert_contains "$agg_normalized" " 5.5" "Missing aggregated value 5.5 (mean of 5.0, 6.0)"

test_section "CSV output with key-value filtering"
# Filter to only ubuntu measurements
ubuntu_output=$(git perf report -m timer -k os=ubuntu -o - | grep -v '^[[:space:]]*$')

# Should have 1 header + 4 ubuntu measurements (2 per commit for first 2 commits) = 5 lines
line_count=$(echo "$ubuntu_output" | grep -v '^[[:space:]]*$' | wc -l)
assert_equals "$line_count" "5" "Expected 5 lines (1 header + 4 measurements), got: $line_count"

# Verify ubuntu values are present with whitespace normalization
ubuntu_normalized=$(echo "$ubuntu_output" | tr -s '[:space:]' ' ')
assert_contains "$ubuntu_normalized" " 1.5 " "Missing ubuntu measurement 1.5"
assert_contains "$ubuntu_normalized" " 2.5 " "Missing ubuntu measurement 2.5"
assert_contains "$ubuntu_normalized" " 3.0 " "Missing ubuntu measurement 3.0"
assert_contains "$ubuntu_normalized" " 4.0 " "Missing ubuntu measurement 4.0"

# Verify mac values are NOT present
assert_not_contains "$ubuntu_normalized" " 5.0 " "Mac measurement 5.0 should not appear in ubuntu filter"
assert_not_contains "$ubuntu_normalized" " 6.0 " "Mac measurement 6.0 should not appear in ubuntu filter"

test_section "HTML output contains measurement data"
git perf report -m timer -o test_output.html

# Verify HTML file was created
assert_file_exists "test_output.html" "HTML output file was not created"

# Verify HTML contains our measurement values
html_content=$(cat test_output.html)
assert_contains "$html_content" "1.5" "HTML missing measurement value 1.5"
assert_contains "$html_content" "2.5" "HTML missing measurement value 2.5"
assert_contains "$html_content" "3.0" "HTML missing measurement value 3.0"
assert_contains "$html_content" "4.0" "HTML missing measurement value 4.0"
assert_contains "$html_content" "5.0" "HTML missing measurement value 5.0"
assert_contains "$html_content" "6.0" "HTML missing measurement value 6.0"

# Verify HTML contains measurement name
assert_contains "$html_content" "timer" "HTML missing measurement name 'timer'"

test_section "HTML output with separation by key"
git perf report -m timer -s os -o separated_output.html

separated_content=$(cat separated_output.html)

# Should contain both ubuntu and mac as trace labels or groupings
assert_contains "$separated_content" "ubuntu" "HTML missing 'ubuntu' separation group"
assert_contains "$separated_content" "mac" "HTML missing 'mac' separation group"

# Should still contain all the values
assert_contains "$separated_content" "1.5" "Separated HTML missing value 1.5"
assert_contains "$separated_content" "5.0" "Separated HTML missing value 5.0"

test_section "Multiple measurements in same report"
# Add a second measurement type
git perf add -m memory 100 -k os=ubuntu
git perf add -m memory 200 -k os=mac

multi_output=$(git perf report -o -)

# Should contain both measurement types
assert_contains "$multi_output" "timer" "Multi-measurement CSV missing 'timer'"
assert_contains "$multi_output" "memory" "Multi-measurement CSV missing 'memory'"

# Should contain values from both measurement types with whitespace normalization
multi_normalized=$(echo "$multi_output" | tr -s '[:space:]' ' ')
assert_contains "$multi_normalized" " 1.5 " "Multi-measurement CSV missing timer value"
assert_contains "$multi_normalized" " 100.0 " "Multi-measurement CSV missing memory value"
assert_contains "$multi_normalized" " 200.0 " "Multi-measurement CSV missing memory value"

test_section "CSV output format validation"
# Verify CSV has proper structure (commit info and measurements)
csv_sample=$(git perf report -m timer -o - | head -1)

# CSV should contain commit hash (first 7+ chars of commit SHA)
short_commit=$(echo "$commit1" | cut -c1-7)
full_output=$(git perf report -m timer -o -)
assert_contains "$full_output" "$short_commit" "CSV missing commit hash"

test_stats
popd
exit 0
