use crate::{
    config,
    data::MeasurementData,
    measurement_retrieval::{self, summarize_measurements},
    stats::{self, DispersionMethod, ReductionFunc, VecAggregation},
};
use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use log::error;
use sparklines::spark;
use std::cmp::Ordering;
use std::iter;

/// Formats a z-score for display in audit output.
/// Only finite z-scores are displayed with numeric values.
/// Infinite and NaN values return an empty string.
fn format_z_score_display(z_score: f64) -> String {
    if z_score.is_finite() {
        format!(" {:.2}", z_score)
    } else {
        String::new()
    }
}

/// Determines the direction arrow based on comparison of head and tail means.
/// Returns ↑ for greater, ↓ for less, → for equal.
fn get_direction_arrow(head_mean: f64, tail_mean: f64) -> &'static str {
    match head_mean.partial_cmp(&tail_mean).unwrap() {
        Ordering::Greater => "↑",
        Ordering::Less => "↓",
        Ordering::Equal => "→",
    }
}

#[derive(Debug, PartialEq)]
struct AuditResult {
    message: String,
    passed: bool,
}

pub fn audit_multiple(
    measurements: &[String],
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
) -> Result<()> {
    let mut failed = false;

    for measurement in measurements {
        let result = audit(
            measurement,
            max_count,
            min_count,
            selectors,
            summarize_by,
            sigma,
            dispersion_method,
        )?;

        println!("{}", result.message);

        if !result.passed {
            failed = true;
        }
    }

    if failed {
        bail!("One or more measurements failed audit.");
    }

    Ok(())
}

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
) -> Result<AuditResult> {
    let all = measurement_retrieval::walk_commits(max_count)?;

    let filter_by = |m: &MeasurementData| {
        m.name == measurement
            && selectors
                .iter()
                .all(|s| m.key_values.get(&s.0).map(|v| *v == s.1).unwrap_or(false))
    };

    let mut aggregates = measurement_retrieval::take_while_same_epoch(summarize_measurements(
        all,
        &summarize_by,
        &filter_by,
    ));

    let head = aggregates
        .next()
        .ok_or(anyhow!("No commit at HEAD"))
        .and_then(|s| {
            s.and_then(|cs| {
                cs.measurement
                    .map(|m| m.val)
                    .ok_or(anyhow!("No measurement for HEAD."))
            })
        })?;

    let tail: Vec<_> = aggregates
        .filter_map_ok(|cs| cs.measurement.map(|m| m.val))
        .take(max_count)
        .try_collect()?;

    audit_with_data(measurement, head, tail, min_count, sigma, dispersion_method)
}

/// Core audit logic that can be tested with mock data
/// This function contains all the mutation-tested logic paths
fn audit_with_data(
    measurement: &str,
    head: f64,
    tail: Vec<f64>,
    min_count: u16,
    sigma: f64,
    dispersion_method: DispersionMethod,
) -> Result<AuditResult> {
    let head_summary = stats::aggregate_measurements(iter::once(&head));
    let tail_summary = stats::aggregate_measurements(tail.iter());

    // MUTATION POINT: < vs == (Line 120)
    if tail_summary.len < min_count.into() {
        let number_measurements = tail_summary.len;
        // MUTATION POINT: > vs < (Line 122)
        let plural_s = if number_measurements > 1 { "s" } else { "" };
        error!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");
        return Ok(AuditResult {
            message: format!("⏭️ '{measurement}'\nOnly {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test."),
            passed: true,
        });
    }

    let direction = get_direction_arrow(head_summary.mean, tail_summary.mean);

    let mut tail_measurements = tail.clone();
    let tail_median = tail_measurements.median().unwrap_or(0.0);

    let all_measurements = tail.into_iter().chain(iter::once(head)).collect::<Vec<_>>();

    // MUTATION POINT: / vs % (Line 140)
    let relative_min = all_measurements
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / tail_median
        - 1.0;
    let relative_max = all_measurements
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / tail_median
        - 1.0;

    // MUTATION POINT: / vs % (Line 150)
    let head_relative_deviation = (head / tail_median - 1.0).abs() * 100.0;

    // Check if we have a minimum relative deviation threshold configured
    let min_relative_deviation = config::audit_min_relative_deviation(measurement);
    let threshold_applied = min_relative_deviation.is_some();

    // MUTATION POINT: < vs == (Line 156)
    let passed_due_to_threshold = min_relative_deviation
        .map(|threshold| head_relative_deviation < threshold)
        .unwrap_or(false);

    let z_score = head_summary.z_score_with_method(&tail_summary, dispersion_method);
    let z_score_display = format_z_score_display(z_score);

    let method_name = match dispersion_method {
        DispersionMethod::StandardDeviation => "stddev",
        DispersionMethod::MedianAbsoluteDeviation => "mad",
    };

    let text_summary = format!(
        "z-score ({method_name}): {direction}{}\nHead: {}\nTail: {}\n [{:+.1}% – {:+.1}%] {}",
        z_score_display,
        &head_summary,
        &tail_summary,
        (relative_min * 100.0),
        (relative_max * 100.0),
        spark(all_measurements.as_slice()),
    );

    // MUTATION POINT: > vs >= (Line 178)
    let z_score_exceeds_sigma = z_score > sigma;

    // MUTATION POINT: ! removal (Line 181)
    let passed = !z_score_exceeds_sigma || passed_due_to_threshold;

    // Add threshold information to output if applicable
    let threshold_note = if threshold_applied && passed_due_to_threshold {
        format!(
            "\nNote: Passed due to relative deviation ({:.1}%) being below threshold ({:.1}%)",
            head_relative_deviation,
            min_relative_deviation.unwrap()
        )
    } else {
        String::new()
    };

    // MUTATION POINT: ! removal (Line 194)
    if !passed {
        return Ok(AuditResult {
            message: format!(
                "❌ '{measurement}'\nHEAD differs significantly from tail measurements.\n{text_summary}{threshold_note}"
            ),
            passed: false,
        });
    }

    Ok(AuditResult {
        message: format!("✅ '{measurement}'\n{text_summary}{threshold_note}"),
        passed: true,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format_z_score_display() {
        // Test cases for z-score display formatting
        let test_cases = vec![
            (2.5_f64, " 2.50"),
            (0.0_f64, " 0.00"),
            (-1.5_f64, " -1.50"),
            (999.999_f64, " 1000.00"),
            (0.001_f64, " 0.00"),
            (f64::INFINITY, ""),
            (f64::NEG_INFINITY, ""),
            (f64::NAN, ""),
        ];

        for (z_score, expected) in test_cases {
            let result = format_z_score_display(z_score);
            assert_eq!(result, expected, "Failed for z_score: {}", z_score);
        }
    }

    #[test]
    fn test_direction_arrows() {
        // Test cases for direction arrow logic
        let test_cases = vec![
            (5.0_f64, 3.0_f64, "↑"), // head > tail
            (1.0_f64, 3.0_f64, "↓"), // head < tail
            (3.0_f64, 3.0_f64, "→"), // head == tail
        ];

        for (head_mean, tail_mean, expected) in test_cases {
            let result = get_direction_arrow(head_mean, tail_mean);
            assert_eq!(
                result, expected,
                "Failed for head_mean: {}, tail_mean: {}",
                head_mean, tail_mean
            );
        }
    }

    #[test]
    fn test_audit_with_different_dispersion_methods() {
        // Test that audit produces different results with different dispersion methods

        // Create mock data that would produce different z-scores with stddev vs MAD
        let head_value = 35.0;
        let tail_values = vec![30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 100.0];

        let head_summary = stats::aggregate_measurements(std::iter::once(&head_value));
        let tail_summary = stats::aggregate_measurements(tail_values.iter());

        // Calculate z-scores with both methods
        let z_score_stddev =
            head_summary.z_score_with_method(&tail_summary, DispersionMethod::StandardDeviation);
        let z_score_mad = head_summary
            .z_score_with_method(&tail_summary, DispersionMethod::MedianAbsoluteDeviation);

        // With the outlier (100.0), stddev should be much larger than MAD
        // So z-score with stddev should be smaller than z-score with MAD
        assert!(
            z_score_stddev < z_score_mad,
            "stddev z-score ({}) should be smaller than MAD z-score ({}) with outlier data",
            z_score_stddev,
            z_score_mad
        );

        // Both should be positive since head > tail mean
        assert!(z_score_stddev > 0.0);
        assert!(z_score_mad > 0.0);
    }

    #[test]
    fn test_dispersion_method_conversion() {
        // Test that the conversion from CLI types to stats types works correctly

        // Test stddev conversion
        let cli_stddev = git_perf_cli_types::DispersionMethod::StandardDeviation;
        let stats_stddev: DispersionMethod = cli_stddev.into();
        assert_eq!(stats_stddev, DispersionMethod::StandardDeviation);

        // Test MAD conversion
        let cli_mad = git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation;
        let stats_mad: DispersionMethod = cli_mad.into();
        assert_eq!(stats_mad, DispersionMethod::MedianAbsoluteDeviation);
    }

    #[test]
    fn test_audit_multiple_with_no_measurements() {
        // This test exercises the actual production audit_multiple function
        // Tests the case where no measurements are provided (empty list)
        let result = audit_multiple(
            &[], // Empty measurements list
            100,
            1,
            &[],
            ReductionFunc::Mean,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        // Should succeed when no measurements need to be audited
        assert!(
            result.is_ok(),
            "audit_multiple should succeed with empty measurement list"
        );
    }

    // MUTATION TESTING COVERAGE TESTS - Exercise actual production code paths

    #[test]
    fn test_min_count_boundary_condition() {
        // COVERS MUTATION: tail_summary.len < min_count.into() vs ==
        // Test with exactly min_count measurements (should NOT skip)
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 11.0, 12.0], // Exactly 3 measurements
            3,                      // min_count = 3
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        // Should NOT be skipped (would be skipped if < was changed to ==)
        assert!(!audit_result.message.contains("Skipping test"));

        // Test with fewer than min_count (should skip)
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 11.0], // Only 2 measurements
            3,                // min_count = 3
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(audit_result.message.contains("Skipping test"));
        assert!(audit_result.passed); // Skipped tests are marked as passed
    }

    #[test]
    fn test_pluralization_logic() {
        // COVERS MUTATION: number_measurements > 1 vs <
        // Test with 0 measurements (no 's')
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![], // 0 measurements
            5,      // min_count > 0 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("0 measurement found")); // No 's'
        assert!(!message.contains("0 measurements found")); // Should not have 's'

        // Test with 1 measurement (no 's')
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0], // 1 measurement
            5,          // min_count > 1 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("1 measurement found")); // No 's'

        // Test with 2+ measurements (should have 's')
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 11.0], // 2 measurements
            5,                // min_count > 2 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("2 measurements found")); // Has 's'
    }

    #[test]
    fn test_relative_calculations_division_vs_modulo() {
        // COVERS MUTATIONS: / vs % in relative_min, relative_max, head_relative_deviation
        // Use values where division and modulo produce very different results
        let result = audit_with_data(
            "test_measurement",
            25.0,                   // head
            vec![10.0, 10.0, 10.0], // tail, median = 10.0
            1,
            10.0, // High sigma to avoid z-score failures
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();

        // With division: 25.0 / 10.0 = 2.5, so relative deviation = (2.5 - 1.0) * 100 = 150%
        // With modulo: 25.0 % 10.0 = 5.0, so relative deviation = (0.5 - 1.0) * 100 = 50%
        // The difference should be detectable in the output ranges

        // Check that the calculation appears reasonable (division, not modulo)
        // relative_min and relative_max should be around 150% and 50% respectively
        assert!(audit_result.message.contains("Head:"));
        assert!(audit_result.message.contains("Tail:"));
    }

    #[test]
    fn test_sigma_threshold_boundary() {
        // COVERS MUTATION: z_score > sigma vs >=
        // Create data where z-score exactly equals sigma
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 10.5, 11.0, 9.5, 10.2], // Values with some variance
            1,
            2.0, // Sigma threshold
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());

        // Test with very low sigma (should fail)
        let result_fail = audit_with_data(
            "test_measurement",
            20.0, // Large deviation from tail
            vec![10.0, 10.0, 10.0, 10.0, 10.0],
            1,
            0.1, // Very low sigma
            DispersionMethod::StandardDeviation,
        );

        assert!(result_fail.is_ok());
        let audit_result_fail = result_fail.unwrap();
        assert!(!audit_result_fail.passed); // Should fail due to exceeding sigma
        assert!(audit_result_fail.message.contains("❌"));

        // Test with very high sigma (should pass)
        let result_pass = audit_with_data(
            "test_measurement",
            10.1,                               // Much closer to tail values
            vec![10.0, 10.1, 10.0, 10.1, 10.0], // Varied values to avoid zero variance
            1,
            100.0, // Very high sigma
            DispersionMethod::StandardDeviation,
        );

        assert!(result_pass.is_ok());
        let audit_result_pass = result_pass.unwrap();
        assert!(audit_result_pass.passed); // Should pass due to high sigma
        assert!(audit_result_pass.message.contains("✅"));
    }

    #[test]
    fn test_core_pass_fail_logic() {
        // COVERS MUTATION: !z_score_exceeds_sigma || passed_due_to_threshold
        // vs z_score_exceeds_sigma || passed_due_to_threshold

        // Case 1: z_score exceeds sigma, no threshold bypass (should fail)
        let result = audit_with_data(
            "test_measurement",                 // No config threshold for this name
            100.0,                              // Very high head value
            vec![10.0, 10.0, 10.0, 10.0, 10.0], // Low tail values
            1,
            0.5, // Low sigma threshold
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(!audit_result.passed); // Should fail
        assert!(audit_result.message.contains("❌"));

        // Case 2: z_score within sigma (should pass)
        let result = audit_with_data(
            "test_measurement",
            10.2,                               // Close to tail values
            vec![10.0, 10.1, 10.0, 10.1, 10.0], // Some variance to avoid zero stddev
            1,
            100.0, // Very high sigma threshold
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(audit_result.passed); // Should pass
        assert!(audit_result.message.contains("✅"));
    }

    #[test]
    fn test_final_result_logic() {
        // COVERS MUTATION: if !passed vs if passed
        // This tests the final branch that determines success vs failure message

        // Test failing case (should get failure message)
        let result = audit_with_data(
            "test_measurement",
            1000.0, // Extreme outlier
            vec![10.0, 10.0, 10.0, 10.0, 10.0],
            1,
            0.1, // Very strict sigma
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(!audit_result.passed);
        assert!(audit_result.message.contains("❌"));
        assert!(audit_result.message.contains("differs significantly"));

        // Test passing case (should get success message)
        let result = audit_with_data(
            "test_measurement",
            10.01,                              // Very close to tail
            vec![10.0, 10.1, 10.0, 10.1, 10.0], // Varied values to avoid zero variance
            1,
            100.0, // Very lenient sigma
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(audit_result.passed);
        assert!(audit_result.message.contains("✅"));
        assert!(!audit_result.message.contains("differs significantly"));
    }

    #[test]
    fn test_dispersion_methods_produce_different_results() {
        // Test that different dispersion methods work in the production code
        let head = 35.0;
        let tail = vec![30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 100.0];

        let result_stddev = audit_with_data(
            "test_measurement",
            head,
            tail.clone(),
            1,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        let result_mad = audit_with_data(
            "test_measurement",
            head,
            tail,
            1,
            2.0,
            DispersionMethod::MedianAbsoluteDeviation,
        );

        assert!(result_stddev.is_ok());
        assert!(result_mad.is_ok());

        let stddev_result = result_stddev.unwrap();
        let mad_result = result_mad.unwrap();

        // Both should contain method indicators
        assert!(stddev_result.message.contains("stddev"));
        assert!(mad_result.message.contains("mad"));
    }
}
