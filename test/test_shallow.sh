#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd_temp_repo
full_repo=$(pwd)
for i in $(seq 1 10); do
  create_commit
  # Create tags to make git-log decorations for the grafted commit more involved
  git tag -a -m "$i" "tag_$i"
  git perf add -m test-measure 5
done
git perf report -n 5
git perf report -n 20
cd "$(mktemp -d)"
git clone "file://$full_repo" --depth=2 shallow_clone
cd shallow_clone
git perf pull
git perf report -n 1
output=$(git perf report -n 10 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow clone'* ]]; then
  echo Missing warning for 'shallow clone'
  echo "$output"
  exit 1
fi
output=$(git perf audit -n 10 -m test-measure 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow clone'* ]]; then
  echo Missing warning for 'shallow clone'
  echo "$output"
  exit 1
fi
