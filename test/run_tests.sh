#!/bin/bash

set -e

script_dir=$(dirname "$0")
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

# Execute commands in parallel with a maximum of 3 processes at a time
for cmd in "${commands[@]}"; do
    execute_command bash -c "$cmd" &
    # Limit the number of concurrent processes
    while [ $(jobs -p | wc -l) -ge $num_procs ]; do
        sleep 1
    done
done

# Wait for all background jobs to finish
wait
echo "All commands executed successfully."

