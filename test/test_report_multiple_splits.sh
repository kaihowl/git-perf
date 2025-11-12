#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

cd_empty_repo
create_commit

# Create measurements with multiple dimensions (os and arch)
git perf add -m timer 1.0 -k os=ubuntu -k arch=x64
git perf add -m timer 0.9 -k os=ubuntu -k arch=x64
git perf add -m timer 1.5 -k os=ubuntu -k arch=arm64
git perf add -m timer 1.4 -k os=ubuntu -k arch=arm64
git perf add -m timer 1.2 -k os=mac -k arch=arm64
git perf add -m timer 1.1 -k os=mac -k arch=arm64

create_commit
git perf add -m timer 2.0 -k os=ubuntu -k arch=x64
git perf add -m timer 2.1 -k os=ubuntu -k arch=x64
git perf add -m timer 2.5 -k os=ubuntu -k arch=arm64
git perf add -m timer 2.4 -k os=ubuntu -k arch=arm64
git perf add -m timer 2.2 -k os=mac -k arch=arm64
git perf add -m timer 2.3 -k os=mac -k arch=arm64

create_commit
git perf add -m timer 3.0 -k os=ubuntu -k arch=x64
git perf add -m timer 3.1 -k os=ubuntu -k arch=x64
git perf add -m timer 3.5 -k os=ubuntu -k arch=arm64
git perf add -m timer 3.4 -k os=ubuntu -k arch=arm64
git perf add -m timer 3.2 -k os=mac -k arch=arm64
git perf add -m timer 3.3 -k os=mac -k arch=arm64

# Test single split (backward compatibility)
git perf report -o single_split.html -s os
single_split_content=$(cat single_split.html)
assert_output_contains "$single_split_content" "ubuntu" "Single split HTML missing 'ubuntu' label"
assert_output_contains "$single_split_content" "mac" "Single split HTML missing 'mac' label"

# Test multiple splits
git perf report -o multi_split.html -s os -s arch
multi_split_content=$(cat multi_split.html)

# Verify combined group labels are present
assert_output_contains "$multi_split_content" "ubuntu/x64" "Multi-split HTML missing 'ubuntu/x64' label"
assert_output_contains "$multi_split_content" "ubuntu/arm64" "Multi-split HTML missing 'ubuntu/arm64' label"
assert_output_contains "$multi_split_content" "mac/arm64" "Multi-split HTML missing 'mac/arm64' label"

# Verify measurement name is still present
assert_output_contains "$multi_split_content" "timer" "Multi-split HTML missing 'timer' measurement name"

# Test that measurements without all split keys are excluded
git perf add -m timer 5.0 -k os=windows  # Missing arch key
output=$(git perf report -s os -s arch -o missing_key_test.html 2>&1) || true
missing_key_content=$(cat missing_key_test.html)

# The windows measurement should NOT appear since it doesn't have the arch key
if grep -q "windows" <<< "$missing_key_content"; then
  echo "Multi-split should not include measurements missing split keys"
  exit 1
fi

# Test CSV output with multiple splits
git perf report -o multi_split.csv -s os -s arch
if [[ ! -f multi_split.csv ]]; then
  echo "Expected CSV file 'multi_split.csv' was not created"
  exit 1
fi

# Test with aggregation
git perf report -o multi_split_agg.html -s os -s arch -a median
multi_split_agg_content=$(cat multi_split_agg.html)
assert_output_contains "$multi_split_agg_content" "ubuntu/x64" "Aggregated multi-split HTML missing 'ubuntu/x64' label"

echo "All multiple splits tests passed!"
