#!/bin/bash

# Disable verbose tracing for cleaner output
export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd "$(mktemp -d)"

mkdir orig
pushd orig
orig=$(pwd)
git init --bare
popd

git clone "$orig" seeding_copy
pushd seeding_copy

for _i in $(seq 1 10); do
  create_commit
  git perf add -m test-measure 5
  git perf add -m test-measure 10
done

ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)

if [[ 2 -ne $ref_count ]]; then
  test_section "Expected the symbolic write-ref and the target write ref to be added after the initial add(s)"
  test_section "Current refs:"
  git for-each-ref '**/notes/perf-*'
  exit 1
fi

git perf report
ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)

if [[ 2 -ne $ref_count ]]; then
  test_section "Expected the symbolic write-ref and the target write ref to be present after the report"
  test_section "Current refs:"
  git for-each-ref '**/notes/perf-*'
  exit 1
fi

git perf push
ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)

if [[ 1 -ne $ref_count ]]; then
  test_section "Expected only the permanent git perf ref to be present after the first push"
  test_section "Current refs:"
  git for-each-ref '**/notes/perf-*'
  exit 1
fi

