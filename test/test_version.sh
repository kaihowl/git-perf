#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

output=$(git perf 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'are required'* ]]; then
  echo No warning for missing arguments
  echo "$output"
  exit 1
fi

output=$(git perf --version)
if ! [[ ${output} =~ \d+\.\d+.\d+|\<\<VERSION\>\> ]]; then
    echo Expected version number or placeholder in output.
    echo "$output"
    exit 1
fi
