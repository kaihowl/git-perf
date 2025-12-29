#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "Setup repository and add measurements"

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

test_section "Verify temporary refs after add operations"

ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)
assert_equals "$ref_count" "2" "Expected the symbolic write-ref and the target write ref after initial add(s)"

test_section "Verify temporary refs persist after report"

git perf report
ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)
assert_equals "$ref_count" "2" "Expected the symbolic write-ref and the target write ref to be present after report"

test_section "Verify temporary refs cleaned up after push"

git perf push
ref_count=$(git for-each-ref '**/notes/perf-*' | wc -l)
assert_equals "$ref_count" "1" "Expected only the permanent git perf ref after first push"

popd

test_stats
exit 0

