#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd_empty_repo
create_commit
git perf add -m timer 1 -k os=ubuntu
git perf add -m timer 0.9 -k os=ubuntu
git perf add -m timer 1.2 -k os=mac
git perf add -m timer 1.1 -k os=mac
create_commit
git perf add -m timer 2.1 -k os=ubuntu
git perf add -m timer 2.2 -k os=ubuntu
git perf add -m timer 2.1 -k os=mac
git perf add -m timer 2.0 -k os=mac
create_commit
git perf add -m timer 3.1 -k os=ubuntu
git perf add -m timer 3.2 -k os=ubuntu
git perf add -m timer 3.3 -k os=mac
git perf add -m timer 3.4 -k os=mac
create_commit
git perf add -m timer 4 -k os=ubuntu
git perf add -m timer 4 -k os=ubuntu
git perf add -m timer 4.3 -k os=mac
git perf add -m timer 4.3 -k os=mac
git perf add -m timer2 2 -k os=mac

git perf report -o all_result.html
git perf report -o separated_result.html -s os
git perf report -o single_result.html -m timer
git perf report -o separated_single_result.html -m timer -s os

output=$(git perf report -m timer-does-not-exist 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements'* ]]; then
  echo "No warning for missing measurements"
  echo "$output"
  exit 1
fi

output=$(git perf report -s does-not-exist 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'invalid separator'* ]]; then
  echo "No warning for invalid separator 'does-not-exist'"
  echo "$output"
  exit 1
fi

