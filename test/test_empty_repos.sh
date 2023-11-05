#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo New repo, error out without crash
cd_empty_repo
output=$(git perf report 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'git error'* ]]; then
  echo "Missing 'git error' in output:"
  echo "$output"
  exit 1
fi

output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'git error'* ]]; then
  echo "Missing 'git error' in output:"
  echo "$output"
  exit 1
fi

echo New repo, single commit, error out without crash
cd_empty_repo
create_commit
output=$(git perf report 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements found'* ]]; then
  echo "Missing 'no performance measurements found' in output:"
  echo "$output"
  exit 1
fi
output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no measurement for HEAD'* ]]; then
  echo "Missing 'no measurement for HEAD' in output:"
  echo "$output"
  exit 1
fi
