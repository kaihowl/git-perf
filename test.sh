#!/bin/bash

set -e
PS4='${LINENO}: '
set -x

shopt -s nocasematch

# TODO(kaihowl) add output expectations for report use cases (maybe add csv output)
# TODO(kaihowl) add test for kvs if previous commits do not contain this selector

script_dir=$(pwd)

export PYTHONOPTIMIZE=TRUE

export GIT_COMMITTER_EMAIL="<>"
export GIT_COMMITTER_NAME="GitHub Actions Test"
export GIT_AUTHOR_EMAIL="<>"
export GIT_AUTHOR_NAME="GitHub Actions Test"

PATH=${script_dir}/:$PATH
python3 -m pip install -r "${script_dir}/requirements.txt"

cd "$(mktemp -d)"
root=$(pwd)

mkdir orig
cd orig
orig=$(pwd)

git init

touch a
git add a
git commit -m 'first commit'

touch b
git add b
git commit -m 'second commit'

touch c
git add c
git commit -m 'third commit'


orig=$(pwd)
cd "$root"
git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo Leave one commit in middle without any notes
cd "$repo1"
git checkout master~2
git perf measure -n 2 -m echo echo test
git checkout master
git perf add -m echo 0.5

git perf push

echo Print from second repo
cd "$repo2"
git perf pull
git perf report -o result.html


function cd_temp_repo() {
  local tmpgit
  tmpgit="$(mktemp -d)"
  pushd "${tmpgit}"
  git init
  touch a
  git add a
  git commit -m 'first commit'

  touch b
  git add b
  git commit -m 'second commit'

  touch c
  git add c
  git commit -m 'third commit'

  touch d
  git add d
  git commit -m 'fourth commit'
}

cd_temp_repo
output=$(git perf measure -m test-measure 2>&1 1>/dev/null) && exit 1
if [[ ${output} != *'following arguments'* ]]; then
  echo Missing 'following arguments' in output:
  echo "$output"
  exit 1
fi

echo Add invalid measurements

echo Empty measurement
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "\n"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'too few items'* ]]; then
  echo Missing 'too few items' in output:
  echo "$output"
  exit 1
fi

echo Measurement with just date
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "$(date +%s)"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'too few items'* ]]; then
  echo Missing 'too few items' in output:
  echo "$output"
  exit 1
fi

echo Measurement without date
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement $RANDOM key=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ ${output} != *'found non-numeric value'* ]]; then
  echo Missing 'found non-numeric value' in output:
  echo "$output"
  exit 1
fi

echo Measurement without kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo There should be no output in stderr but instead there is:
  echo "$output"
  exit 1
fi

echo Measurement with invalid kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM test othertest stuff"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo There should be no output in stderr but instead there is:
  echo "$output"
  exit 1
fi

echo Measurement valid but with too many spaces
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement    $(date +%s)      $RANDOM key=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo There should be no output in stderr but instead there is:
  echo "$output"
  exit 1
fi

echo Duplicate kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM key=value key=value"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo There should be no output in stderr but instead there is:
  echo "$output"
  exit 1
fi

echo Conflicting kvs
cd_temp_repo
git perf add -m echo 0.5
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM key=value key=value2"
output=$(git perf report 2>&1 1>/dev/null)
if [[ -n ${output} ]]; then
  echo There should be no output in stderr but instead there is:
  echo "$output"
  exit 1
fi

# Basic audit tests

cd_temp_repo
git checkout HEAD~2
git perf add -m timer 1
git checkout - && git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf add -m timer 4
# mean: 2, std: 1
git perf audit -m timer -d 4
git perf audit -m timer -d 3
git perf audit -m timer -d 2
git perf audit -m timer -d 1.9999 && exit 1
git perf audit -m timer -d 1 && exit 1

cd_temp_repo
echo No measurements available
git perf audit -m timer
echo Only HEAD measurement available
git perf add -m timer 3
git perf audit -m timer
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf audit -m timer
echo Two historical measurements available
git checkout HEAD~2
git perf add -m timer 3
git checkout -
git perf audit -m timer

cd_temp_repo
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf audit -m timer

echo Only measurements for different value available
cd_temp_repo
git checkout HEAD~1
git perf add -m othertimer 3
git checkout -
git perf add -m othertimer 3
git perf audit -m timer && exit 1
echo New measurement for HEAD but only historical measurements for different measurements
git perf add -m timer 3
git perf audit -m timer

exit 0

