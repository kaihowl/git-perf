#!/bin/bash

set -e
set -x

# TODO(kaihowl) add output expectations for report use cases (based on markdown?)
# TODO(kaihowl) running without a git repo as current working directory
# TODO(kaihowl) allow pushing to different remotes

script_dir=$(dirname "$0")
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo New repo, error out without crash
cd_empty_repo
output=$(git perf report 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements found'* ]]; then
  echo Missing 'no performance measurements found' in output:
  echo "$output"
  exit 1
fi
output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements'* ]]; then
  echo Missing 'no performance measurements' in output:
  echo "$output"
  exit 1
fi

echo New repo, single commit, error out without crash
cd_empty_repo
create_commit
output=$(git perf report 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements found'* ]]; then
  echo Missing 'no performance measurements found' in output:
  echo "$output"
  exit 1
fi
output=$(git perf audit -m non-existent 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements'* ]]; then
  echo Missing 'no performance measurements found' in output:
  echo "$output"
  exit 1
fi

cd_temp_repo
full_repo=$(pwd)
for i in $(seq 1 10); do
  create_commit
  # Create tags to make git-log decorations for the grafted commit more involved
  git tag -a -m "$i" "tag_$i"
  git perf add -m test-measure 5
done
git perf report -n 5
git perf report -n 20
cd "$(mktemp -d)"
git clone "file://$full_repo" --depth=2 shallow_clone
cd shallow_clone
git perf pull
git perf report -n 1
output=$(git perf report -n 10 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow clone'* ]]; then
  echo Missing warning for 'shallow clone'
  echo "$output"
  exit 1
fi
output=$(git perf audit -n 10 -m test-measure 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow clone'* ]]; then
  echo Missing warning for 'shallow clone'
  echo "$output"
  exit 1
fi

cd_empty_repo
create_commit
git perf add -m timer 1 -kv os=ubuntu
git perf add -m timer 0.9 -kv os=ubuntu
git perf add -m timer 1.2 -kv os=mac
git perf add -m timer 1.1 -kv os=mac
create_commit
git perf add -m timer 2.1 -kv os=ubuntu
git perf add -m timer 2.2 -kv os=ubuntu
git perf add -m timer 2.1 -kv os=mac
git perf add -m timer 2.0 -kv os=mac
create_commit
git perf add -m timer 3.1 -kv os=ubuntu
git perf add -m timer 3.2 -kv os=ubuntu
git perf add -m timer 3.3 -kv os=mac
git perf add -m timer 3.4 -kv os=mac
create_commit
git perf add -m timer 4 -kv os=ubuntu
git perf add -m timer 4 -kv os=ubuntu
git perf add -m timer 4.3 -kv os=mac
git perf add -m timer 4.3 -kv os=mac
git perf add -m timer2 2 -kv os=mac

git perf report -o all_result.html
git perf report -o separated_result.html -s os
git perf report -o single_result.html -m timer
git perf report -o separated_single_result.html -m timer -s os
git perf report -o single_result_different_group.html -m timer -g os

output=$(git perf report -m timer-does-not-exist 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'no performance measurements'* ]]; then
  echo No warning for missing measurements
  echo "$output"
  exit 1
fi

output=$(git perf report -s does-not-exist 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'does-not-exist'* ]]; then
  echo No warning for invalid separator 'does-not-exist'
  echo "$output"
  exit 1
fi

output=$(git perf report -g does-not-exist 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'does-not-exist'* ]]; then
  echo No warning for invalid grouper 'does-not-exist'
  echo "$output"
  exit 1
fi

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

## Check git perf prune functionality

# Refuse to run on a shallow clone
cd_temp_repo
repo=$(pwd)

create_commit
create_commit
create_commit
cd "$(mktemp -d)"
git init
git remote add origin "${repo}"
git fetch --no-tags --prune --progress --no-recurse-submodules --depth=1 --update-head-ok origin master:master
output=$(git perf prune 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'shallow'* ]]; then
  echo No warning for 'shallow' clone
  echo "$output"
  exit 1
fi

# Normal operations on main repo
cd_temp_repo
create_commit
git perf add -m test 5
git perf prune
nr_notes=$(git notes --ref=perf list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo Expected to have 1 note but found "$nr_notes" instead
  exit 1
fi
git reset --hard HEAD~1
nr_notes=$(git notes --ref=perf list | wc -l)
if [[ $nr_notes -ne 1 ]]; then
  echo Expected to have 1 note but found "$nr_notes" instead
  exit 1
fi
git reflog expire --expire-unreachable=now --all
git prune --expire=now
git perf prune
nr_notes=$(git notes --ref=perf list | wc -l)
if [[ $nr_notes -ne 0 ]]; then
  echo Expected to have no notes but found "$nr_notes" instead
  exit 1
fi
