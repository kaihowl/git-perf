#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Pull in repo without a remote
cd_empty_repo

output="$(git perf pull 2>&1 1>/dev/null)" && exit 1
if [[ $output != *"No upstream found"* ]]; then
  echo "Missing 'No upstream found' from output:"
  echo "$output"
  exit 1
fi

echo Pull from remote without measurements

cd "$(mktemp -d)"
root=$(pwd)

mkdir orig
cd orig
orig=$(pwd)

git init --bare

cd "$(mktemp -d)"
git clone "$orig" myworkrepo

cd myworkrepo

touch a
git add a
git commit -m 'first commit'

git push

# TODO(kaihowl) move functionality for check for output in common function
output="$(git perf pull 2>&1 1>/dev/null)" && exit 1
if [[ $output != *'Remote repository is empty or has never been pushed to'* ]]; then
  echo "Missing 'Remote repository is empty or has never been pushed to' in output:"
  echo "$output"
  exit 1
fi

cd "$root"
git clone "$orig" repo1
repo1=$(pwd)/repo1

cd "$repo1"

git perf add -m test-measure 12
git perf push

output=$(git perf pull 2>/dev/null) || exit 1
if [[ $output != *'Already up to date'* ]]; then
  echo "Missing 'Already up to date' in output:"
  echo "$output"
  exit 1
fi

echo "Pulling from remote with measurements should have output"

cd "$root"
git clone "$orig" repo2
repo2=$(pwd)/repo2

cd "$repo2"

output="$(git perf pull 2>/dev/null)" || exit 1
if [[ $output != *'[new ref]'* ]]; then
  echo "Missing '[new ref]' in output:"
  echo "$output"
  exit 1
fi

exit 0
