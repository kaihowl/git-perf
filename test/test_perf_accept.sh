#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
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
assert_failure git perf audit -m test -s os=ubuntu
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
assert_failure git perf audit -m test -s os=ubuntu
git perf bump-epoch -m someother
git add .gitperfconfig
git commit --amend --no-edit

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
git checkout master
git merge --no-ff -
git perf add -m test 10000
assert_failure git perf audit -m test
# Undo merge, back to feature branch, bump epoch
git reset --hard HEAD~1
git checkout feature
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# Back to master branch
git checkout master
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
# Back to master branch, add out-of-range measurement
git checkout master
create_commit
git rev-parse HEAD
git perf add -m test 10000
assert_failure git perf audit -m test
# Go to feature branch
git checkout feature
git merge --no-ff -
git perf add -m test 10000
assert_failure git perf audit -m test
# Undo merge
git reset --hard HEAD~1
# Back to master branch and bump epoch
git checkout master
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# To feature branch
git checkout feature
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
assert_failure git perf audit -m test
git perf bump-epoch -m test
git add .gitperfconfig
git commit --amend --no-edit
# Back to master branch, bump epoch
git checkout master
git perf bump-epoch -m test
git add .gitperfconfig
git perf add -m test 10000
git commit --amend --no-edit
# On main branch
# This has to result in a conflict
assert_failure git merge --no-ff -

test_stats
exit 0
