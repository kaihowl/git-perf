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
  echo Missing 'following arguments' in output:
  echo "$output"
  exit 1
fi

