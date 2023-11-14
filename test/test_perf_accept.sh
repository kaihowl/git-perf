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

# Check perf-accept functionality (merge case 2)
# Accept performance regression on epoch bump (from main branch)
cd_empty_repo
# On main branch
create_commit
git perf add -m test 2
create_commit
git rev-parse HEAD
git perf add -m test 3
# Feature branch
git checkout -b feature
create_commit
git rev-parse HEAD
git perf add -m test 2
# Back to main branch, add out-of-range measurement
git checkout -
# TODO(kaihowl) introduce random into create_commit
sleep 1
create_commit
git rev-parse HEAD
git perf add -m test 10000
git perf audit -m test && exit 1
# Go to feature branch
git checkout -
git merge --no-ff -
git perf add -m test 10000
git perf audit -m test && exit 1
# Undo merge
git reset --hard HEAD~1
# Back to main branch and bump epoch
git checkout -
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# To feature branch
git checkout -
git merge --no-ff -
git perf add -m test 1000
git perf audit -m test

# Check perf-accept functionality (merge conflict case)
# Ensure that there is a conflict if the merge branch and the feature branch both force an epoch bump
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
git perf add -m test 10000
git perf audit -m test && exit 1
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# Back to main branch, bump epoch
git checkout -
git perf bump-epoch -m test
git add .gitperfconfig
git perf add -m test 10000
git commit --amend --no-edit
# On main branch
# This has to result in a conflict
git merge --no-ff - && exit 1

exit 0
