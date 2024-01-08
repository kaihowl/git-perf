#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check that only first parents for a merged branch are considered
# Create steady (stddev == 0) measurement on main branch with flaky
# results on merged branch's intermediate commits.
cd_empty_repo
create_commit
git perf add -m timer 1
create_commit
git perf add -m timer 1
create_commit
git perf add -m timer 5
# Base test: Expect this to fail
git perf audit -m timer && exit 1
# Reset main branch
git reset --hard HEAD~1
# branch off steady master branch
git checkout -b feature_branch
create_commit
# Bad intermediate result
git perf add -m timer 5
create_commit
# Fixed perf in this commit
git perf add -m timer 1
git checkout master
# Merged feature_branch has ok performance (as flaky intermediate is skipped)
git merge --no-ff -
git perf add -m timer 1
# True performance regression on main branch must fail
# This would not fail if the flaky measurement from the feature branch is considered.
create_commit
git perf add -m timer 2
git perf audit -m timer && exit 1

exit 0
