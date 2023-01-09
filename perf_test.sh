#!/bin/bash

set -exuo pipefail
PS4='${BASH_SOURCE}:${LINENO}: '

script_dir=$(pwd)

cd "$(mktemp -d)"

mkdir deep
cd deep

git init

for _i in $(seq 1 100); do
  echo a >> a
  git add a
  git commit -m 'test' -q
  "${script_dir}/perf_test.py"
  git notes --ref=perf add -F test.txt
done

test_repo=$(pwd)


echo "TODO(kaihowl) Test reconcile with our implementation"
cd ..
mkdir shallow
cd shallow

git init
git remote add origin "file://${test_repo}"
git fetch origin master --depth=1
git checkout -b test-branch origin/master

# TODO(kaihowl) debug
export PATH=~/Documents/repos/git-perf/target/Release:$PATH

gtime -v git perf add -m test -k os=something 12.0

# Prefetch to avoid spending the push time mostly on fetching
gtime -v git fetch origin refs/notes/perf

gtime -v git perf push


echo "TODO(kaihowl) Test reconcile with git note implementation"
cd ..
mkdir shallow2
cd shallow2

git init
git remote add origin "file://${test_repo}"
git fetch origin master --depth=1
git checkout -b test-branch origin/master

git notes --ref=perf add -m "test 12345678.0 123 os=ubuntu-20.04"

gtime -v git fetch origin --depth=1 refs/notes/perf
gtime -v git notes --ref perf merge -s cat_sort_uniq FETCH_HEAD
gtime -v git push origin refs/notes/perf:refs/notes/perf


