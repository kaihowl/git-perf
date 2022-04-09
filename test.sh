#!/bin/bash

set -e
PS4='${LINENO}: '
set -x

shopt -s nocasematch

# TODO(kaihowl) add output expectations for report use cases (maybe add csv output)
# TODO(kaihowl) add warning for shallow repos
# TODO(kaihowl) running without a git repo as current working directory
# TODO(kaihowl) allow pushing to different remotes

script_dir=$(pwd)

export PYTHONOPTIMIZE=TRUE

export GIT_COMMITTER_EMAIL="<>"
export GIT_COMMITTER_NAME="GitHub Actions Test"
export GIT_AUTHOR_EMAIL="<>"
export GIT_AUTHOR_NAME="GitHub Actions Test"

PATH=${script_dir}/:$PATH
python3 -m pip install -r "${script_dir}/requirements.txt"

function cd_empty_repo() {
  local tmpgit
  tmpgit="$(mktemp -d)"
  pushd "${tmpgit}"
  git init
}

function create_commit() {
  echo "a" >> a
  git add a
  git commit -m 'my commit'
}

function cd_temp_repo() {
  cd_empty_repo
  create_commit
  create_commit
  create_commit
  create_commit
}

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

echo Basic audit tests

cd_temp_repo
git checkout HEAD~3
git perf add -m timer 1
git checkout - && git checkout HEAD~2
git perf add -m timer 2
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

echo Stable measurements with zero stddev
cd_empty_repo
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 3
git perf audit -m timer
create_commit
git perf add -m timer 4
git perf audit -m timer && exit 1

echo Check audit with different measurements available
cd_temp_repo
echo No measurements available
git perf audit -m timer && exit 1
echo Only HEAD measurement available
git perf add -m timer 3
git perf audit -m timer
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 4
git checkout -
git perf audit -m timer
echo Two historical measurements available
git checkout HEAD~2
git perf add -m timer 3.5
git checkout -
git perf audit -m timer

cd_temp_repo
echo Only one historical measurement available
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf audit -m timer && exit 1

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

echo Only single historical measurement available, should accept new measurement
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 3
git checkout -
git perf add -m timer 4
git perf audit -m timer

echo Two historical measurements available, and acceptable new measurement
cd_temp_repo
git checkout HEAD~2
git perf add -m timer 3
git checkout -
git checkout HEAD~1
git perf add -m timer 4
git checkout -
git perf add -m timer 5
git perf audit -m timer

echo New measure with selector, only historical measurements with a different selector
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv otherselector=test
git checkout -
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

echo New measure with selector, only historical measurements with the same selector but different value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv myselector=other
git checkout -
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

echo New non-matching measures, only historical measurements with matching key and value
cd_temp_repo
git checkout HEAD~1
git perf add -m timer 4 -kv myselector=test
git checkout -
git perf add -m timer 4
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv otherselector=test
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv myselector=other
git perf audit -m timer -s myselector=test && exit 1
git perf add -m timer 4 -kv myselector=test
git perf audit -m timer -s myselector=test

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
git perf report -o separated_result.html -s separated_result.html
git perf report -o single_result.html -m timer
git perf report -o separated_single_result.html -m timer -s separated_single_result.html

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


exit 0

