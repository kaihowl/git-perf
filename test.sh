#!/bin/bash

set -e
set -x

script_dir=$(pwd)

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
"${script_dir}/measure.sh"
"${script_dir}/measure.sh"
git checkout master
"${script_dir}/measure.sh"

echo Add invalid measurements
# Empty measurement
"${script_dir}/measure.sh" ""
# Measurement with just date
"${script_dir}/measure.sh" "$(date +%s)"
# Measurement without date
"${script_dir}/measure.sh" "myothermeasurement $RANDOM key=value"
# Measurement without kvs
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM"
# Measurement with invalid kvs
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM test othertest stuff"
# Measurement valid but with too many spaces
"${script_dir}/measure.sh" "myothermeasurement    $(date +%s)      $RANDOM key=value"
# Duplicate kvs
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM key=value key=value"
# Conflicting kvs
"${script_dir}/measure.sh" "myothermeasurement $(date +%s) $RANDOM key=value key=value2"
#
echo Print from second repo
cd "$repo2"

echo from hereon
git --no-pager log --no-color --pretty='oneline'
"${script_dir}/report.sh"
echo "$(pwd)/result.html"
