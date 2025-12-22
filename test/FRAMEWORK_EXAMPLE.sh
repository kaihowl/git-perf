#!/bin/bash

# Example test demonstrating the new testing framework
# This file shows all available assertion functions and best practices

# Disable verbose tracing - our assertions provide better output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

# ============================================================================
# Section 1: Equality Assertions
# ============================================================================

test_section "Equality assertions"

# Basic equality check
output="hello world"
assert_equals "$output" "hello world"

# Inequality check
assert_not_equals "$output" "goodbye world"

# Custom error messages
value="42"
assert_equals "$value" "42" "The answer should be 42"

# ============================================================================
# Section 2: String Containment Assertions
# ============================================================================

test_section "String containment assertions"

cd_temp_repo
output=$(git perf --version)

# Check that output contains expected strings
assert_contains "$output" "git-perf"
assert_not_contains "$output" "unexpected-string"

# OLD PATTERN (still works but deprecated):
# if ! echo "$output" | grep -q "git-perf"; then
#   echo "Expected to find git-perf"
#   exit 1
# fi

# ============================================================================
# Section 3: Command Execution Assertions
# ============================================================================

test_section "Command execution assertions - success cases"

cd_temp_repo

# Assert command succeeds
assert_success git perf --version

# Assert command succeeds and capture output
assert_success output git perf --version
assert_contains "$output" "git-perf"

# OLD PATTERN:
# git perf --version  # Relies on set -e
# output=$(git perf --version)

test_section "Command execution assertions - failure cases"

# Assert command fails
assert_failure git perf invalid-command

# Assert command fails and capture error message
assert_failure error_msg git perf audit -m nonexistent
assert_contains "$error_msg" "No measurements found"

# OLD PATTERN:
# git perf invalid-command && exit 1
# output=$(git perf audit -m nonexistent 2>&1 || true)
# if [[ $? -eq 0 ]]; then
#   echo "Expected command to fail"
#   exit 1
# fi

# ============================================================================
# Section 4: Regex Matching Assertions
# ============================================================================

test_section "Regex matching assertions"

cd_temp_repo
create_commit
git perf add -m build_time 123

output=$(git perf list)

# Match against patterns
assert_matches "$output" "build_time.*123"
assert_not_matches "$output" "^test_time"

# OLD PATTERN:
# re="build_time.*123"
# if ! [[ "$output" =~ $re ]]; then
#   echo "Pattern $re did not match"
#   exit 1
# fi

# ============================================================================
# Section 5: Boolean Condition Assertions
# ============================================================================

test_section "Boolean condition assertions"

cd_temp_repo
create_commit
git perf add -m timer 100

output=$(git perf list)

# Assert conditions
assert_true '[[ -n "$output" ]]' "Output should not be empty"
assert_false '[[ -z "$output" ]]' "Output should not be empty"

line_count=$(echo "$output" | wc -l)
assert_true '[[ $line_count -gt 0 ]]' "Should have at least one line"

# OLD PATTERN:
# if [[ -z "$output" ]]; then
#   echo "Output should not be empty"
#   exit 1
# fi

# ============================================================================
# Section 6: File/Directory Assertions
# ============================================================================

test_section "File and directory assertions"

cd_temp_repo

# Check git directory exists
assert_dir_exists ".git"

# Create a test file
echo "test content" > test_file.txt
assert_file_exists "test_file.txt"

# Remove it
rm test_file.txt
assert_file_not_exists "test_file.txt"

# OLD PATTERN:
# if [[ ! -f "test_file.txt" ]]; then
#   echo "File should exist"
#   exit 1
# fi

# ============================================================================
# Section 7: Complex Workflow Example
# ============================================================================

test_section "Complex workflow - audit with measurements"

cd_empty_repo

# Setup: Create measurements with known distribution
# mean: 2, std: 1
create_commit && git perf add -m timer 1
create_commit && git perf add -m timer 2
create_commit && git perf add -m timer 3
create_commit && git perf add -m timer 4

# Verify measurements were added
assert_success output git perf list
assert_contains "$output" "timer"

# Audit should pass with large deviation threshold
assert_success git perf audit -m timer -d 4

# Audit should fail with small deviation threshold
assert_failure git perf audit -m timer -d 1

# Verify we can get size information
assert_success size_output git perf size
assert_matches "$size_output" "[0-9]+ bytes"

# ============================================================================
# Section 8: Error Message Validation
# ============================================================================

test_section "Validating error messages"

cd_empty_repo

# Test that appropriate error is shown for missing measurements
assert_failure err git perf audit -m nonexistent
assert_contains "$err" "No measurements found"

# Test invalid flag produces error
assert_failure err git perf --invalid-flag
assert_matches "$err" "(unknown|invalid|unrecognized)"

# ============================================================================
# Section 9: Migration Pattern Examples
# ============================================================================

test_section "Common migration patterns"

cd_temp_repo
create_commit
git perf add -m build_time 100

# Pattern 1: Simple command that should succeed
# OLD: git perf list
# NEW: assert_success git perf list

# Pattern 2: Command that should fail
# OLD: git perf invalid-cmd && exit 1
# NEW: assert_failure git perf invalid-cmd

# Pattern 3: Output capture and validation
# OLD: output=$(git perf list)
#      if ! echo "$output" | grep -q "build_time"; then
#        echo "Missing build_time"
#        exit 1
#      fi
# NEW: assert_success output git perf list
#      assert_contains "$output" "build_time"

assert_success output git perf list
assert_contains "$output" "build_time"

# Pattern 4: Negative grep
# OLD: ! echo "$output" | grep -q "unexpected"
# NEW: assert_not_contains "$output" "unexpected"

assert_not_contains "$output" "unexpected"

# Pattern 5: Empty/non-empty checks
# OLD: if [[ -z "$output" ]]; then
#        echo "Should have output"
#        exit 1
#      fi
# NEW: assert_not_equals "$output" ""
#      OR: assert_true '[[ -n "$output" ]]'

assert_not_equals "$output" ""

# ============================================================================
# Final Statistics
# ============================================================================

test_stats
exit 0
