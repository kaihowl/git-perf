#!/bin/bash

set -euxo pipefail

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

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

echo ---- Test case 1: Conflicting on single commit.

echo Setup both working copies to have a single shared, upstream commit, which already has a single measurement on it
bare_repo_and_two_clones
pushd repo1
create_commit
git perf add -m echo 0.5 -k repo=original
git push
git perf push
popd
pushd repo2
git pull
popd


echo In first working copy, add a perf measurement, but do not push it
pushd repo1
git perf pull
git perf add -m echo 0.5 -k repo=first
popd

echo In the second working copy, also add a perf measurement to cause a conflict
pushd repo2
git perf pull
git perf add -m echo 0.5 -k repo=second
popd

echo Then push from first working copy
pushd repo1
git perf push
popd

echo Pushing in second working copy should fail and require a pull first
pushd repo2
git perf push && exit 1
git perf report -o -
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 2 ]] || exit 1
git perf pull
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 3 ]] || exit 1
git perf push
popd

echo In the first working copy, we should see all three measurements now
pushd repo1
git perf pull
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 3 ]] || exit 1
popd

echo ---- Test case 2: No conflicts but merge needed.

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
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 1 ]] || exit 1
git push
# There is a conflict, we must pull first
git perf push && exit 1
git perf pull
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 2 ]] || exit 1
git perf push
popd

echo In the first working copy, we should also see both measurements on separate commits now
pushd repo1
git pull
git perf pull
num_measurements=$(git perf report -o - | wc -l)
[[ ${num_measurements} -eq 2 ]] || exit 1
popd


exit 0
