#!/bin/bash

set -e
PS4='${LINENO}: '
set -x

shopt -s nocasematch

# TODO(kaihowl) extend with "audit" use cases
# TODO(kaihowl) add output expectations for report use cases (maybe add csv output)

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
}


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
