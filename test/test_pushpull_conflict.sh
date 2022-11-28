#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo Setup a bare remote with a single commit
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

echo Checkout two working copies
cd "$root"
git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo In first working copy, add a measurement the commit and push it
cd "$repo1"
git perf add -m echo 0.5 -k repo=first
git perf push

echo In the second working copy, add a measurement as well
cd "$repo2"
git perf add -m echo 0.5 -k repo=second
git perf push && exit 1
git perf pull
git perf push

# TODO(kaihowl) debug
cd "$repo1"
out=$(mktemp).csv
git perf pull
git perf report -o "$out"
cat "$out"


exit 0
