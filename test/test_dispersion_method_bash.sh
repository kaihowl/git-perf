#!/bin/bash

export TEST_TRACE=0

# Integration test for dispersion method functionality
# This test verifies that the CLI options work correctly and help text is displayed

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

GIT_PERF_BINARY="$(cd "$script_dir/.." && pwd)/target/debug/git-perf"

test_section "CLI help shows dispersion method option"
assert_success_with_output output "$GIT_PERF_BINARY" audit --help
assert_contains "$output" "dispersion-method"

test_section "Help text explains default behavior"
assert_success_with_output output "$GIT_PERF_BINARY" audit --help
# Normalize the text by removing extra whitespace and line breaks
NORMALIZED_HELP=$(echo "$output" | tr '\n' ' ' | tr -s ' ')
assert_contains "$NORMALIZED_HELP" "If not specified, uses the value from .gitperfconfig file, or defaults to stddev"

test_section "Both dispersion method values are accepted"
assert_success_with_output output "$GIT_PERF_BINARY" audit --help
assert_contains "$output" "possible values: stddev, mad"

test_section "Short option -D works"
assert_success_with_output output "$GIT_PERF_BINARY" audit --help
assert_contains "$output" "D, --dispersion-method"

test_section "Invalid values are rejected"
assert_failure_with_output output "$GIT_PERF_BINARY" audit --measurement test --dispersion-method invalid
assert_contains "$output" "invalid value"

test_section "stddev option works"
assert_success "$GIT_PERF_BINARY" audit --measurement test --dispersion-method stddev --help

test_section "mad option works"
assert_success "$GIT_PERF_BINARY" audit --measurement test --dispersion-method mad --help

test_section "Default behavior works"
assert_success "$GIT_PERF_BINARY" audit --measurement test --help

test_section "Both dispersion method options parsed successfully"
assert_success_with_output HELP_STDDEV "$GIT_PERF_BINARY" audit --measurement test --dispersion-method stddev --help
assert_success_with_output HELP_MAD "$GIT_PERF_BINARY" audit --measurement test --dispersion-method mad --help

test_stats
exit 0 