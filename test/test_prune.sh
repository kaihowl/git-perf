#!/bin/bash

set -e
set -x

# TODO(kaihowl) add output expectations for report use cases (based on markdown?)
# TODO(kaihowl) running without a git repo as current working directory
# TODO(kaihowl) allow pushing to different remotes

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Check git perf prune functionality

# Refuse to run on a shallow clone
cd_temp_repo
repo=$(pwd)

create_commit
create_commit
create_commit
cd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --no-tags --prune --progress --no-recurse-submodules --depth=1 --update-head-ok origin master:master
output=$(git perf prune 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow'* ]]; then
  echo No warning for 'shallow' clone
  echo "$output"
  exit 1
fi

# Normal operations on main repo
cd_temp_repo
create_commit
git perf add -m test 5
git perf prune
nr_notes=$(git notes --ref=refs/notes/perf-v2 list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo "Expected to have 1 note but found '$nr_notes' instead"
  exit 1
fi
git reset --hard HEAD~1
nr_notes=$(git notes --ref=refs/notes/perf-v2 list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo "Expected to have 1 note but found '$nr_notes' instead"
  exit 1
fi
git reflog expire --expire-unreachable=now --all
git prune --expire=now
git perf prune
nr_notes=$(git notes --ref=refs/notes/perf-v2 list | wc -l)
if [[ $nr_notes -ne 0 ]]; then
  echo "Expected to have no notes but found '$nr_notes' instead"
  exit 1
fi
