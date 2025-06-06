#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
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
# TODO(kaihowl)
# Only setting the values with envvars fails for libgit2 git_signature_default
git config user.name "$GIT_COMMITTER_NAME"
git config user.email "$GIT_COMMITTER_EMAIL"

git checkout master~2
git perf add -m echo 0.5
git perf add -m echo 0.5
git checkout master
git perf add -m echo 0.5

output=$(git perf push)
if [[ ${output} != *'new reference'* ]]; then
  echo "Missing 'new reference' in output:"
  echo "$output"
  exit 1
fi

# Second git perf push should be no-op
git perf push

echo Print from second repo
cd "$repo2"
# TODO(kaihowl)
# Only setting the values with envvars fails for libgit2 git_signature_default
git config user.name "$GIT_COMMITTER_NAME"
git config user.email "$GIT_COMMITTER_EMAIL"
git perf pull
git perf report -o result.html

exit 0
