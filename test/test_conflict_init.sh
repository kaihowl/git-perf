#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" myworkrepo
pushd myworkrepo
myworkrepo=$(pwd)

touch a
git add a
git commit -m 'first commit'

git push

popd

git clone "$orig" repo1
git clone "$orig" repo2
repo1=$(pwd)/repo1
repo2=$(pwd)/repo2

echo Init git perf in two repos independently
pushd "$repo1"

git perf add -m echo 0.5

git perf push

popd

pushd "$repo2"

git perf add -m echo 0.5

output=$(git perf push 2>&1 1>/dev/null)
assert_output_contains "$output" "retrying" "Output is missing 'retrying'"

popd

echo "Check number of measurements from myworkrepo"
pushd "$myworkrepo"

git perf pull
num_measurements=$(git perf report -o -  | wc -l)
# CSV now includes header row, so 2 measurements + 1 header = 3 lines
if [[ $num_measurements -ne 3 ]]; then
  echo "Expected two measurements (3 lines with header), but have $num_measurements lines:"
  git perf report -o -
  exit 1
fi

popd


exit 0
