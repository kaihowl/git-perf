#!/bin/bash


script_dir=$(dirname "$0")

python3 -m pip install -r "${script_dir}/../requirements.txt"

tests=()

# TODO(kaihowl) no limit
for file in "${script_dir}"/test_*.sh; do
  echo "Running test $file..."
  log_file=$(mktemp)
  "./$file" > "${log_file}" 2>&1 &
  tests+=("$! $log_file $file")
done

for test in "${tests[@]}"; do
  IFS=' ' read -r pid logfile file <<< "${test}"
  echo "Waiting for $file..."
  if ! wait "$pid"; then
    echo "Failed."
    cat "$logfile" | while read line; do echo "[$file] $line"; done
  else
    echo "Success."
  fi
done

exit 0

