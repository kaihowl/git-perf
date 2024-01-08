#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo New repo but current working directory different
cd_empty_repo
create_commit
git perf add -m test-measure 10

work_dir=$(pwd)
cd /tmp

if ! (git -C "$work_dir" perf report -o - | grep test-measure); then
  echo "Failed to change to work_dir and retrieve measurement"
  exit 1
fi
