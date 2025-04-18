#!/bin/bash

set -e
set -x

# TODO(kaihowl) add output expectations for report use cases (based on markdown?)
# TODO(kaihowl) running without a git repo as current working directory
# TODO(kaihowl) allow pushing to different remotes

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

## Check git perf prune functionality

# Refuse to run on a shallow clone
pushd "$(mktemp -d)"
repo=$(pwd)
git init --bare
popd

pushd "$(mktemp -d)"
git clone "$repo" work/
cd work/
create_commit
create_commit
create_commit
git push
popd

pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --no-tags --prune --progress --no-recurse-submodules --depth=1 --update-head-ok origin master:master
output=$(git perf prune 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow'* ]]; then
  echo No warning for 'shallow' clone
  echo "$output"
  exit 1
fi
popd

# Normal operations on main repo
pushd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --update-head-ok origin master:master

git perf add -m test 5

# Must push once to create the remote perf branch
git perf push

# TODO(kaihowl) probably should be deprecated and subsumed into the normal remove operation
git perf prune

nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo "Expected to have 1 note but found '$nr_notes' instead"
  exit 1
fi

git reset --hard HEAD~1
git push --force origin master:master

nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo "Expected to have 1 note but found '$nr_notes' instead"
  exit 1
fi
git reflog expire --expire-unreachable=now --all
git prune --expire=now
git perf prune
nr_notes=$(git notes --ref=refs/notes/perf-v3 list | wc -l)
if [[ $nr_notes -ne 0 ]]; then
  echo "Expected to have no notes but found '$nr_notes' instead"
  exit 1
fi

popd
