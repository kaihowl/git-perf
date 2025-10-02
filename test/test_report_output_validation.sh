#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Test fine-grained report output validation
# This addresses the TODO at reporting.rs:354 about needing more detailed e2e output tests

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

echo "=== Test 1: CSV output contains correct measurement values ==="
csv_output=$(git perf report -m timer -o - | grep -v '^[[:space:]]*$')

# Verify we have data lines (should be 6 measurements)
line_count=$(echo "$csv_output" | grep -v '^[[:space:]]*$' | wc -l)
if [[ "$line_count" -ne 6 ]]; then
  echo "Expected 6 measurement lines in CSV, got: $line_count"
  echo "Output: $csv_output"
  exit 1
fi

# Verify specific values appear in output
# Normalize whitespace (convert tabs/spaces to single space) to avoid whitespace sensitivity
csv_normalized=$(echo "$csv_output" | tr -s '[:space:]' ' ')
assert_output_contains "$csv_normalized" " 1.5 " "Missing measurement value 1.5"
assert_output_contains "$csv_normalized" " 2.5 " "Missing measurement value 2.5"
assert_output_contains "$csv_normalized" " 3.0 " "Missing measurement value 3.0"
assert_output_contains "$csv_normalized" " 4.0 " "Missing measurement value 4.0"
assert_output_contains "$csv_normalized" " 5.0 " "Missing measurement value 5.0"
assert_output_contains "$csv_normalized" " 6.0 " "Missing measurement value 6.0"

# Verify measurement name appears in all lines
timer_count=$(echo "$csv_output" | grep -c "timer")
if [[ "$timer_count" -ne 6 ]]; then
  echo "Expected 'timer' to appear 6 times, got: $timer_count"
  exit 1
fi

echo "✓ CSV output contains all expected measurement values"

echo "=== Test 2: CSV aggregation produces correct calculated values ==="
# Mean of [1.5, 2.5] = 2.0
# Mean of [3.0, 4.0] = 3.5
# Mean of [5.0, 6.0] = 5.5
agg_output=$(git perf report -m timer -a mean -o - | grep -v '^[[:space:]]*$')

line_count=$(echo "$agg_output" | grep -v '^[[:space:]]*$' | wc -l)
if [[ "$line_count" -ne 3 ]]; then
  echo "Expected 3 aggregated lines (one per commit), got: $line_count"
  echo "Output: $agg_output"
  exit 1
fi

# Check for the mean values with whitespace normalization
agg_normalized=$(echo "$agg_output" | tr -s '[:space:]' ' ')
assert_output_contains "$agg_normalized" " 2.0" "Missing aggregated value 2.0 (mean of 1.5, 2.5)"
assert_output_contains "$agg_normalized" " 3.5" "Missing aggregated value 3.5 (mean of 3.0, 4.0)"
assert_output_contains "$agg_normalized" " 5.5" "Missing aggregated value 5.5 (mean of 5.0, 6.0)"

echo "✓ CSV aggregation produces correct mean values"

echo "=== Test 3: CSV output with key-value filtering ==="
# Filter to only ubuntu measurements
ubuntu_output=$(git perf report -m timer -k os=ubuntu -o - | grep -v '^[[:space:]]*$')

# Should have 4 ubuntu measurements (2 per commit for first 2 commits)
line_count=$(echo "$ubuntu_output" | grep -v '^[[:space:]]*$' | wc -l)
if [[ "$line_count" -ne 4 ]]; then
  echo "Expected 4 ubuntu measurements, got: $line_count"
  echo "Output: $ubuntu_output"
  exit 1
fi

# Verify ubuntu values are present with whitespace normalization
ubuntu_normalized=$(echo "$ubuntu_output" | tr -s '[:space:]' ' ')
assert_output_contains "$ubuntu_normalized" " 1.5 " "Missing ubuntu measurement 1.5"
assert_output_contains "$ubuntu_normalized" " 2.5 " "Missing ubuntu measurement 2.5"
assert_output_contains "$ubuntu_normalized" " 3.0 " "Missing ubuntu measurement 3.0"
assert_output_contains "$ubuntu_normalized" " 4.0 " "Missing ubuntu measurement 4.0"

# Verify mac values are NOT present
assert_output_not_contains "$ubuntu_normalized" " 5.0 " "Mac measurement 5.0 should not appear in ubuntu filter"
assert_output_not_contains "$ubuntu_normalized" " 6.0 " "Mac measurement 6.0 should not appear in ubuntu filter"

echo "✓ Key-value filtering correctly filters measurements"

echo "=== Test 4: HTML output contains measurement data ==="
git perf report -m timer -o test_output.html

# Verify HTML file was created
if [[ ! -f test_output.html ]]; then
  echo "HTML output file was not created"
  exit 1
fi

# Verify HTML contains our measurement values
html_content=$(cat test_output.html)
assert_output_contains "$html_content" "1.5" "HTML missing measurement value 1.5"
assert_output_contains "$html_content" "2.5" "HTML missing measurement value 2.5"
assert_output_contains "$html_content" "3.0" "HTML missing measurement value 3.0"
assert_output_contains "$html_content" "4.0" "HTML missing measurement value 4.0"
assert_output_contains "$html_content" "5.0" "HTML missing measurement value 5.0"
assert_output_contains "$html_content" "6.0" "HTML missing measurement value 6.0"

# Verify HTML contains measurement name
assert_output_contains "$html_content" "timer" "HTML missing measurement name 'timer'"

echo "✓ HTML output contains expected measurement data"

echo "=== Test 5: HTML output with separation by key ==="
git perf report -m timer -s os -o separated_output.html

separated_content=$(cat separated_output.html)

# Should contain both ubuntu and mac as trace labels or groupings
assert_output_contains "$separated_content" "ubuntu" "HTML missing 'ubuntu' separation group"
assert_output_contains "$separated_content" "mac" "HTML missing 'mac' separation group"

# Should still contain all the values
assert_output_contains "$separated_content" "1.5" "Separated HTML missing value 1.5"
assert_output_contains "$separated_content" "5.0" "Separated HTML missing value 5.0"

echo "✓ HTML separation by key includes group labels and data"

echo "=== Test 6: Multiple measurements in same report ==="
# Add a second measurement type
git perf add -m memory 100 -k os=ubuntu
git perf add -m memory 200 -k os=mac

multi_output=$(git perf report -o -)

# Should contain both measurement types
assert_output_contains "$multi_output" "timer" "Multi-measurement CSV missing 'timer'"
assert_output_contains "$multi_output" "memory" "Multi-measurement CSV missing 'memory'"

# Should contain values from both measurement types with whitespace normalization
multi_normalized=$(echo "$multi_output" | tr -s '[:space:]' ' ')
assert_output_contains "$multi_normalized" " 1.5 " "Multi-measurement CSV missing timer value"
assert_output_contains "$multi_normalized" " 100 " "Multi-measurement CSV missing memory value"
assert_output_contains "$multi_normalized" " 200 " "Multi-measurement CSV missing memory value"

echo "✓ Multiple measurement types appear in same report"

echo "=== Test 7: CSV output format validation ==="
# Verify CSV has proper structure (commit info and measurements)
csv_sample=$(git perf report -m timer -o - | head -1)

# CSV should contain commit hash (first 7+ chars of commit SHA)
short_commit=$(echo "$commit1" | cut -c1-7)
full_output=$(git perf report -m timer -o -)
assert_output_contains "$full_output" "$short_commit" "CSV missing commit hash"

echo "✓ CSV output includes commit information"

echo ""
echo "All report output validation tests passed!"
echo "These tests verify:"
echo "  - CSV contains correct measurement values"
echo "  - Aggregation calculates correct mean values"
echo "  - Key-value filtering works correctly"
echo "  - HTML output contains measurement data"
echo "  - Separation by key works in HTML output"
echo "  - Multiple measurement types in same report"
echo "  - CSV includes commit information"

popd
