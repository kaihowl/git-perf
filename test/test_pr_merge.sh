#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check that only first parents for a merged branch are considered
# Create steady (stddev == 0) measurement on main branch with flaky
# results on merged branch's intermediate commits.
# Producing another flaky result on the main branch after merging will
# only pass the audit if the merged branch's history is considered.
cd_empty_repo
create_commit
git perf add -m timer 1
create_commit
git perf add -m timer 1
create_commit
git perf add -m timer 5
# Base test: Expect this to fail
git perf audit -m timer && exit 1
git checkout HEAD~1
git checkout -b feature_branch
create_commit
# Bad intermediate result
git perf add -m timer 5
create_commit
# Fixed perf in this commit
git perf add -m timer 1
git checkout -
# Merged feature_branch has ok performance
git merge --no-ff -
git perf add -m timer 1
# True performance regression on main branch must fail
create_commit
git perf add -m timer 2
git perf audit -m timer && exit 1

exit 0
