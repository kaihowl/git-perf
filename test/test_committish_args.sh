#!/usr/bin/env bash

# Test that commands can target specific commits using --commit flag

set -euo pipefail
source "$(dirname "$0")/common.sh"

setup_test_repo() {
    cd_empty_repo
    create_commit
}

test_add_to_specific_commit() {
    setup_test_repo

    # Create multiple commits
    git commit --allow-empty -m "Commit 1"
    local commit1=$(git rev-parse HEAD)

    git commit --allow-empty -m "Commit 2"
    local commit2=$(git rev-parse HEAD)

    git commit --allow-empty -m "Commit 3"
    local commit3=$(git rev-parse HEAD)

    # Add measurement to commit2 (not HEAD) - write operation uses --commit flag
    git perf add 100.5 -m test_metric --commit "$commit2"

    # Verify measurement was added to commit2
    local notes=$(git notes --ref refs/notes/perf-v3 show "$commit2")
    assert_contains "$notes" "test_metric"
    assert_contains "$notes" "100.5"

    # Verify commit3 (HEAD) does not have the measurement
    if git notes --ref refs/notes/perf-v3 show "$commit3" 2>/dev/null; then
        fail "Measurement should not be on HEAD (commit3)"
    fi

    echo "✓ test_add_to_specific_commit passed"
}

test_add_using_HEAD_tilde() {
    setup_test_repo

    # Create multiple commits
    git commit --allow-empty -m "Commit 1"
    git commit --allow-empty -m "Commit 2"
    git commit --allow-empty -m "Commit 3"

    local commit1=$(git rev-parse HEAD~2)

    # Add measurement to HEAD~2 - write operation uses --commit flag
    git perf add 200.5 -m test_metric2 --commit HEAD~2

    # Verify measurement was added to HEAD~2
    local notes=$(git notes --ref refs/notes/perf-v3 show "$commit1")
    assert_contains "$notes" "test_metric2"
    assert_contains "$notes" "200.5"

    echo "✓ test_add_using_HEAD_tilde passed"
}

test_add_using_branch_name() {
    setup_test_repo

    # Create a branch and add a commit
    git checkout -b feature-branch
    git commit --allow-empty -m "Feature commit"
    local feature_commit=$(git rev-parse HEAD)

    git checkout master
    git commit --allow-empty -m "Master commit"

    # Add measurement to feature-branch commit - write operation uses --commit flag
    git perf add 300.5 -m test_metric3 --commit feature-branch

    # Verify measurement was added to feature branch commit
    local notes=$(git notes --ref refs/notes/perf-v3 show "$feature_commit")
    assert_contains "$notes" "test_metric3"
    assert_contains "$notes" "300.5"

    echo "✓ test_add_using_branch_name passed"
}

test_audit_specific_commit() {
    setup_test_repo

    # Create commits with measurements
    git commit --allow-empty -m "Commit 1"
    git perf add 100.0 -m perf_test

    git commit --allow-empty -m "Commit 2"
    git perf add 105.0 -m perf_test

    git commit --allow-empty -m "Commit 3"
    local commit3=$(git rev-parse HEAD)
    git perf add 110.0 -m perf_test

    # Audit commit3 specifically - read operation uses positional argument
    local output=$(git perf audit "$commit3" -m perf_test -n 3)

    # Should show audit results for commit3
    assert_contains "$output" "perf_test"

    echo "✓ test_audit_specific_commit passed"
}

test_default_to_HEAD() {
    setup_test_repo

    git commit --allow-empty -m "Commit 1"

    # Add measurement without --commit flag (should default to HEAD)
    git perf add 42.0 -m default_test

    # Verify measurement was added to HEAD
    local head_commit=$(git rev-parse HEAD)
    local notes=$(git notes --ref refs/notes/perf-v3 show "$head_commit")
    assert_contains "$notes" "default_test"
    assert_contains "$notes" "42.0"

    echo "✓ test_default_to_HEAD passed"
}

test_invalid_committish_error() {
    setup_test_repo

    git commit --allow-empty -m "Commit 1"

    # Try to add measurement to invalid commit - write operation uses --commit flag
    if git perf add 50.0 -m error_test --commit nonexistent_commit 2>/dev/null; then
        fail "Should have failed with invalid committish"
    fi

    echo "✓ test_invalid_committish_error passed"
}

# Run all tests
test_add_to_specific_commit
test_add_using_HEAD_tilde
test_add_using_branch_name
test_audit_specific_commit
test_default_to_HEAD
test_invalid_committish_error

echo ""
echo "All committish argument tests passed!"
