#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

output=$(git perf 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'--help'* ]]; then
  echo "No warning for missing arguments"
  echo "$output"
  exit 1
fi

output=$(git perf --version)
if ! [[ ${output} =~ ^(git-perf )?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Expected version number or placeholder in output."
    echo "$output"
    exit 1
fi

# Git version too old
export PATH=${script_dir}/fake_git_2.40.0:$PATH
git-perf --version && exit 1

# Git version just right
export PATH=${script_dir}/fake_git_2.41.0:$PATH
git-perf --version

exit 0
