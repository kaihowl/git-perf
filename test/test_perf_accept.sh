#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check perf-accept functionality
cd_empty_repo
create_commit
git perf add -m test 2 -k os=ubuntu
create_commit
git perf add -m test 4 -k os=ubuntu
create_commit
git perf add -m test 5000 -k os=ubuntu
git perf audit -m test -s os=ubuntu && exit 1
# Accept regression on this platform
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
git perf add -m test 5000 -k os=ubuntu
git perf audit -m test -s os=ubuntu
git perf audit -m test
create_commit
git perf add -m test 5010
git perf audit -m test

# Check when using non-existent selectors for trailers
cd_empty_repo
create_commit
git perf add -m test 2 -k os=ubuntu
create_commit
git perf add -m test 3 -k os=ubuntu
create_commit
git perf add -m test 10 -k os=ubuntu
git perf audit -m test -s os=ubuntu && exit 1
git perf bump-epoch -m someother
git add .gitperfconfig
git commit --amend --no-edit

# TODO(kaihowl) needs complete rework. no longer valid.

# Check perf-accept functionality (merge case)
# Accept performance regression on epoch bump (from feature branch)
cd_empty_repo
# On main branch
create_commit
git perf add -m test 2
create_commit
git perf add -m test 3
# Feature branch
git checkout -b feature
create_commit
create_commit
# Attempt to merge to main
git checkout -
git merge --no-ff -
git perf add -m test 10000
git perf audit -m test && exit 1
# Undo merge, back to feature branch, bump epoch
git reset --hard HEAD~1
git checkout -
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# Back to main branch
git checkout -
git merge --no-ff -
git perf add -m test 10000
git perf audit -m test


# # Test for duplicated trailers
# cd_empty_repo
# create_commit
# git perf good -m test-measure
# nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
# if [[ $nr_git_trailers != 1 ]]; then
#   echo "Expected exactly one git trailer 'accept-perf' but found $nr_git_trailers"
#   exit 1
# fi
# # Second invocation for the same git trailer
# nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
# if [[ $nr_git_trailers != 1 ]]; then
#   echo "Expected exactly one git trailer 'accept-perf' but found $nr_git_trailers"
#   exit 1
# fi
# git perf good -m test-measure -k os=ubuntu
# nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
# if [[ $nr_git_trailers != 2 ]]; then
#   echo "Expected exactly two git trailers 'accept-perf' but found $nr_git_trailers"
#   exit 1
# fi

exit 0
