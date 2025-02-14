#!/bin/bash

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
num_procs=4

# Function to execute command and capture output
execute_command() {
    local output_file
    output_file=$(mktemp)
    # Trap to clean up temporary file on exit
    trap 'rm -f "$output_file"' EXIT

    echo "Running '$*'"

    if ! "$@" > "$output_file" 2>&1; then
        echo "Command '$*' failed. Output:"
        cat "$output_file"
        exit 1
    fi
}

# Find command to generate the list of scripts
commands=()
while IFS= read -r -d '' file; do
    commands+=("$file")
done < <(find "${script_dir}" -name 'test_*.sh' -print0)

pids=()

# Execute commands in parallel with a maximum of 3 processes at a time
for cmd in "${commands[@]}"; do
    execute_command bash -c "$cmd" &
    pids+=($!)
    # Limit the number of concurrent processes
    while [ $(jobs -p | wc -l) -ge $num_procs ]; do
        sleep 1
    done
done

fail_count=0
# Wait for all background jobs to finish
for pid in "${pids[@]}"; do
  if ! wait "$pid"; then
    ((fail_count++))
  fi
done

if [[ $fail_count != 0 ]]; then
  echo "Failed tests: $fail_count"
  exit $fail_count
else
  echo "All commands executed successfully."
  exit 0
fi

