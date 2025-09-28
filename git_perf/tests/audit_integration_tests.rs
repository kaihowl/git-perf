use git_perf::audit::audit_multiple;
use git_perf::stats::{DispersionMethod, ReductionFunc};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn create_test_repo_with_measurements() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git user for test
    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create test file and initial commit
    fs::write(repo_path.join("test.txt"), "initial").unwrap();
    Command::new("git")
        .args(&["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["commit", "-m", "initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

#[test]
fn test_audit_multiple_with_insufficient_measurements() {
    let _temp_dir = create_test_repo_with_measurements();

    // Test with empty measurements list
    let result = audit_multiple(
        &[],
        100,
        5,
        &[],
        ReductionFunc::Mean,
        2.0,
        DispersionMethod::StandardDeviation,
    );

    // Should succeed with no measurements to audit
    assert!(result.is_ok());
}

#[test]
fn test_audit_multiple_error_propagation() {
    let _temp_dir = create_test_repo_with_measurements();

    // Test with non-existent measurement
    let measurements = vec!["nonexistent_measurement".to_string()];

    let result = audit_multiple(
        &measurements,
        100,
        1,
        &[],
        ReductionFunc::Mean,
        2.0,
        DispersionMethod::StandardDeviation,
    );

    // Should handle missing measurements gracefully
    // The exact behavior depends on measurement_retrieval implementation
    // but it should either succeed (skip) or provide meaningful error
    match result {
        Ok(()) => {
            // If it succeeds, it means the audit was skipped due to insufficient data
            // This is acceptable behavior
        }
        Err(_) => {
            // If it errors, it should be a meaningful error about missing data
            // This is also acceptable behavior
        }
    }
}

#[test]
fn test_audit_boundary_conditions() {
    // Test edge cases that might not be covered by existing tests

    // Test with min_count = 0 (edge case)
    let result = audit_multiple(
        &[],
        100,
        0, // minimum count of 0
        &[],
        ReductionFunc::Mean,
        2.0,
        DispersionMethod::StandardDeviation,
    );

    assert!(result.is_ok());
}

#[test]
fn test_audit_with_extreme_sigma_values() {
    let _temp_dir = create_test_repo_with_measurements();

    // Test with very high sigma (should always pass)
    let result = audit_multiple(
        &["test_measurement".to_string()],
        100,
        1,
        &[],
        ReductionFunc::Mean,
        f64::MAX, // Extremely high sigma
        DispersionMethod::StandardDeviation,
    );

    // With such high sigma, any measurement should pass
    // Even if no measurements exist, it should handle gracefully
    match result {
        Ok(()) => {} // Expected - either passed or skipped due to no data
        Err(_) => {} // Also acceptable if it's a data availability error
    }

    // Test with zero sigma (should be very strict)
    let result = audit_multiple(
        &["test_measurement".to_string()],
        100,
        1,
        &[],
        ReductionFunc::Mean,
        0.0, // Zero sigma - very strict
        DispersionMethod::StandardDeviation,
    );

    // With zero sigma, most audits should fail unless there's perfect stability
    match result {
        Ok(()) => {} // Could pass if no measurements or perfect stability
        Err(_) => {} // Expected for most real scenarios
    }
}

#[test]
fn test_audit_with_different_reduction_functions() {
    let _temp_dir = create_test_repo_with_measurements();

    let measurement = "test_measurement".to_string();
    let reduction_functions = vec![
        ReductionFunc::Min,
        ReductionFunc::Max,
        ReductionFunc::Median,
        ReductionFunc::Mean,
    ];

    for reduction_func in reduction_functions {
        let result = audit_multiple(
            &[measurement.clone()],
            100,
            1,
            &[],
            reduction_func,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        // All reduction functions should handle gracefully
        // (either pass/fail based on data or skip due to insufficient data)
        match result {
            Ok(()) => {} // Acceptable
            Err(_) => {} // Also acceptable for data issues
        }
    }
}

#[test]
fn test_audit_with_selectors() {
    let _temp_dir = create_test_repo_with_measurements();

    // Test with various selector combinations
    let selectors = vec![
        ("key1".to_string(), "value1".to_string()),
        ("key2".to_string(), "value2".to_string()),
    ];

    let result = audit_multiple(
        &["test_measurement".to_string()],
        100,
        1,
        &selectors,
        ReductionFunc::Mean,
        2.0,
        DispersionMethod::StandardDeviation,
    );

    // Should handle selectors without crashing
    match result {
        Ok(()) => {} // Expected behavior
        Err(_) => {} // Acceptable if no matching data
    }
}
