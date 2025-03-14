#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" myworkrepo
pushd myworkrepo
myworkrepo=$(pwd)

touch a
git add a
git commit -m 'first commit'

git push

popd

git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo Init git perf in two repos independently
pushd "$repo1"
# TODO(kaihowl)
# Only setting the values with envvars fails for libgit2 git_signature_default
git config user.name "$GIT_COMMITTER_NAME"
git config user.email "$GIT_COMMITTER_EMAIL"

git perf add -m echo 0.5

git perf push

popd

pushd "$repo2"
# TODO(kaihowl)
# Only setting the values with envvars fails for libgit2 git_signature_default
git config user.name "$GIT_COMMITTER_NAME"
git config user.email "$GIT_COMMITTER_EMAIL"

git perf add -m echo 0.5

git perf push

popd

echo "Check number of measurements from myworkrepo"
pushd "$myworkrepo"

git perf pull
num_measurements=$(git perf report -o -  | wc -l)
if [[ $num_measurements -ne 2 ]]; then
  echo "Expected two measurements, but only have these:"
  git perf report -o -
  exit 1
fi

popd


exit 0
