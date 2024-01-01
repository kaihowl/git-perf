#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Output into broken pipe
cd_empty_repo
create_commit
git perf add -m test 2
create_commit
git perf add -m test 4
create_commit
git perf report -o - | true
if [[ ${PIPESTATUS[0]} -ne 0 ]]; then
  echo git-perf pipe failed
  exit 1
fi
