#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Missing arguments
cd_temp_repo
output=$(git perf measure -m test-measure 2>&1 1>/dev/null) && exit 1
re=".*following (required )?arguments.*"
if [[ ! ${output} =~ $re ]]; then
  echo "Missing 'following arguments' in output:"
  echo "$output"
  exit 1
fi

echo Non-existing command
cd_temp_repo
git perf measure -m test-measure -- does-not-exist && exit 1

echo Valid command, repeated measurements
cd_temp_repo
git perf measure -m test-measure -n 5 -- true
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 5 ]] || exit 1

echo Measurements in nanoseconds
cd_temp_repo
git perf measure -m test-measure -- bash -c 'sleep 0.1'
val=$(git perf report -o - | cut -f5 -d, | head -n 1)
if [[ 1 -eq "$(echo "${val} < 10^(9-1)" | bc)" ]]; then
    echo "Measure is not in nanosecond precision"
    echo "0.1 seconds of sleep + fork + etc. overhead is currently $val"
    exit 1
fi

exit 0
