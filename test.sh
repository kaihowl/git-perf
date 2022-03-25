#!/bin/bash

set -e
set -x

script_dir=$(pwd)

cd $(mktemp -d)
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
cd $root
git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo Leave one commit in middle without any notes
cd $repo1
git checkout master~2
${script_dir}/measure.sh
${script_dir}/measure.sh
git checkout master
${script_dir}/measure.sh

echo Print from second repo
cd $repo2

echo from hereon
git --no-pager log --no-color --pretty='oneline'
${script_dir}/report.sh
echo $(pwd)/result.html
