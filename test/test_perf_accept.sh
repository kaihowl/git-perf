#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Check perf-accept functionality
cd_empty_repo
create_commit
git perf add -m test 2 -kv os=ubuntu
create_commit
git perf add -m test 4 -kv os=ubuntu
create_commit
git perf add -m test 5000 -kv os=ubuntu
git perf audit -m test -s os=ubuntu && exit 1
# Accept regression for other platform
git perf good -m test -kv os=macOS
# Must not accept regression for this platform
git perf audit -m test -s os=ubuntu && exit 1
# TODO(kaihowl) Do we need to seperate kvs from mere labels?
# Must not accept regression when no platform specified?
# git perf audit -m test && exit 1
# Accept regression on this platform
git perf good -m test -kv os=ubuntu
git perf audit -m test -s os=ubuntu
git perf audit -m test
create_commit
git perf add -m test 5010
git perf audit -m test

# Check when using non-existent selectors for trailers
cd_empty_repo
create_commit
git perf add -m test 2 -kv os=ubuntu
create_commit
git perf add -m test 3 -kv os=ubuntu
create_commit
git perf add -m test 10 -kv os=ubuntu
git perf audit -m test -s os=ubuntu && exit 1
git perf good -m someother
# This can fail is filtering with "os" on the trailers without any kvs
# is done incorrectly.
git perf audit -m test -s os=ubuntu && exit 1
git perf good -m test
git perf audit -m test -s os=ubuntu

# Check perf-accept functionality (base case)
# Only accept performance regressions if non-merge HEAD commit has corresponding trailer
cd_empty_repo
create_commit
git perf add -m test 2
create_commit
git perf add -m test 3
# This trailer should not count!
git perf good -m test
create_commit
git perf add -m test 10
git perf audit -m test -d 1 && exit 1
git perf good -m test
git perf audit -m test -d 1

# Check perf-accept functionality (merge case)
# Only accpet performance regressions if freshly merged branch contains trailer
cd_empty_repo
create_commit
git perf add -m test 2
create_commit
# This trailer should not contribute and make measurements acceptable
git perf good -m test
git perf add -m test 3
git checkout -b feature
create_commit
create_commit
git checkout -
git merge --no-ff -
git perf add -m test 10000
git perf audit -m test && exit 1
# Undo merge, back to feature branch
git reset --hard HEAD~1
git checkout -
git perf good -m test
git checkout -
git merge --no-ff -
git perf add -m test 10000
git perf audit -m test


# Test for duplicated trailers
cd_empty_repo
create_commit
git perf good -m test-measure
nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
if [[ $nr_git_trailers != 1 ]]; then
  echo "Expected exactly one git trailer 'accept-perf' but found $nr_git_trailers"
  exit 1
fi
# Second invocation for the same git trailer
nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
if [[ $nr_git_trailers != 1 ]]; then
  echo "Expected exactly one git trailer 'accept-perf' but found $nr_git_trailers"
  exit 1
fi
git perf good -m test-measure -kv os=ubuntu
nr_git_trailers=$(git show HEAD | grep -c 'accept-perf')
if [[ $nr_git_trailers != 2 ]]; then
  echo "Expected exactly two git trailers 'accept-perf' but found $nr_git_trailers"
  exit 1
fi
