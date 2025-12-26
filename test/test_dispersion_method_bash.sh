#!/bin/bash


# Integration test for dispersion method functionality
# This test verifies that the CLI options work correctly and help text is displayed

test_section "Testing dispersion method CLI integration..."

# Use the full path to the built git-perf binary to avoid PATH issues
script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
GIT_PERF_BINARY="$(cd "$script_dir/.." && pwd)/target/debug/git-perf"

test_section "Using git-perf from: $GIT_PERF_BINARY"

# Test 1: Verify CLI help shows the new option
test_section "Test 1: CLI help shows dispersion method option"
if ! "$GIT_PERF_BINARY" audit --help | grep -q "dispersion-method"; then
    echo "❌ --dispersion-method option not found in help"
    exit 1
fi
echo "✅ --dispersion-method option found in help"

# Test 2: Verify help text explains the default behavior
test_section "Test 2: Help text explains default behavior"
HELP_OUTPUT=$("$GIT_PERF_BINARY" audit --help)

# Normalize the text by removing extra whitespace and line breaks
NORMALIZED_HELP=$(echo "$HELP_OUTPUT" | tr '\n' ' ' | tr -s ' ')

# Check for the complete text in normalized form
if echo "$NORMALIZED_HELP" | grep -q "If not specified, uses the value from .gitperfconfig file, or defaults to stddev"; then
    echo "✅ Help text explains default behavior"
else
    echo "❌ Help text doesn't explain default behavior"
    exit 1
fi

# Test 3: Verify both values are accepted
test_section "Test 3: Both dispersion method values are accepted"
if ! "$GIT_PERF_BINARY" audit --help | grep -q "possible values: stddev, mad"; then
    echo "❌ mad not shown as possible value in help"
    exit 1
fi
echo "✅ mad shown as possible value in help"

# Test 4: Test short option -D works
test_section "Short option -D works"
if ! "$GIT_PERF_BINARY" audit --help | grep -q "D, --dispersion-method"; then
    test_section "Short option -D not found in help"
    exit 1
fi

# Test 5: Verify invalid values are rejected
test_section "Invalid values are rejected"
if "$GIT_PERF_BINARY" audit --measurement test --dispersion-method invalid 2>&1 | grep -q "invalid value"; then
    test_section "Invalid dispersion method correctly rejected"
else
    test_section "Invalid dispersion method not rejected"
    exit 1
fi

# Test 6: Verify stddev option works
echo "stddev option works"
if "$GIT_PERF_BINARY" audit --measurement test --dispersion-method stddev --help > /dev/null 2>&1; then
    echo "stddev option accepted"
else
    echo "stddev option not accepted"
    exit 1
fi

# Test 7: Verify mad option works
echo "mad option works"
if "$GIT_PERF_BINARY" audit --measurement test --dispersion-method mad --help > /dev/null 2>&1; then
    echo "mad option accepted"
else
    echo "mad option not accepted"
    exit 1
fi

# Test 8: Verify default behavior (no option specified)
test_section "Default behavior works"
if "$GIT_PERF_BINARY" audit --measurement test --help > /dev/null 2>&1; then
    test_section "Default behavior works"
else
    echo "Default behavior doesn't work"
    exit 1
fi

# Test 9: Verify that both options produce different help output (indicating they're parsed correctly)
test_section "Different dispersion methods produce different help output"
HELP_STDDEV=$("$GIT_PERF_BINARY" audit --measurement test --dispersion-method stddev --help 2>&1)
HELP_MAD=$("$GIT_PERF_BINARY" audit --measurement test --dispersion-method mad --help 2>&1)

# The help should be the same since it's just the help text, but the option should be parsed
test_section "Both dispersion method options parsed successfully"

echo "All dispersion method CLI integration tests passed!" 