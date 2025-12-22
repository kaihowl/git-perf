#!/bin/bash

set -e

# Optional tracing (default: enabled for backward compatibility)
# Set TEST_TRACE=0 to disable verbose set -x output
if [[ "${TEST_TRACE:-1}" == "1" ]]; then
  PS4='${BASH_SOURCE}:${LINENO}: '
  set -x
fi

export RUST_BACKTRACE=1

shopt -s nocasematch

# Hermetic git environment - ignore system and global git config
# This prevents issues with commit signing and other global git settings
export GIT_CONFIG_NOSYSTEM=true
export GIT_CONFIG_GLOBAL=/dev/null

# Set git author and committer info for tests
export GIT_COMMITTER_NAME="github-actions[bot]"
export GIT_COMMITTER_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"
export GIT_AUTHOR_NAME="github-actions[bot]"
export GIT_AUTHOR_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"

function cd_empty_repo() {
  local tmpgit
  tmpgit="$(mktemp -d)"
  pushd "${tmpgit}"
  git init --initial-branch=master
}

function create_commit() {
  # Since some of the commits are added in the same instant with the same content, they result in the same hash.
  # Instead, use random files such that there is a very small chance in collision.
  local file
  file=$RANDOM
  # As the RANDOM function can collide, ensure that with each call of create_commit, the file content changes
  # by appending to the (often but not always) new file.
  # Without this, the git commit might end up as 'empty'.
  echo content >> "$file"
  git add "$file"
  git commit -m 'my commit'
}

function cd_temp_repo() {
  cd_empty_repo
  create_commit
  create_commit
  create_commit
  create_commit
}

# Deprecated: Use assert_contains instead
function assert_output_contains() {
  local output="$1"
  local expected="$2"
  local error_message="${3:-Missing expected string in output}"

  if [[ $output != *"$expected"* ]]; then
    echo "$error_message:"
    echo "$output"
    exit 1
  fi
}

# Deprecated: Use assert_not_contains instead
function assert_output_not_contains() {
  local output="$1"
  local unexpected="$2"
  local error_message="${3:-Unexpected string found in output}"

  if [[ $output == *"$unexpected"* ]]; then
    echo "$error_message:"
    echo "$output"
    exit 1
  fi
}

# ============================================================================
# New Test Framework - Explicit Assertions with Clear Failure Output
# ============================================================================

# Test state tracking
_TEST_SECTION_COUNT=0
_TEST_PASS_COUNT=0
_TEST_FAIL_COUNT=0

# Internal failure handler
# Captures context and formats output with FAIL:/ERROR: prefixes for easy grepping
_test_fail() {
  _TEST_FAIL_COUNT=$((_TEST_FAIL_COUNT + 1))

  # Get caller information for better context
  local caller_line="${BASH_LINENO[1]}"
  local caller_file="${BASH_SOURCE[2]}"
  local test_name=$(basename "$caller_file" .sh)

  # Disable set -x temporarily for clean error output
  local xtrace_enabled=0
  if [[ "$-" =~ x ]]; then
    xtrace_enabled=1
    set +x
  fi

  echo "" >&2
  echo "FAIL: $test_name:$caller_line" >&2
  echo "ERROR: $1" >&2
  shift

  # Print additional context lines
  while [[ $# -gt 0 ]]; do
    echo "       $1" >&2
    shift
  done

  echo "" >&2

  # Re-enable set -x if it was on
  if [[ $xtrace_enabled -eq 1 ]]; then
    set -x
  fi

  # Exit immediately if TEST_FAIL_FAST is set (default: yes)
  if [[ "${TEST_FAIL_FAST:-1}" == "1" ]]; then
    exit 1
  fi
}

# ============================================================================
# Equality Assertions
# ============================================================================

assert_equals() {
  local actual="$1"
  local expected="$2"
  local message="${3:-Assertion failed: values not equal}"

  if [[ "$actual" != "$expected" ]]; then
    _test_fail "$message" \
      "Expected: $expected" \
      "Actual:   $actual"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_not_equals() {
  local actual="$1"
  local expected="$2"
  local message="${3:-Assertion failed: values should not be equal}"

  if [[ "$actual" == "$expected" ]]; then
    _test_fail "$message" \
      "Value should not equal: $expected" \
      "But got:                $actual"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# String Containment Assertions
# ============================================================================

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="${3:-Assertion failed: string not found}"

  if [[ "$haystack" != *"$needle"* ]]; then
    _test_fail "$message" \
      "Expected to find: $needle" \
      "In output:" \
      "$haystack"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_not_contains() {
  local haystack="$1"
  local needle="$2"
  local message="${3:-Assertion failed: unexpected string found}"

  if [[ "$haystack" == *"$needle"* ]]; then
    _test_fail "$message" \
      "Expected NOT to find: $needle" \
      "But found it in output:" \
      "$haystack"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# Regex Matching Assertions
# ============================================================================

assert_matches() {
  local string="$1"
  local regex="$2"
  local message="${3:-Assertion failed: pattern does not match}"

  if ! [[ "$string" =~ $regex ]]; then
    _test_fail "$message" \
      "Pattern: $regex" \
      "String:  $string"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_not_matches() {
  local string="$1"
  local regex="$2"
  local message="${3:-Assertion failed: pattern should not match}"

  if [[ "$string" =~ $regex ]]; then
    _test_fail "$message" \
      "Pattern should not match: $regex" \
      "But matched string:       $string"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# Command Execution Assertions
# ============================================================================

assert_success() {
  local _unused_output
  assert_success_with_output _unused_output "$@"
}

assert_success_with_output() {
  local output_var="$1"
  shift

  local _cmd_output
  local exit_code

  _cmd_output=$("$@" 2>&1)
  exit_code=$?
  eval "$output_var=\$_cmd_output"

  if [[ $exit_code -ne 0 ]]; then
    _test_fail "Command should succeed but failed with exit code $exit_code" \
      "Command: $*" \
      "Output:" \
      "$_cmd_output"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_failure() {
  local _unused_output
  assert_failure_with_output _unused_output "$@"
}

assert_failure_with_output() {
  local output_var="$1"
  shift

  local _cmd_output
  local exit_code

  set +e
  _cmd_output=$("$@" 2>&1)
  exit_code=$?
  set -e

  eval "$output_var=\$_cmd_output"

  if [[ $exit_code -eq 0 ]]; then
    _test_fail "Command should fail but succeeded" \
      "Command: $*" \
      "Output:" \
      "$_cmd_output"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# Boolean Condition Assertions
# ============================================================================

assert_true() {
  local condition="$1"
  local message="${2:-Assertion failed: condition is false}"

  if ! eval "$condition"; then
    _test_fail "$message" \
      "Condition: $condition" \
      "Expected: true" \
      "Actual:   false"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_false() {
  local condition="$1"
  local message="${2:-Assertion failed: condition is true}"

  if eval "$condition"; then
    _test_fail "$message" \
      "Condition: $condition" \
      "Expected: false" \
      "Actual:   true"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# File/Directory Assertions
# ============================================================================

assert_file_exists() {
  local file="$1"
  local message="${2:-File does not exist: $file}"

  if [[ ! -f "$file" ]]; then
    _test_fail "$message"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_file_not_exists() {
  local file="$1"
  local message="${2:-File should not exist: $file}"

  if [[ -f "$file" ]]; then
    _test_fail "$message"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

assert_dir_exists() {
  local dir="$1"
  local message="${2:-Directory does not exist: $dir}"

  if [[ ! -d "$dir" ]]; then
    _test_fail "$message"
  fi
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))
}

# ============================================================================
# Test Organization Functions
# ============================================================================

test_section() {
  local section_name="$1"
  _TEST_SECTION_COUNT=$((_TEST_SECTION_COUNT + 1))

  # Disable set -x temporarily for clean output
  local xtrace_enabled=0
  if [[ "$-" =~ x ]]; then
    xtrace_enabled=1
    set +x
  fi

  echo ""
  echo "=== Section $_TEST_SECTION_COUNT: $section_name ==="

  # Re-enable set -x if it was on
  if [[ $xtrace_enabled -eq 1 ]]; then
    set -x
  fi
}

test_pass() {
  local message="${1:-Test passed}"
  _TEST_PASS_COUNT=$((_TEST_PASS_COUNT + 1))

  # Disable set -x temporarily for clean output
  local xtrace_enabled=0
  if [[ "$-" =~ x ]]; then
    xtrace_enabled=1
    set +x
  fi

  echo "PASS: $message"

  # Re-enable set -x if it was on
  if [[ $xtrace_enabled -eq 1 ]]; then
    set -x
  fi
}

test_stats() {
  # Disable set -x temporarily for clean output
  local xtrace_enabled=0
  if [[ "$-" =~ x ]]; then
    xtrace_enabled=1
    set +x
  fi

  echo ""
  echo "Test Statistics:"
  echo "  Sections: ${_TEST_SECTION_COUNT:-0}"
  echo "  Assertions Passed: ${_TEST_PASS_COUNT:-0}"
  echo "  Assertions Failed: ${_TEST_FAIL_COUNT:-0}"

  # Re-enable set -x if it was on
  if [[ $xtrace_enabled -eq 1 ]]; then
    set -x
  fi
}
