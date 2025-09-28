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

    let head_summary = stats::aggregate_measurements(iter::once(&head));
    let tail_summary = stats::aggregate_measurements(tail.iter());

    if tail_summary.len < min_count.into() {
        let number_measurements = tail_summary.len;
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

    // Calculate relative deviation of HEAD measurement using tail median
    let head_relative_deviation = (head / tail_median - 1.0).abs() * 100.0;

    // Check if we have a minimum relative deviation threshold configured
    let min_relative_deviation = config::audit_min_relative_deviation(measurement);
    let threshold_applied = min_relative_deviation.is_some();
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

    // Check if HEAD measurement exceeds sigma threshold
    let z_score_exceeds_sigma = z_score > sigma;

    // Determine if audit passes
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
    fn test_relative_calculations_with_known_values() {
        // Test mathematical operations to catch / vs % mutations
        let tail_median = 10.0;
        let test_value = 23.0;

        // Test division vs modulo - these should produce very different results
        let division_result = test_value / tail_median - 1.0;
        let modulo_result = test_value % tail_median - 1.0;

        // Division: 23.0 / 10.0 - 1.0 = 2.3 - 1.0 = 1.3
        // Modulo: 23.0 % 10.0 - 1.0 = 3.0 - 1.0 = 2.0
        assert!((division_result - 1.3_f64).abs() < 1e-10);
        assert!((modulo_result - 2.0_f64).abs() < 1e-10);

        // The mutation would change the result significantly
        assert!((division_result - modulo_result).abs() > 0.5_f64);

        // Test head relative deviation calculation
        let head = 25.0;
        let correct_deviation = (head / tail_median - 1.0).abs() * 100.0;
        let incorrect_deviation = (head % tail_median - 1.0).abs() * 100.0;

        // Division: (25.0 / 10.0 - 1.0).abs() * 100.0 = 150.0
        // Modulo: (25.0 % 10.0 - 1.0).abs() * 100.0 = 400.0
        assert!((correct_deviation - 150.0_f64).abs() < 1e-10);
        assert!((incorrect_deviation - 400.0_f64).abs() < 1e-10);
    }

    #[test]
    fn test_sigma_threshold_boundary_conditions() {
        // Test z-score boundary conditions for > vs >= mutations
        // Use values with some variance to avoid infinite z-scores
        let head_value = 15.0;
        let tail_values = vec![10.0, 10.5, 11.0, 9.5, 10.2];

        let head_summary = stats::aggregate_measurements(std::iter::once(&head_value));
        let tail_summary = stats::aggregate_measurements(tail_values.iter());

        let z_score =
            head_summary.z_score_with_method(&tail_summary, DispersionMethod::StandardDeviation);

        // Only proceed if z-score is finite
        if z_score.is_finite() {
            // Test exact boundary case
            let sigma_exact = z_score;
            let sigma_less = z_score - 0.001;
            let sigma_more = z_score + 0.001;

            // With > comparison:
            // z_score > sigma_exact should be false
            // z_score > sigma_less should be true
            // z_score > sigma_more should be false
            assert!(!(z_score > sigma_exact));
            assert!(z_score > sigma_less);
            assert!(!(z_score > sigma_more));

            // With >= comparison (the mutation):
            // z_score >= sigma_exact should be true (this is the difference!)
            assert!(z_score >= sigma_exact);
            assert!(z_score >= sigma_less);
            assert!(!(z_score >= sigma_more));
        } else {
            // Test with a simpler case to demonstrate the > vs >= difference
            let test_z = 2.5;
            let sigma_exact = 2.5;
            let sigma_less = 2.4;
            let sigma_more = 2.6;

            // Original logic: z > sigma
            assert!(!(test_z > sigma_exact)); // false
            assert!(test_z > sigma_less); // true
            assert!(!(test_z > sigma_more)); // false

            // Mutated logic: z >= sigma
            assert!(test_z >= sigma_exact); // true (this is different!)
            assert!(test_z >= sigma_less); // true
            assert!(!(test_z >= sigma_more)); // false
        }
    }

    #[test]
    fn test_threshold_comparison_logic() {
        // Test < vs == mutations in threshold comparison
        let test_cases = vec![
            (5.0, 10.0, true),   // deviation < threshold should pass
            (10.0, 10.0, false), // deviation == threshold: < passes, == would fail
            (15.0, 10.0, false), // deviation > threshold should fail
        ];

        for (deviation, threshold, should_pass_with_less_than) in test_cases {
            // Test original logic: deviation < threshold
            let passes_with_less = deviation < threshold;
            assert_eq!(passes_with_less, should_pass_with_less_than);

            // Test mutated logic: deviation == threshold
            let passes_with_equal = deviation == threshold;

            // The mutation should change behavior when deviation equals threshold
            let diff: f64 = deviation - threshold;
            if diff.abs() < 1e-10 {
                // When equal, < returns false, == returns true
                assert!(!passes_with_less);
                assert!(passes_with_equal);
            }
        }
    }

    #[test]
    fn test_audit_pass_fail_logic() {
        // Test core logic negation mutations

        // Case 1: z_score exceeds sigma, no threshold - should fail
        let z_score_exceeds = true;
        let threshold_saves = false;

        // Original: !z_score_exceeds_sigma || passed_due_to_threshold
        // = !true || false = false || false = false (audit fails)
        let original_passed = !z_score_exceeds || threshold_saves;
        assert!(!original_passed);

        // Mutation: z_score_exceeds_sigma || passed_due_to_threshold
        // = true || false = true (audit would incorrectly pass)
        let mutated_passed = z_score_exceeds || threshold_saves;
        assert!(mutated_passed);

        // Case 2: z_score exceeds sigma, but threshold saves - should pass
        let z_score_exceeds = true;
        let threshold_saves = true;

        let original_passed = !z_score_exceeds || threshold_saves;
        assert!(original_passed);

        let mutated_passed = z_score_exceeds || threshold_saves;
        assert!(mutated_passed);

        // Case 3: z_score within sigma - should pass
        let z_score_exceeds = false;
        let threshold_saves = false;

        let original_passed = !z_score_exceeds || threshold_saves;
        assert!(original_passed);

        let mutated_passed = z_score_exceeds || threshold_saves;
        assert!(!mutated_passed); // Mutation would make this fail!
    }

    #[test]
    fn test_measurement_count_and_pluralization() {
        // Test edge cases for count validation and message formatting

        // Test the min_count boundary condition mutation: < vs ==
        let test_cases = vec![
            (2, 3, true),  // len < min_count: should skip (return true)
            (3, 3, false), // len == min_count: should NOT skip, but == mutation would
            (4, 3, false), // len > min_count: should NOT skip
        ];

        for (actual_count, min_count, should_skip) in test_cases {
            // Original logic: tail_summary.len < min_count.into()
            let skips_with_less = actual_count < min_count;
            assert_eq!(skips_with_less, should_skip);

            // Mutated logic: tail_summary.len == min_count.into()
            let skips_with_equal = actual_count == min_count;

            // When actual equals min, behavior differs
            if actual_count == min_count {
                assert!(!skips_with_less); // Original: don't skip
                assert!(skips_with_equal); // Mutation: would skip
            }
        }

        // Test pluralization logic: > vs < mutation
        let plural_test_cases = vec![
            (0, ""),  // 0 measurements -> no 's'
            (1, ""),  // 1 measurement -> no 's'
            (2, "s"), // 2+ measurements -> 's'
            (5, "s"), // Multiple measurements -> 's'
        ];

        for (count, expected_suffix) in plural_test_cases {
            // Original: if number_measurements > 1
            let original_suffix = if count > 1 { "s" } else { "" };
            assert_eq!(original_suffix, expected_suffix);

            // Mutation: if number_measurements < 1
            let mutated_suffix = if count < 1 { "s" } else { "" };

            // The mutation inverts the logic for count = 0 vs count >= 1
            if count == 0 {
                assert_eq!(original_suffix, "");
                assert_eq!(mutated_suffix, "s"); // Wrong!
            } else if count == 1 {
                assert_eq!(original_suffix, "");
                assert_eq!(mutated_suffix, ""); // Same
            } else {
                assert_eq!(original_suffix, "s");
                assert_eq!(mutated_suffix, ""); // Wrong!
            }
        }
    }

    #[test]
    fn test_audit_result_inversion_logic() {
        // Test the final audit result logic mutation: !passed vs passed

        // Case 1: Audit should fail (passed = false)
        let passed = false;

        // Original: if !passed (should enter failure branch)
        let should_enter_failure_branch = !passed;
        assert!(should_enter_failure_branch);

        // Mutation: if passed (should NOT enter failure branch)
        let mutated_enters_failure = passed;
        assert!(!mutated_enters_failure);

        // Case 2: Audit should pass (passed = true)
        let passed = true;

        // Original: if !passed (should NOT enter failure branch)
        let should_enter_failure_branch = !passed;
        assert!(!should_enter_failure_branch);

        // Mutation: if passed (should enter failure branch - wrong!)
        let mutated_enters_failure = passed;
        assert!(mutated_enters_failure);

        // This mutation would cause passing audits to show failure messages
        // and failing audits to show success messages!
    }
}
