#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Test case 1: Conflicting on single commit.
# Test case 2: No conflicts but merge needed.

# When run, creates and changes into a temp folder with the following structure:
# - original/  # bare, upstream repo
# - repo1/     # First working copy
# - repo2/     # Second working copy
function bare_repo_and_two_clones() {
  cd "$(mktemp -d)"
  mkdir original
  cd original
  git init --bare
  cd ..

  git clone original repo1
  git clone original repo2
}

echo Test case 1: Conflicting on single commit.

echo Setup both working copies to have a single shared, upstream commit
bare_repo_and_two_clones
pushd repo1
create_commit
git push
popd
pushd repo2
git pull
popd


echo In first working copy, add a perf measurement and push it
pushd repo1
git perf add -m echo 0.5 -k repo=first
git perf push
popd

echo In the second working copy, also add a perf measurement to cause a conflict
pushd repo2
git perf add -m echo 0.5 -k repo=second
# There is a conflict, we must pull first
git perf push && exit 1
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
git perf pull
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
git perf push
popd

echo In the first working copy, we should also see both measurements now
pushd repo1
git perf pull
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
popd

echo Test case 2: No conflicts but merge needed.

echo Setup empty upstream
bare_repo_and_two_clones

echo In first working copy, add a commit with a perf measurement, publish commit and perf
pushd repo1
create_commit
git perf add -m echo 0.5 -k repo=first
git push
git perf push
popd

echo "In the second working copy, pull (without perf), add commit with perf, publish commit and perf"
pushd repo2
git pull
create_commit
git perf add -m echo 0.5 -k repo=second
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
git push
# There is a conflict, we must pull first
git perf push && exit 1
git perf pull
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
git perf push
popd

echo In the first working copy, we should also see both measurements on separate commits now
pushd repo1
git pull
git perf pull
out=$(mktemp).csv && git perf report -o "$out" && cat "$out"
popd


exit 0
