#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# ============================================================================
# Setup: A repo with 4 commits, all created "today" (no faketime needed)
# ============================================================================

test_section "Setup: repo with multiple commits"

cd_empty_repo

# Create 4 commits with measurements
create_commit; git perf add -m timer 10.0
create_commit; git perf add -m timer 10.0
create_commit; git perf add -m timer 10.0
create_commit; git perf add -m timer 10.0

# ============================================================================
# --since with an old date: should include all recent commits
# ============================================================================

test_section "--since with old date includes all recent commits"

# "1 year ago" should include all commits made today (4 data rows + 1 header = 5 lines)
assert_success_with_output output git perf report --since="1 year ago" -o -
lines=$(echo "$output" | wc -l | tr -d ' ')
assert_equals "$lines" "5" "Expected all 4 measurements + header with --since=1 year ago"

# ============================================================================
# --until with a very old date: no commits before 2000, so error expected
# ============================================================================

test_section "--until with old date excludes all recent commits"

assert_failure git perf report --until="2000-01-01" -o -

# ============================================================================
# --since + -n combined: -n still limits count within the time window
# ============================================================================

test_section "--since and -n can be combined"

# --since="1 year ago" gives all 4 commits; -n 2 restricts to the 2 most recent
assert_success_with_output output git perf report --since="1 year ago" -n 2 -o -
lines=$(echo "$output" | wc -l | tr -d ' ')
assert_equals "$lines" "3" "Expected 2 measurements + header with --since + -n 2"

# ============================================================================
# --until with today or future date: includes all commits
# ============================================================================

test_section "--until with future date includes all commits"

# A future date should include all current commits (4 data rows + 1 header = 5 lines)
assert_success_with_output output git perf report --until="2030-01-01" -o -
lines=$(echo "$output" | wc -l | tr -d ' ')
assert_equals "$lines" "5" "Expected all 4 measurements + header with --until=2030-01-01"

# ============================================================================
# --since for audit: all recent commits are within range
# ============================================================================

test_section "--since works for audit"

# All 4 commits are within last year: HEAD + 3 historical → should pass
assert_success git perf audit -m timer --since="1 year ago"

# --since with old date + -n 2: only 2 commits (HEAD + 1 historical) → passes with default min-measurements=2
assert_success git perf audit -m timer --since="1 year ago" -n 2

# ============================================================================
# --after alias works (same as --since)
# ============================================================================

test_section "--after alias is equivalent to --since"

assert_success_with_output output git perf report --after="1 year ago" -o -
lines=$(echo "$output" | wc -l | tr -d ' ')
assert_equals "$lines" "5" "--after alias should behave identically to --since"

# ============================================================================
# --before alias works (same as --until)
# ============================================================================

test_section "--before with old date excludes all recent commits"

assert_failure git perf report --before="2000-01-01" -o -

# ============================================================================
# --until for audit: all commits before a future date
# ============================================================================

test_section "--until works for audit"

assert_success git perf audit -m timer --until="2030-01-01"

popd

test_stats
exit 0
