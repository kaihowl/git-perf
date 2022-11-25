#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

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

touch b
git add b
git commit -m 'second commit'

touch c
git add c
git commit -m 'third commit'

git push

cd "$root"
git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo Leave one commit in middle without any notes
cd "$repo1"
git checkout master~2
git perf add -m echo 0.5
git perf add -m echo 0.5
git checkout master
git perf add -m echo 0.5

git perf push

echo Print from second repo
cd "$repo2"
git perf pull
git perf report -o result.html

exit 0
