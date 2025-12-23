#!/bin/bash

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
num_procs=4

if [[ $# -eq 1 ]]; then
  include_pattern=$1
else
  # Include both test_ and testslow_
  include_pattern="test"
fi

# Color support for terminals
if [[ -t 1 ]]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  BLUE='\033[0;34m'
  NC='\033[0m' # No Color
else
  RED=''
  GREEN=''
  YELLOW=''
  BLUE=''
  NC=''
fi

# Function to execute command and capture output
execute_command() {
    local test_script="$1"
    local output_file
    output_file=$(mktemp)
    local test_name=$(basename "$test_script")

    echo "Running '$test_name'"

    if ! bash "$test_script" > "$output_file" 2>&1; then
        echo -e "${RED}✗ FAILED${NC}: $test_name"
        echo ""
        echo "--- Test Output ---"
        cat "$output_file"
        echo "--- End Output ---"
        echo ""

        # Extract just the failures for summary
        if grep -q "^FAIL:" "$output_file"; then
          echo -e "${YELLOW}Failures Summary:${NC}"
          grep "^FAIL:\|^ERROR:" "$output_file" | sed 's/^/  /'
          echo ""
        fi

        rm -f "$output_file"
        return 1
    else
        echo -e "${GREEN}✓ PASSED${NC}: $test_name"

        # Show stats if available
        if grep -q "^Test Statistics:" "$output_file"; then
          grep "^Test Statistics:\|^  " "$output_file" | sed 's/^/  /'
        fi

        rm -f "$output_file"
        return 0
    fi
}

# Find command to generate the list of scripts
commands=()
while IFS= read -r -d '' file; do
    commands+=("$file")
done < <(find "${script_dir}" -name "$include_pattern*.sh" -print0)

pids=()

# Execute commands in parallel with a maximum of 3 processes at a time
for cmd in "${commands[@]}"; do
    execute_command "$cmd" &
    pids+=($!)
    # Limit the number of concurrent processes
    while [ $(jobs -p | wc -l) -ge $num_procs ]; do
        sleep 1
    done
done

fail_count=0
failed_tests=()

# Wait for all background jobs to finish and track failures
for i in "${!pids[@]}"; do
  pid=${pids[$i]}
  if ! wait "$pid"; then
    ((fail_count++))
    test_name=$(basename "${commands[$i]}" .sh)
    failed_tests+=("$test_name")
  fi
done

echo ""
echo "======================================"
if [[ $fail_count == 0 ]]; then
  echo -e "${GREEN}All tests passed!${NC}"
  exit 0
else
  echo -e "${RED}Failed tests: $fail_count${NC}"
  echo ""
  echo "Failed test files:"
  for test in "${failed_tests[@]}"; do
    echo -e "  ${RED}✗${NC} $test"
  done
  echo ""
  echo "To see only failures, run:"
  echo "  $0 $include_pattern 2>&1 | grep -E '^FAIL:|^ERROR:'"
  exit $fail_count
fi

