#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Verify that providing --template with CSV output shows a warning

cd_empty_repo

# Create a commit with measurements
create_commit
git perf add -m timer 1.0

# Create a dummy template file
template_file=$(mktemp)
echo "<html><body>{{PLOTLY_BODY}}</body></html>" > "$template_file"

# Test 1: CSV file with template should warn
csv_output=$(mktemp --suffix=.csv)
warn_output=$(git perf report -m timer -o "$csv_output" --template "$template_file" 2>&1 || true)
assert_output_contains "$warn_output" "Template argument is ignored for CSV output format" "Expected warning about template being ignored for CSV output"

# Test 2: Stdout (CSV) with template should warn
stdout_output=$(git perf report -m timer -o - --template "$template_file" 2>&1 || true)
assert_output_contains "$stdout_output" "Template argument is ignored for CSV output format" "Expected warning about template being ignored for stdout (CSV) output"

# Test 3: HTML file with template should NOT warn (legitimate use case)
html_output=$(mktemp --suffix=.html)
html_warn_output=$(git perf report -m timer -o "$html_output" --template "$template_file" 2>&1 || true)
assert_output_not_contains "$html_warn_output" "Template argument is ignored" "HTML output should NOT warn about template usage"

# Clean up
rm -f "$template_file" "$csv_output" "$html_output"

echo "CSV template warning test passed."
