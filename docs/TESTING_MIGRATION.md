# Bash Testing Framework Migration Guide

This guide explains how to use the new bash testing framework and migrate existing tests from the old pattern-based approach.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Environment Variables](#environment-variables)
- [Assertion Functions](#assertion-functions)
- [Migration Patterns](#migration-patterns)
- [Example Tests](#example-tests)
- [Debugging Failed Tests](#debugging-failed-tests)

## Overview

### What Changed?

The old testing approach relied on:
- `set -e` for implicit failure detection
- `set -x` for verbose trace output
- Manual `&& exit 1` patterns for negative assertions
- Inconsistent error messages
- Hard-to-read failure output

The new framework provides:
- **Explicit assertion functions** with clear semantics
- **Standardized failure output** with `FAIL:` and `ERROR:` prefixes
- **Line number tracking** for quick debugging
- **Optional quiet mode** (disable `set -x` clutter)
- **Test statistics** showing sections and assertion counts
- **Better failure messages** with actual vs expected values

### Backward Compatibility

All existing tests continue to work unchanged. The new framework is opt-in:
- Set `TEST_TRACE=0` to disable verbose `set -x` output
- Use new assertion functions for clearer test code
- Keep existing patterns where they work well

## Quick Start

### Using the New Framework

```bash
#!/bin/bash

# Disable verbose tracing (optional but recommended)
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
source "$script_dir/common.sh"

test_section "Basic functionality tests"

# Setup (regular commands, not assertions)
cd_temp_repo

# Actual test expectations (use assertions)
assert_success git perf --version
assert_failure git perf invalid-command

# Capture output and validate
assert_success_with_output output git perf list
assert_contains "$output" "expected-string"

# Show statistics at end
test_stats
exit 0
```

### Example Output

**When tests pass:**
```
=== Section 1: Basic functionality tests ===

Test Statistics:
  Sections: 1
  Assertions Passed: 3
  Assertions Failed: 0
```

**When tests fail:**
```
=== Section 1: Basic functionality tests ===

FAIL: test_example:27
ERROR: Command should fail but succeeded
       Command: git perf audit -m timer -d 1.9999
       Output:
       Audit passed for measurement 'timer'

Test Statistics:
  Sections: 1
  Assertions Passed: 2
  Assertions Failed: 1
```

## Environment Variables

### TEST_TRACE

Controls verbose `set -x` output.

```bash
# Disable verbose tracing (recommended for new tests)
export TEST_TRACE=0

# Enable verbose tracing (default, for backward compatibility)
export TEST_TRACE=1
```

### TEST_FAIL_FAST

Controls whether tests stop on first failure.

```bash
# Stop on first failure (default)
export TEST_FAIL_FAST=1

# Continue after failures (collect all failures)
export TEST_FAIL_FAST=0
```

## Assertion Functions

### Equality Assertions

#### assert_equals

```bash
assert_equals actual expected [message]
```

**Example:**
```bash
output="hello"
assert_equals "$output" "hello"
assert_equals "$output" "hello" "Output should be hello"
```

**Failure output:**
```
FAIL: test_example:15
ERROR: Assertion failed: values not equal
       Expected: goodbye
       Actual:   hello
```

#### assert_not_equals

```bash
assert_not_equals actual expected [message]
```

**Example:**
```bash
output="hello"
assert_not_equals "$output" "goodbye"
```

### String Containment Assertions

#### assert_contains

```bash
assert_contains haystack needle [message]
```

**Example:**
```bash
output=$(git perf list)
assert_contains "$output" "build_time"
```

**Failure output:**
```
FAIL: test_example:20
ERROR: Assertion failed: string not found
       Expected to find: build_time
       In output:
       test_time: epoch=12345 unit=ms
```

#### assert_not_contains

```bash
assert_not_contains haystack needle [message]
```

**Example:**
```bash
output=$(git perf list)
assert_not_contains "$output" "unexpected-measurement"
```

### Regex Matching Assertions

#### assert_matches

```bash
assert_matches string regex [message]
```

**Example:**
```bash
output=$(git perf list)
assert_matches "$output" "build_time.*[0-9]+"
```

#### assert_not_matches

```bash
assert_not_matches string regex [message]
```

**Example:**
```bash
assert_not_matches "$output" "^ERROR:"
```

### Command Execution Assertions

#### assert_success

```bash
assert_success command args...
```

Verifies a command succeeds (exit code 0). Does not capture output.

**Example:**
```bash
# Just verify command succeeds
assert_success git perf --version
```

**Failure output:**
```
FAIL: test_example:25
ERROR: Command should succeed but failed with exit code 2
       Command: git perf audit -m nonexistent
       Output:
       Error: No measurements found for 'nonexistent'
```

#### assert_success_with_output

```bash
assert_success_with_output output_var command args...
```

Verifies a command succeeds AND captures its output (stdout + stderr) into the specified variable.

**Example:**
```bash
# Capture output for later validation
assert_success_with_output output git perf list
assert_contains "$output" "build_time"
```

#### assert_failure

```bash
assert_failure command args...
```

Verifies a command fails (non-zero exit code). Does not capture output.

**Example:**
```bash
# Just verify command fails
assert_failure git perf invalid-command
```

**Failure output:**
```
FAIL: test_example:30
ERROR: Command should fail but succeeded
       Command: git perf audit -m timer -d 1.9999
       Output:
       Audit passed for measurement 'timer'
```

#### assert_failure_with_output

```bash
assert_failure_with_output output_var command args...
```

Verifies a command fails AND captures its output (stdout + stderr) into the specified variable.

**Example:**
```bash
# Capture error message for validation
assert_failure_with_output error_msg git perf audit -m nonexistent
assert_contains "$error_msg" "No measurements found"
```

### Boolean Condition Assertions

#### assert_true

```bash
assert_true condition [message]
```

**Example:**
```bash
line_count=$(echo "$output" | wc -l)
assert_true '[[ $line_count -gt 0 ]]' "Should have at least one line"
```

#### assert_false

```bash
assert_false condition [message]
```

**Example:**
```bash
assert_false '[[ -z "$output" ]]' "Output should not be empty"
```

### File/Directory Assertions

#### assert_file_exists / assert_file_not_exists

```bash
assert_file_exists file [message]
assert_file_not_exists file [message]
```

**Example:**
```bash
assert_file_exists ".gitperfconfig"
assert_file_not_exists "nonexistent.txt"
```

#### assert_dir_exists

```bash
assert_dir_exists dir [message]
```

**Example:**
```bash
assert_dir_exists ".git"
```

### Test Organization Functions

#### test_section

```bash
test_section "description"
```

Marks a new test section with automatic numbering.

**Example:**
```bash
test_section "Basic functionality tests"
test_section "Error handling tests"
test_section "Integration tests"
```

**Output:**
```
=== Section 1: Basic functionality tests ===
=== Section 2: Error handling tests ===
=== Section 3: Integration tests ===
```

#### test_stats

```bash
test_stats
```

Shows test statistics at end of test file.

**Example:**
```bash
test_stats
exit 0
```

**Output:**
```
Test Statistics:
  Sections: 3
  Assertions Passed: 15
  Assertions Failed: 0
```

## Migration Patterns

### Pattern 1: Command Expected to Fail

**OLD:**
```bash
git perf audit -m timer -d 1 && exit 1
```

**NEW:**
```bash
assert_failure git perf audit -m timer -d 1
```

**Why better:** Clearer intent, better error message if assertion fails.

---

### Pattern 2: Output Validation with grep

**OLD:**
```bash
output=$(git perf config --list)
if ! echo "$output" | grep -q "build_time"; then
  echo "Expected to find build_time"
  exit 1
fi
```

**NEW:**
```bash
assert_success_with_output output git perf config --list
assert_contains "$output" "build_time"
```

**Why better:**
- Combines command execution and output capture
- Clearer error message with actual output shown
- Less boilerplate code

---

### Pattern 3: Negative grep Check

**OLD:**
```bash
output=$(git perf config --list --measurement build_time)
! echo "$output" | grep -q "test_time"
```

**NEW:**
```bash
assert_success_with_output output git perf config --list --measurement build_time
assert_not_contains "$output" "test_time"
```

**Why better:** Explicit assertion with clear error message on failure.

---

### Pattern 4: Empty/Non-Empty Checks

**OLD:**
```bash
if [[ -z ${output} ]]; then
  echo "There should be output in stderr but instead it is empty"
  exit 1
fi
```

**NEW:**
```bash
assert_not_equals "$output" "" "Output should not be empty"
# OR
assert_true '[[ -n "$output" ]]' "Output should not be empty"
```

**Why better:** Explicit assertion with automatic context on failure.

---

### Pattern 5: Section Markers

**OLD:**
```bash
echo "Config command tests - basic list"
```

**NEW:**
```bash
test_section "Config command tests - basic list"
```

**Why better:** Automatic numbering, consistent formatting, cleaner output.

---

### Pattern 6: Error Capture and Validation

**OLD:**
```bash
if git perf config --list --validate 2>&1; then
    echo "Expected validation to fail due to missing epoch"
    exit 1
fi
output=$(git perf config --list --validate 2>&1 || true)
echo "$output" | grep -q "No epoch configured"
```

**NEW:**
```bash
assert_failure_with_output output git perf config --list --validate
assert_contains "$output" "No epoch configured"
```

**Why better:** Much more concise, clearer intent, better error messages.

---

### Pattern 7: Regex Matching

**OLD:**
```bash
re=".*following (required )?arguments.*"
if [[ ! ${output} =~ $re ]]; then
  echo "Missing 'following arguments' in output:"
  echo "Output: '$output'"
  exit 1
fi
```

**NEW:**
```bash
assert_matches "$output" ".*following (required )?arguments.*" \
  "Missing 'following arguments' in output"
```

**Why better:** Automatic context showing pattern and actual string.

---

### Pattern 8: File Existence Checks

**OLD:**
```bash
if [[ ! -f ".gitperfconfig" ]]; then
  echo "Config file should exist"
  exit 1
fi
```

**NEW:**
```bash
assert_file_exists ".gitperfconfig" "Config file should exist"
```

**Why better:** Clearer intent, consistent with other assertions.

---

### When to Use Assertions vs Regular Commands

**IMPORTANT:** Only use assertion functions for actual test expectations. Do NOT use them for test setup or preparation commands.

**Why?** Each assertion:
- Increments the pass/fail counter
- Appears in test statistics
- Adds noise to test output when used for setup

**Use assertions for:**
- ✅ Verifying expected behavior (the actual test)
- ✅ Validating output contains expected strings
- ✅ Checking command success/failure as the test goal

**Use regular commands for:**
- ❌ Setting up test environment
- ❌ Creating test data
- ❌ Navigating directories
- ❌ Initializing state

**Example - GOOD:**
```bash
# Setup (regular commands)
cd_temp_repo
git perf add timer 100

# Actual test expectations (assertions)
assert_success_with_output output git perf list
assert_contains "$output" "timer"
```

**Example - BAD:**
```bash
# DON'T do this - clutters output with setup noise
assert_success cd_temp_repo
assert_success git perf add timer 100
assert_success_with_output output git perf list
assert_contains "$output" "timer"
```

**Output difference:**

Good approach shows:
```
Test Statistics:
  Assertions Passed: 2
```

Bad approach shows:
```
Test Statistics:
  Assertions Passed: 4  # Misleading - only 2 are actual tests
```

**Rule of thumb:** If the command failing is a setup problem (not a test failure), don't use an assertion.

---

## Example Tests

### FRAMEWORK_EXAMPLE.sh

A comprehensive example demonstrating all assertion types and patterns. Located at `test/FRAMEWORK_EXAMPLE.sh`.

This is a documentation example showing:
- All assertion types with examples
- Migration patterns with before/after code
- Test organization with sections
- Proper usage of TEST_TRACE=0

To run (requires git-perf in PATH):
```bash
export PATH="$PWD/target/release:$PATH"
bash test/FRAMEWORK_EXAMPLE.sh
```

Note: This file is not run as part of the automated test suite (renamed from test_*.sh to avoid automatic execution).

## Debugging Failed Tests

### Finding Failures

The new framework makes it easy to grep for failures:

```bash
# Run tests and show only failures
./test/run_tests.sh 2>&1 | grep -E '^FAIL:|^ERROR:'

# Find failures in specific test
./test/run_tests.sh test_config 2>&1 | grep '^FAIL:'

# Show failure summary
./test/run_tests.sh 2>&1 | grep -A 2 '^FAIL:'
```

### Reading Failure Output

Each failure shows:
1. **File and line number** - `FAIL: test_name:27`
2. **Error message** - `ERROR: Command should fail but succeeded`
3. **Context** - Command, expected/actual values, output

**Example:**
```
FAIL: test_audit_basic:28
ERROR: Command should fail but succeeded
       Command: git perf audit -m timer -d 1.9999
       Output:
       Audit passed for measurement 'timer'
```

This tells you:
- Failure is in `test/test_audit_basic.sh` at line 28
- The command was expected to fail but succeeded
- The exact command and output are shown

### Enabling Verbose Output

For debugging, enable verbose tracing:

```bash
# Run specific test with trace
TEST_TRACE=1 ./test/test_audit_basic.sh

# Run all tests with trace
TEST_TRACE=1 ./test/run_tests.sh
```

### Collecting All Failures

To see all failures in a test (not just the first):

```bash
TEST_FAIL_FAST=0 ./test/test_example.sh
```

## Migration Strategy

### Recommended Approach

1. **Start with simple tests** - Migrate small tests first to get familiar
2. **Use examples as templates** - Copy patterns from `test_framework_example.sh`
3. **Test incrementally** - Run migrated test after each change
4. **Keep old patterns temporarily** - Add new assertions alongside old code, then remove old code once verified

### Gradual Migration

You don't need to migrate all tests at once:

1. **Phase 1:** Use new framework for new tests
2. **Phase 2:** Migrate simple tests (like `test_version.sh`)
3. **Phase 3:** Migrate complex tests (like `test_config_cmd.sh`)
4. **Phase 4:** Clean up deprecated patterns

### Testing Migration

After migrating a test:

```bash
# Test the specific migrated file
./test/test_your_migrated_test.sh

# Verify it still passes in the suite
./test/run_tests.sh test_your_migrated_test

# Run full suite to ensure no breakage
./test/run_tests.sh
```

## Best Practices

1. **Use assertions only for test expectations** - NOT for setup commands (see "When to Use Assertions vs Regular Commands")
2. **Always add `TEST_TRACE=0`** to migrated tests for clean output
3. **Use `test_section()`** to organize tests into logical groups
4. **Provide custom messages** for important assertions
5. **Use `_with_output` variants when you need to capture** - `assert_success_with_output` or `assert_failure_with_output`
6. **Call `test_stats`** at end of test for assertion counts
7. **Prefer specific assertions** - use `assert_contains` over `assert_true` with grep
8. **Keep tests focused** - one assertion per logical check

## FAQ

### Q: Do I need to migrate existing tests?

No. The new framework is backward compatible. Migrate tests when:
- You're modifying them anyway
- You want better failure output
- You want to add new test cases

### Q: Can I mix old and new patterns?

Yes. The old and new patterns work together:

```bash
# Old pattern still works
git perf --version

# New pattern provides better output
assert_success git perf --version
```

### Q: What if I want verbose output?

Use `TEST_TRACE=1` (the default for backward compatibility):

```bash
TEST_TRACE=1 ./test/run_tests.sh
```

### Q: How do I debug a failing assertion?

Look at the `FAIL:` line for file and line number:

```
FAIL: test_example:42
```

This means `test/test_example.sh` line 42 failed.

### Q: Can I use these functions in non-test scripts?

The functions are defined in `test/common.sh` and designed for tests, but technically they work in any bash script that sources `common.sh`.

## See Also

- **[test/test_framework_example.sh](../test/test_framework_example.sh)** - Comprehensive examples
- **[test/test_audit_basic.sh](../test/test_audit_basic.sh)** - ✅ Fully migrated real-world example (14 assertions, 3 sections)
- **[test/common.sh](../test/common.sh)** - Framework implementation
- **[CONTRIBUTING.md](../CONTRIBUTING.md)** - Testing requirements for contributors
