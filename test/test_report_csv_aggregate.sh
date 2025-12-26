#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Verify that CSV/stdout reporting with aggregation prints one summarized line per commit

cd_empty_repo

# Commit 1 with two measurements for the same metric
create_commit
git perf add -m timer 1.0
git perf add -m timer 2.0

# Commit 2 with two measurements for the same metric
create_commit
git perf add -m timer 3.0
git perf add -m timer 4.0

# Aggregate by mean and write to stdout (CSV/TSV)
agg_output=$(git perf report -m timer -a mean -o - | grep -v '^[[:space:]]*$')
num_lines=$(echo "$agg_output" | grep -v '^[[:space:]]*$' | wc -l)

if [[ "$num_lines" -ne 3 ]]; then
  test_section "Expected 3 lines (1 header + 2 summarized), got: $num_lines"
  test_section "Output: $agg_output"
  exit 1
fi

# Basic sanity: ensure measurement name appears on each data line + header
name_count=$(echo "$agg_output" | grep -c "timer")
if [[ "$name_count" -ne 2 ]]; then
  echo "Expected measurement name 'timer' to appear on 2 data lines, got: $name_count"
  exit 1
fi

# Verify the aggregated values are correct
# Commit 1: mean of [1.0, 2.0] = 1.5
# Commit 2: mean of [3.0, 4.0] = 3.5
# Normalize whitespace for reliable matching
agg_normalized=$(echo "$agg_output" | tr -s '[:space:]' ' ')
assert_contains "$agg_normalized" " 1.5" "CSV missing aggregated mean value 1.5 (mean of 1.0, 2.0)"
assert_contains "$agg_normalized" " 3.5" "CSV missing aggregated mean value 3.5 (mean of 3.0, 4.0)"

test_section "CSV aggregated report produced correct number of summarized lines and correct mean values."

