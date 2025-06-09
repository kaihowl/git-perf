#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)

# shellcheck source=test/common.sh
source "$script_dir/common.sh"

epoch=42

echo Add invalid measurements

echo Empty measurement
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "\n"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'too few items'* ]]; then
  echo "Missing 'too few items' in output:"
  echo "$output"
  exit 1
fi

echo Measurement with just date
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$(date +%s)"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'too few items'* ]]; then
  echo "Missing 'too few items' in output:"
  echo "$output"
  exit 1
fi

echo Measurement without date
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$RANDOMkey=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'skipping'* ]]; then
  echo "Missing 'skipping' in output:"
  echo "$output"
  exit 1
fi

echo Measurement without kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOM"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo "There should be no output in stderr but instead there is:"
  echo "$output"
  exit 1
fi

echo Measurement with invalid kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMtestotherteststuff"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -z ${output} ]]; then
  echo "There should be output in stderr but instead it is empty"
  exit 1
fi

echo Measurement valid but with too many separators
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo "There should be no output in stderr but instead there is:"
  echo "$output"
  exit 1
fi

echo Duplicate kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=valuekey=valuekey=valuekey=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'Duplicate entries for key key with same value'* ]]; then
  echo "Expected warning about 'Duplicate entries for key key with same value' in the output"
  echo "Output:"
  echo "$output"
  exit 1
fi

# Verify warning is only printed once
warning_count=$(echo "$output" | grep -c "Duplicate entries for key key with same value")
if [[ $warning_count -ne 1 ]]; then
  echo "Expected warning to appear exactly once, but found $warning_count occurrences"
  echo "Output:"
  echo "$output"
  exit 1
fi

echo Conflicting kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$epochmyothermeasurement$(date +%s)$RANDOMkey=valuekey=value2"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'Conflicting values'* ]]; then
  echo "Expected warning about 'Conflicting values' in the output"
  echo "Output:"
  echo "$output"
  exit 1
fi
