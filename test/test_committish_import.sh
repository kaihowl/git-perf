#!/bin/bash

# Test committish support for import command
# Tests that measurements can be imported to specific commits using --commit flag

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Helper function to create a simple JUnit XML file
create_junit_xml() {
    local filename="$1"
    cat > "$filename" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="test_suite" tests="3">
    <testcase classname="MyTests" name="test_fast" time="0.001"/>
    <testcase classname="MyTests" name="test_medium" time="0.050"/>
    <testcase classname="MyTests" name="test_slow" time="0.100"/>
  </testsuite>
</testsuites>
EOF
}

# Helper function to create JUnit XML with different values
create_junit_xml_custom() {
    local filename="$1"
    local test1_time="$2"
    local test2_time="$3"
    cat > "$filename" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="custom_suite" tests="2">
    <testcase classname="CustomTests" name="test_one" time="$test1_time"/>
    <testcase classname="CustomTests" name="test_two" time="$test2_time"/>
  </testsuite>
</testsuites>
EOF
}

echo "Test 1: Import to specific commit by SHA"
cd_temp_repo
commit1=$(git rev-parse HEAD~2)
head=$(git rev-parse HEAD)

# Create JUnit XML file
create_junit_xml "junit.xml"

# Import to HEAD~2 using --commit flag
git perf import --commit "$commit1" junit junit.xml

# Verify measurements appear in report from commit1
output=$(git perf report "$commit1" -o -)
assert_output_contains "$output" "test::test_fast" "Test measurement should be in report from commit1"
assert_output_contains "$output" "test::test_medium" "Test measurement should be in report from commit1"
assert_output_contains "$output" "test::test_slow" "Test measurement should be in report from commit1"

# Clean up
rm junit.xml

echo "Test 2: Import to HEAD~N format"
cd_temp_repo
target=$(git rev-parse HEAD~2)

# Create JUnit XML file
create_junit_xml "junit2.xml"

# Import using HEAD~N format
git perf import --commit HEAD~2 junit junit2.xml

# Verify measurements appear in report
output=$(git perf report HEAD~2 -o -)
assert_output_contains "$output" "test::test_fast" "Import should target HEAD~2"

# Clean up
rm junit2.xml

echo "Test 3: Import to branch name"
cd_empty_repo
create_commit

# Create feature branch
git checkout -b feature-branch
create_commit
feature_commit=$(git rev-parse HEAD)

# Switch back to master
git checkout master
create_commit

# Create JUnit XML file
create_junit_xml "junit3.xml"

# Import to feature branch
git perf import --commit feature-branch junit junit3.xml

# Verify measurements appear in report from feature branch
output=$(git perf report feature-branch -o -)
assert_output_contains "$output" "test::test_fast" "Import should target feature-branch"

# Clean up
rm junit3.xml

echo "Test 4: Import to tag"
cd_temp_repo
tagged_commit=$(git rev-parse HEAD~1)
git tag v1.0 "$tagged_commit"

# Create JUnit XML file
create_junit_xml "junit4.xml"

# Import using tag name
git perf import --commit v1.0 junit junit4.xml

# Verify measurements appear in report from tag
output=$(git perf report v1.0 -o -)
assert_output_contains "$output" "test::test_fast" "Import should target tagged commit"

# Clean up
rm junit4.xml

echo "Test 5: Default import behavior without --commit flag"
cd_empty_repo
create_commit
head=$(git rev-parse HEAD)

# Create JUnit XML file
create_junit_xml "junit5.xml"

# Import without --commit flag (should default to HEAD)
git perf import junit junit5.xml

# Verify measurements appear in report from HEAD
output=$(git perf report -o -)
assert_output_contains "$output" "test::test_fast" "Default import should target HEAD"

# Clean up
rm junit5.xml

echo "Test 6: Import with prefix to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create JUnit XML file
create_junit_xml "junit6.xml"

# Import with prefix to specific commit
git perf import --commit HEAD~1 --prefix ci junit junit6.xml

# Verify measurements with prefix appear in report
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "ci::test::test_fast" "Prefix should be applied"

# Clean up
rm junit6.xml

echo "Test 7: Import with metadata to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create JUnit XML file
create_junit_xml "junit7.xml"

# Import with metadata to specific commit
git perf import --commit HEAD~1 --metadata env=ci --metadata branch=main junit junit7.xml

# Verify measurements are imported (metadata verification through report is complex, just check measurements exist)
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "test::test_fast" "Measurements should be imported"

# Clean up
rm junit7.xml

echo "Test 8: Import with filter to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create JUnit XML file
create_junit_xml "junit8.xml"

# Import with filter (only tests matching "test_fast")
git perf import --commit HEAD~1 --filter "test_fast" junit junit8.xml

# Verify only filtered measurements are imported
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "test::test_fast" "Filtered test should be imported"
assert_output_not_contains "$output" "test::test_medium" "Unmatched test should NOT be imported"
assert_output_not_contains "$output" "test::test_slow" "Unmatched test should NOT be imported"

# Clean up
rm junit8.xml

echo "Test 9: Import dry-run to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create JUnit XML file
create_junit_xml "junit9.xml"

# Import with dry-run (should not actually store measurements)
output=$(git perf import --commit HEAD~1 --dry-run junit junit9.xml)

# Verify output shows what would be imported
assert_output_contains "$output" "test_fast" "Dry-run should show import preview"

# Verify no measurements were actually stored
# Report may error with "No performance measurements found" which is expected
report_output=$(git perf report HEAD~1 -o - 2>&1 || true)
# Report should not contain our test data
if echo "$report_output" | grep "test::test_fast" > /dev/null 2>&1; then
    echo "FAIL: Dry-run should not store measurements"
    echo "Report output: $report_output"
    exit 1
fi

# Clean up
rm junit9.xml

echo "Test 10: Import multiple times to same commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create first JUnit XML file
create_junit_xml_custom "junit10a.xml" "0.001" "0.002"

# First import
git perf import --commit HEAD~1 junit junit10a.xml

# Create second JUnit XML file with different values
create_junit_xml_custom "junit10b.xml" "0.003" "0.004"

# Second import to same commit
git perf import --commit HEAD~1 junit junit10b.xml

# Verify both sets of measurements appear in report
output=$(git perf report HEAD~1 -o -)

# Count occurrences of test_one (should be 2)
count=$(echo "$output" | grep -c "test::test_one" || true)
if [[ $count -ne 2 ]]; then
    echo "FAIL: Expected 2 measurements for test_one, found $count"
    echo "Output: $output"
    exit 1
fi

# Clean up
rm junit10a.xml junit10b.xml

echo "Test 11: Import from stdin to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Import from stdin using echo
echo '<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="stdin_suite" tests="1">
    <testcase classname="StdinTests" name="test_stdin" time="0.123"/>
  </testsuite>
</testsuites>' | git perf import --commit HEAD~1 junit

# Verify measurement from stdin appears in report
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "test::test_stdin" "Import from stdin should work"

echo "Test 12: Import single testsuite (not testsuites) to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create single testsuite XML (without wrapper)
cat > "junit12.xml" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="single_suite" tests="1">
  <testcase classname="SingleTests" name="test_single" time="0.999"/>
</testsuite>
EOF

# Import single testsuite format
git perf import --commit HEAD~1 junit junit12.xml

# Verify measurement is imported
output=$(git perf report HEAD~1 -o -)
assert_output_contains "$output" "test::test_single" "Single testsuite format should work"

# Clean up
rm junit12.xml

echo "Test 13: Import with verbose flag to specific commit"
cd_temp_repo
target=$(git rev-parse HEAD~1)

# Create JUnit XML file
create_junit_xml "junit13.xml"

# Import with verbose flag
output=$(git perf import --commit HEAD~1 --verbose junit junit13.xml)

# Verbose output should show details
assert_output_contains "$output" "test_fast" "Verbose output should show test names"

# Verify measurements are stored
report_output=$(git perf report HEAD~1 -o -)
assert_output_contains "$report_output" "test::test_fast" "Measurements should be imported"

# Clean up
rm junit13.xml

echo "Test 14: Import error with invalid committish"
cd_temp_repo

# Create JUnit XML file
create_junit_xml "junit14.xml"

# Try to import to invalid commit
output=$(git perf import --commit nonexistent_commit junit junit14.xml 2>&1) && exit 1

# Should fail with resolution error
assert_output_contains "$output" "Failed to resolve commit" "Error should mention failed commit resolution"

# Clean up
rm junit14.xml

echo "All committish import tests passed!"
exit 0
