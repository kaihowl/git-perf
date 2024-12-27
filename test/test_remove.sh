#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

if [[ $(uname -s) = Darwin ]]; then
  export DYLD_FORCE_FLAT_NAMESPACE=1
  export DYLD_INSERT_LIBRARIES=/opt/homebrew/lib/faketime/libfaketime.1.dylib
else
  echo "not implemented"
  exit 1
fi

cd_temp_repo

# --- 14 days ago
export FAKETIME='-14d'

echo Add measurement on commit in the past
create_commit 
git perf add -m test-measure 10.0
num_measurements=$(git perf report -o - | wc -l)
# Exactly one measurement should be present
[[ ${num_measurements} -eq 1 ]] || exit 1


# TODO(kaihowl) specify >= or > precisely
echo "Remove measurements on commits older than 10 days"
git perf remove --older-than 10d
num_measurements=$(git perf report -o - | wc -l)
# Nothing should have been removed
[[ ${num_measurements} -eq 1 ]] || exit 1

# --- NOW
export FAKETIME=

echo "Add a commit with a newer measurement"
create_commit
git perf add -m test-measure 20.0
num_measurements=$(git perf report -o - | wc -l)
# Two measurements should be there
[[ ${num_measurements} -eq 2 ]] || exit 1

echo "Remove older than 10 days measurements"
git perf remove --older-than 10d
num_measurements=$(git perf report -o - | wc -l)
# One measurement should still be there
[[ ${num_measurements} -eq 1 ]] || exit 1
# The measurement should be 20.0
git perf report -o - | grep '20\.0'


exit 0
