#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Shallow clone warning tests - basic setup"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" full_repo
pushd full_repo

for i in $(seq 1 10); do
  create_commit
  # Create tags to make git-log decorations for the grafted commit more involved
  git tag -a -m "$i" "tag_$i"
  git perf add -m test-measure 5
done

git perf report -n 5
git perf report -n 20

git push
git perf push

popd

test_section "Shallow clone warnings with limited history"

git clone "file://$orig" --depth=2 shallow_clone
pushd shallow_clone
git perf pull
assert_success git perf report -n 2
assert_success git perf audit -n 2 -m test-measure
assert_failure_with_output output git perf report -n 3
assert_contains "$output" "shallow clone" "Missing warning for 'shallow clone'"
assert_failure_with_output output git perf audit -n 3 -m test-measure
assert_contains "$output" "shallow clone" "Missing warning for 'shallow clone'"

popd

# CONSTRAINT: Must use a bare remote repository, not a working copy
# When pulling measurements, the remote must be a bare repository. Using a working copy
# as a remote will cause test failures. The original test case failed because it used
# a different working copy as a remote instead of a bare remote.

test_section "Shallow clone with merge commit - first parent history"

# The shallow warning for a PR-branch with a merge as HEAD should be counting the first parent's history.
# This is already the default behavior for git-fetch with the depth option.

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "file://$orig" full_repo
pushd full_repo
for i in $(seq 1 10); do
  create_commit
  git perf add -m test-measure 5
done
git checkout -b feature_branch
for i in $(seq 1 5); do
  create_commit
done
git checkout master

# Simulate temp merge branch on GitHub
git merge --no-ff -
git push
git perf push

# Test first-parent fetch-depth behavior even if HEAD is non-merge commit.
commit=$(git rev-parse HEAD)

# Shallow clone with depth == 10 for main branch
cd "$(mktemp -d)"
git init
git remote add origin "file://$orig"
# Simulate the behavior of github actions checkout closely
git fetch origin "$commit:local-ref" --depth=10
git checkout local-ref
git perf pull

# Must fail as this expects more history
assert_failure_with_output output git perf report -n 11
assert_contains "$output" "shallow clone" "Missing warning for 'shallow clone'"

# Must work as this is the exact history length
# If we erroneously considered the feature_branch's history, it would be filtered
# out and we end up with fewer than 10 commits when following the first parent.
assert_success git perf report -n 10

test_stats
exit 0
