#!/bin/bash

set -e
set -x

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
num_lines=$(git perf report -m timer -a mean -o - | wc -l)

if [[ "$num_lines" -ne 2 ]]; then
  echo "Expected 2 summarized lines (one per commit), got: $num_lines"
  exit 1
fi

# Basic sanity: ensure measurement name appears on each line
name_count=$(git perf report -m timer -a mean -o - | grep -c "timer")
if [[ "$name_count" -ne 2 ]]; then
  echo "Expected measurement name 'timer' to appear on 2 lines, got: $name_count"
  exit 1
fi

echo "CSV aggregated report produced correct number of summarized lines."

