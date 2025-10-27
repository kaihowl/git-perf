use crate::{
    config,
    data::MeasurementData,
    measurement_retrieval::{self, summarize_measurements},
    stats::{self, DispersionMethod, ReductionFunc, StatsWithUnit, VecAggregation},
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
/// Returns → for NaN values to avoid panicking.
fn get_direction_arrow(head_mean: f64, tail_mean: f64) -> &'static str {
    match head_mean.partial_cmp(&tail_mean) {
        Some(Ordering::Greater) => "↑",
        Some(Ordering::Less) => "↓",
        Some(Ordering::Equal) | None => "→",
    }
}

#[derive(Debug, PartialEq)]
struct AuditResult {
    message: String,
    passed: bool,
}

/// Resolved audit parameters for a specific measurement.
#[derive(Debug, PartialEq)]
pub(crate) struct ResolvedAuditParams {
    pub min_count: u16,
    pub summarize_by: ReductionFunc,
    pub sigma: f64,
    pub dispersion_method: DispersionMethod,
}

/// Resolves audit parameters for a specific measurement with proper precedence:
/// CLI option -> measurement-specific config -> global config -> built-in default
///
/// Note: When CLI provides min_count, the caller (audit_multiple) uses the same
/// value for all measurements. When CLI is None, this function reads per-measurement config.
pub(crate) fn resolve_audit_params(
    measurement: &str,
    cli_min_count: Option<u16>,
    cli_summarize_by: Option<ReductionFunc>,
    cli_sigma: Option<f64>,
    cli_dispersion_method: Option<DispersionMethod>,
) -> ResolvedAuditParams {
    let min_count = cli_min_count
        .or_else(|| config::audit_min_measurements(measurement))
        .unwrap_or(2);

    let summarize_by = cli_summarize_by
        .or_else(|| config::audit_aggregate_by(measurement).map(ReductionFunc::from))
        .unwrap_or(ReductionFunc::Min);

    let sigma = cli_sigma
        .or_else(|| config::audit_sigma(measurement))
        .unwrap_or(4.0);

    let dispersion_method = cli_dispersion_method
        .or_else(|| {
            Some(DispersionMethod::from(config::audit_dispersion_method(
                measurement,
            )))
        })
        .unwrap_or(DispersionMethod::StandardDeviation);

    ResolvedAuditParams {
        min_count,
        summarize_by,
        sigma,
        dispersion_method,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn audit_multiple(
    measurements: &[String],
    max_count: usize,
    min_count: Option<u16>,
    selectors: &[(String, String)],
    summarize_by: Option<ReductionFunc>,
    sigma: Option<f64>,
    dispersion_method: Option<DispersionMethod>,
    combined_patterns: &[String],
) -> Result<()> {
    // Compile combined regex patterns (measurements as exact matches + filter patterns)
    // early to fail fast on invalid patterns
    let filters = crate::filter::compile_filters(combined_patterns)?;

    let mut failed = false;

    for measurement in measurements {
        let params = resolve_audit_params(
            measurement,
            min_count,
            summarize_by,
            sigma,
            dispersion_method,
        );

        // Warn if max_count limits historical data below min_measurements requirement
        if (max_count as u16) < params.min_count {
            eprintln!(
                "⚠️  Warning: --max_count ({}) is less than min_measurements ({}) for measurement '{}'.",
                max_count, params.min_count, measurement
            );
            eprintln!(
                "   This limits available historical data and may prevent achieving statistical significance."
            );
        }

        let result = audit(
            measurement,
            max_count,
            params.min_count,
            selectors,
            params.summarize_by,
            params.sigma,
            params.dispersion_method,
            &filters,
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

#[allow(clippy::too_many_arguments)]
fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
    filters: &[regex::Regex],
) -> Result<AuditResult> {
    let all = measurement_retrieval::walk_commits(max_count)?;

    // Filter using subset relation: selectors ⊆ measurement.key_values
    // The filters now include both exact measurement matches (as anchored regex)
    // and user-provided filter patterns, so we only need to check:
    // 1. Does the name match any filter (which includes exact measurement name)
    // 2. Do the selectors match
    let filter_by = |m: &MeasurementData| {
        // Apply regex filters (handles both exact measurement match and patterns)
        if !crate::filter::matches_any_filter(&m.name, filters) {
            return false;
        }

        // Check that we're looking at the right measurement for this audit
        // (since filters may match multiple measurements)
        if m.name != measurement {
            return false;
        }

        // Existing selector check
        m.key_values_is_superset_of(selectors)
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
    // Get unit for this measurement from config
    let unit = config::measurement_unit(measurement);
    let unit_str = unit.as_deref();

    let head_summary = stats::aggregate_measurements(iter::once(&head));
    let tail_summary = stats::aggregate_measurements(tail.iter());

    // Generate sparkline and calculate range for all measurements - used in both skip and normal paths
    let all_measurements = tail.into_iter().chain(iter::once(head)).collect::<Vec<_>>();

    let mut tail_measurements = all_measurements.clone();
    tail_measurements.pop(); // Remove head to get just tail for median calculation
    let tail_median = tail_measurements.median().unwrap_or(0.0);

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

    let sparkline = format!(
        " [{:+.2}% – {:+.2}%] {}",
        (relative_min * 100.0),
        (relative_max * 100.0),
        spark(all_measurements.as_slice())
    );

    // Helper function to build the measurement summary text
    // This is used for both skipped and normal audit results to avoid duplication
    let build_summary = || -> String {
        let mut summary = String::new();

        // Use the length of all_measurements vector for total count
        let total_measurements = all_measurements.len();

        // If only 1 total measurement (head only, no tail), show only head summary
        if total_measurements == 1 {
            let head_display = StatsWithUnit {
                stats: &head_summary,
                unit: unit_str,
            };
            summary.push_str(&format!("Head: {}\n", head_display));
        } else if total_measurements >= 2 {
            // 2+ measurements: show z-score, head, tail, and sparkline
            let direction = get_direction_arrow(head_summary.mean, tail_summary.mean);
            let z_score = head_summary.z_score_with_method(&tail_summary, dispersion_method);
            let z_score_display = format_z_score_display(z_score);
            let method_name = match dispersion_method {
                DispersionMethod::StandardDeviation => "stddev",
                DispersionMethod::MedianAbsoluteDeviation => "mad",
            };

            let head_display = StatsWithUnit {
                stats: &head_summary,
                unit: unit_str,
            };
            let tail_display = StatsWithUnit {
                stats: &tail_summary,
                unit: unit_str,
            };

            summary.push_str(&format!(
                "z-score ({method_name}): {direction}{}\n",
                z_score_display
            ));
            summary.push_str(&format!("Head: {}\n", head_display));
            summary.push_str(&format!("Tail: {}\n", tail_display));
            summary.push_str(&sparkline);
        }
        // If 0 total measurements, return empty summary

        summary
    };

    // MUTATION POINT: < vs == (Line 120)
    if tail_summary.len < min_count.into() {
        let number_measurements = tail_summary.len;
        // MUTATION POINT: > vs < (Line 122)
        let plural_s = if number_measurements > 1 { "s" } else { "" };
        error!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");

        let mut skip_message = format!(
            "⏭️ '{measurement}'\nOnly {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test."
        );

        // Add summary using the same logic as passing/failing cases
        let summary = build_summary();
        if !summary.is_empty() {
            skip_message.push('\n');
            skip_message.push_str(&summary);
        }

        return Ok(AuditResult {
            message: skip_message,
            passed: true,
        });
    }

    // MUTATION POINT: / vs % (Line 150)
    let head_relative_deviation = (head / tail_median - 1.0).abs() * 100.0;

    // Check if we have a minimum relative deviation threshold configured
    let min_relative_deviation = config::audit_min_relative_deviation(measurement);
    let threshold_applied = min_relative_deviation.is_some();

    // MUTATION POINT: < vs == (Line 156)
    let passed_due_to_threshold = min_relative_deviation
        .map(|threshold| head_relative_deviation < threshold)
        .unwrap_or(false);

    let text_summary = build_summary();

    // MUTATION POINT: > vs >= (Line 178)
    let z_score_exceeds_sigma =
        head_summary.is_significant(&tail_summary, sigma, dispersion_method);

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
        let tail_values = [30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 100.0];

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
            Some(1),
            &[],
            Some(ReductionFunc::Mean),
            Some(2.0),
            Some(DispersionMethod::StandardDeviation),
            &[], // Empty filter patterns
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
    fn test_skip_with_summaries() {
        // Test that when audit is skipped, summaries are shown based on TOTAL measurement count
        // Total measurements = 1 head + N tail
        // and the format matches passing/failing cases

        // Test with 0 tail measurements (1 total): should show Head only
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![], // 0 tail measurements = 1 total measurement
            5,      // min_count > 0 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("Skipping test"));
        assert!(message.contains("Head:")); // Head summary shown
        assert!(!message.contains("z-score")); // No z-score (only 1 total measurement)
        assert!(!message.contains("Tail:")); // No tail
        assert!(!message.contains("[")); // No sparkline

        // Test with 1 tail measurement (2 total): should show everything
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0], // 1 tail measurement = 2 total measurements
            5,          // min_count > 1 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("Skipping test"));
        assert!(message.contains("z-score (stddev):")); // Z-score with method shown
        assert!(message.contains("Head:")); // Head summary shown
        assert!(message.contains("Tail:")); // Tail summary shown
        assert!(message.contains("[")); // Sparkline shown
                                        // Verify order: z-score, Head, Tail, sparkline
        let z_pos = message.find("z-score").unwrap();
        let head_pos = message.find("Head:").unwrap();
        let tail_pos = message.find("Tail:").unwrap();
        let spark_pos = message.find("[").unwrap();
        assert!(z_pos < head_pos, "z-score should come before Head");
        assert!(head_pos < tail_pos, "Head should come before Tail");
        assert!(tail_pos < spark_pos, "Tail should come before sparkline");

        // Test with 2 tail measurements (3 total): should show everything
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 11.0], // 2 tail measurements = 3 total measurements
            5,                // min_count > 2 to trigger skip
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("Skipping test"));
        assert!(message.contains("z-score (stddev):")); // Z-score with method shown
        assert!(message.contains("Head:")); // Head summary shown
        assert!(message.contains("Tail:")); // Tail summary shown
        assert!(message.contains("[")); // Sparkline shown
                                        // Verify order: z-score, Head, Tail, sparkline
        let z_pos = message.find("z-score").unwrap();
        let head_pos = message.find("Head:").unwrap();
        let tail_pos = message.find("Tail:").unwrap();
        let spark_pos = message.find("[").unwrap();
        assert!(z_pos < head_pos, "z-score should come before Head");
        assert!(head_pos < tail_pos, "Head should come before Tail");
        assert!(tail_pos < spark_pos, "Tail should come before sparkline");

        // Test with MAD dispersion method to ensure method name is correct
        let result = audit_with_data(
            "test_measurement",
            15.0,
            vec![10.0, 11.0], // 2 tail measurements = 3 total measurements
            5,                // min_count > 2 to trigger skip
            2.0,
            DispersionMethod::MedianAbsoluteDeviation,
        );

        assert!(result.is_ok());
        let message = result.unwrap().message;
        assert!(message.contains("z-score (mad):")); // MAD method shown
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

        // With division:
        // - relative_min = (10.0 / 10.0 - 1.0) * 100 = 0.0%
        // - relative_max = (25.0 / 10.0 - 1.0) * 100 = 150.0%
        // With modulo:
        // - relative_min = (10.0 % 10.0 - 1.0) * 100 = -100.0% (since 10.0 % 10.0 = 0.0)
        // - relative_max = (25.0 % 10.0 - 1.0) * 100 = -50.0% (since 25.0 % 10.0 = 5.0)

        // Check that the calculation uses division, not modulo
        // The range should show [+0.00% – +150.00%], not [-100.00% – -50.00%]
        assert!(audit_result.message.contains("[+0.00% – +150.00%]"));

        // Ensure the modulo results are NOT present
        assert!(!audit_result.message.contains("[-100.00% – -50.00%]"));
        assert!(!audit_result.message.contains("-100.00%"));
        assert!(!audit_result.message.contains("-50.00%"));
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

    #[test]
    fn test_head_and_tail_have_units_and_auto_scaling() {
        // Test that both head and tail measurements display units with auto-scaling

        // First, set up a test environment with a configured unit
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .output()
            .unwrap();

        // Create .gitperfconfig with unit configuration
        let config_content = r#"
[measurement."build_time"]
unit = "ms"
"#;
        let config_path = temp_dir.path().join(".gitperfconfig");
        fs::write(&config_path, config_content).unwrap();

        // Test with large millisecond values that should auto-scale to seconds
        let head = 12_345.67; // Will auto-scale to ~12.35s
        let tail = vec![10_000.0, 10_500.0, 11_000.0, 11_500.0, 12_000.0]; // Will auto-scale to 10s, 10.5s, 11s, etc.

        let result = audit_with_data(
            "build_time",
            head,
            tail,
            1,
            10.0, // High sigma to ensure it passes
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        let message = &audit_result.message;

        // Verify Head section exists
        assert!(
            message.contains("Head:"),
            "Message should contain Head section"
        );

        // With auto-scaling, 12345.67ms should become ~12.35s or 12.3s
        // Check that the value is auto-scaled (contains 's' for seconds)
        assert!(
            message.contains("12.3s") || message.contains("12.35s"),
            "Head mean should be auto-scaled to seconds, got: {}",
            message
        );

        let head_section: Vec<&str> = message
            .lines()
            .filter(|line| line.contains("Head:"))
            .collect();

        assert!(
            !head_section.is_empty(),
            "Should find Head section in message"
        );

        let head_line = head_section[0];

        // With auto-scaling, all values (mean, stddev, MAD) get their units auto-scaled
        // They should all have units now (not just mean)
        assert!(
            head_line.contains("μ:") && head_line.contains("σ:") && head_line.contains("MAD:"),
            "Head line should contain μ, σ, and MAD labels, got: {}",
            head_line
        );

        // Verify Tail section has units
        assert!(
            message.contains("Tail:"),
            "Message should contain Tail section"
        );

        let tail_section: Vec<&str> = message
            .lines()
            .filter(|line| line.contains("Tail:"))
            .collect();

        assert!(
            !tail_section.is_empty(),
            "Should find Tail section in message"
        );

        let tail_line = tail_section[0];

        // Tail mean should be auto-scaled to seconds (10000-12000ms → 10-12s)
        assert!(
            tail_line.contains("11s")
                || tail_line.contains("11.")
                || tail_line.contains("10.")
                || tail_line.contains("12."),
            "Tail should contain auto-scaled second values, got: {}",
            tail_line
        );

        // Verify the basic format structure is present
        assert!(
            tail_line.contains("μ:")
                && tail_line.contains("σ:")
                && tail_line.contains("MAD:")
                && tail_line.contains("n:"),
            "Tail line should contain all stat labels, got: {}",
            tail_line
        );
    }

    // Integration tests that verify per-measurement config determination
    #[cfg(test)]
    mod integration {
        use super::*;
        use crate::config::{
            audit_aggregate_by, audit_dispersion_method, audit_min_measurements, audit_sigma,
        };
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        fn setup_test_env_with_config(config_content: &str) -> TempDir {
            let temp_dir = TempDir::new().unwrap();

            // Initialize git repo
            env::set_current_dir(&temp_dir).unwrap();
            std::process::Command::new("git")
                .args(["init"])
                .output()
                .unwrap();
            std::process::Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .output()
                .unwrap();
            std::process::Command::new("git")
                .args(["config", "user.name", "Test User"])
                .output()
                .unwrap();

            // Create .gitperfconfig
            let config_path = temp_dir.path().join(".gitperfconfig");
            fs::write(&config_path, config_content).unwrap();

            temp_dir
        }

        #[test]
        fn test_different_dispersion_methods_per_measurement() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement]
dispersion_method = "stddev"

[measurement."build_time"]
dispersion_method = "mad"

[measurement."memory_usage"]
dispersion_method = "stddev"
"#,
            );

            // Verify each measurement gets its own config
            let build_time_method = audit_dispersion_method("build_time");
            let memory_usage_method = audit_dispersion_method("memory_usage");
            let other_method = audit_dispersion_method("other_metric");

            assert_eq!(
                DispersionMethod::from(build_time_method),
                DispersionMethod::MedianAbsoluteDeviation,
                "build_time should use MAD"
            );
            assert_eq!(
                DispersionMethod::from(memory_usage_method),
                DispersionMethod::StandardDeviation,
                "memory_usage should use stddev"
            );
            assert_eq!(
                DispersionMethod::from(other_method),
                DispersionMethod::StandardDeviation,
                "other_metric should use default stddev"
            );
        }

        #[test]
        fn test_different_min_measurements_per_measurement() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement]
min_measurements = 5

[measurement."build_time"]
min_measurements = 10

[measurement."memory_usage"]
min_measurements = 3
"#,
            );

            assert_eq!(
                audit_min_measurements("build_time"),
                Some(10),
                "build_time should require 10 measurements"
            );
            assert_eq!(
                audit_min_measurements("memory_usage"),
                Some(3),
                "memory_usage should require 3 measurements"
            );
            assert_eq!(
                audit_min_measurements("other_metric"),
                Some(5),
                "other_metric should use default 5 measurements"
            );
        }

        #[test]
        fn test_different_aggregate_by_per_measurement() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement]
aggregate_by = "median"

[measurement."build_time"]
aggregate_by = "max"

[measurement."memory_usage"]
aggregate_by = "mean"
"#,
            );

            assert_eq!(
                audit_aggregate_by("build_time"),
                Some(git_perf_cli_types::ReductionFunc::Max),
                "build_time should use max"
            );
            assert_eq!(
                audit_aggregate_by("memory_usage"),
                Some(git_perf_cli_types::ReductionFunc::Mean),
                "memory_usage should use mean"
            );
            assert_eq!(
                audit_aggregate_by("other_metric"),
                Some(git_perf_cli_types::ReductionFunc::Median),
                "other_metric should use default median"
            );
        }

        #[test]
        fn test_different_sigma_per_measurement() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement]
sigma = 3.0

[measurement."build_time"]
sigma = 5.5

[measurement."memory_usage"]
sigma = 2.0
"#,
            );

            assert_eq!(
                audit_sigma("build_time"),
                Some(5.5),
                "build_time should use sigma 5.5"
            );
            assert_eq!(
                audit_sigma("memory_usage"),
                Some(2.0),
                "memory_usage should use sigma 2.0"
            );
            assert_eq!(
                audit_sigma("other_metric"),
                Some(3.0),
                "other_metric should use default sigma 3.0"
            );
        }

        #[test]
        fn test_cli_overrides_config() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement."build_time"]
min_measurements = 10
aggregate_by = "max"
sigma = 5.5
dispersion_method = "mad"
"#,
            );

            // Test that CLI values override config
            let params = super::resolve_audit_params(
                "build_time",
                Some(2),                                   // CLI min
                Some(ReductionFunc::Min),                  // CLI aggregate
                Some(3.0),                                 // CLI sigma
                Some(DispersionMethod::StandardDeviation), // CLI dispersion
            );

            assert_eq!(
                params.min_count, 2,
                "CLI min_measurements should override config"
            );
            assert_eq!(
                params.summarize_by,
                ReductionFunc::Min,
                "CLI aggregate_by should override config"
            );
            assert_eq!(params.sigma, 3.0, "CLI sigma should override config");
            assert_eq!(
                params.dispersion_method,
                DispersionMethod::StandardDeviation,
                "CLI dispersion should override config"
            );
        }

        #[test]
        fn test_config_overrides_defaults() {
            let _temp_dir = setup_test_env_with_config(
                r#"
[measurement."build_time"]
min_measurements = 10
aggregate_by = "max"
sigma = 5.5
dispersion_method = "mad"
"#,
            );

            // Test that config values are used when no CLI values provided
            let params = super::resolve_audit_params(
                "build_time",
                None, // No CLI values
                None,
                None,
                None,
            );

            assert_eq!(
                params.min_count, 10,
                "Config min_measurements should override default"
            );
            assert_eq!(
                params.summarize_by,
                ReductionFunc::Max,
                "Config aggregate_by should override default"
            );
            assert_eq!(params.sigma, 5.5, "Config sigma should override default");
            assert_eq!(
                params.dispersion_method,
                DispersionMethod::MedianAbsoluteDeviation,
                "Config dispersion should override default"
            );
        }

        #[test]
        fn test_uses_defaults_when_no_config_or_cli() {
            let _temp_dir = setup_test_env_with_config("");

            // Test that defaults are used when no CLI or config
            let params = super::resolve_audit_params(
                "non_existent_measurement",
                None, // No CLI values
                None,
                None,
                None,
            );

            assert_eq!(
                params.min_count, 2,
                "Should use default min_measurements of 2"
            );
            assert_eq!(
                params.summarize_by,
                ReductionFunc::Min,
                "Should use default aggregate_by of Min"
            );
            assert_eq!(params.sigma, 4.0, "Should use default sigma of 4.0");
            assert_eq!(
                params.dispersion_method,
                DispersionMethod::StandardDeviation,
                "Should use default dispersion of stddev"
            );
        }
    }
}
