use crate::{
    config,
    data::{Commit, MeasurementData},
    measurement_retrieval::{self, summarize_measurements},
    stats::{self, DispersionMethod, ReductionFunc, StatsWithUnit, VecAggregation},
};
use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use log::error;
use sparklines::spark;
use std::cmp::Ordering;
use std::collections::HashSet;
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

/// Discovers all unique measurement names from commits that match the filters and selectors.
/// This is used to efficiently find which measurements to audit when filters are provided.
fn discover_matching_measurements(
    commits: &[Result<Commit>],
    filters: &[regex::Regex],
    selectors: &[(String, String)],
) -> Vec<String> {
    let mut unique_measurements = HashSet::new();

    for commit in commits.iter().flatten() {
        for measurement in &commit.measurements {
            // Check if measurement name matches any filter
            if !crate::filter::matches_any_filter(&measurement.name, filters) {
                continue;
            }

            // Check if measurement matches selectors
            if !measurement.key_values_is_superset_of(selectors) {
                continue;
            }

            // This measurement matches - add to set
            unique_measurements.insert(measurement.name.clone());
        }
    }

    // Convert to sorted vector for deterministic ordering
    let mut result: Vec<String> = unique_measurements.into_iter().collect();
    result.sort();
    result
}

#[allow(clippy::too_many_arguments)]
pub fn audit_multiple(
    max_count: usize,
    min_count: Option<u16>,
    selectors: &[(String, String)],
    summarize_by: Option<ReductionFunc>,
    sigma: Option<f64>,
    dispersion_method: Option<DispersionMethod>,
    combined_patterns: &[String],
    _no_change_point_warning: bool, // TODO: Implement change point warning in Phase 2
) -> Result<()> {
    // Early return if patterns are empty - nothing to audit
    if combined_patterns.is_empty() {
        return Ok(());
    }

    // Compile combined regex patterns (measurements as exact matches + filter patterns)
    // early to fail fast on invalid patterns
    let filters = crate::filter::compile_filters(combined_patterns)?;

    // Phase 1: Walk commits ONCE (optimization: scan commits only once)
    // Collect into Vec so we can reuse the data for multiple measurements
    let all_commits: Vec<Result<Commit>> =
        measurement_retrieval::walk_commits(max_count)?.collect();

    // Phase 2: Discover all measurements that match the combined patterns from the commit data
    // The combined_patterns already include both measurements (as exact regex) and filters (OR behavior)
    let measurements_to_audit = discover_matching_measurements(&all_commits, &filters, selectors);

    // If no measurements were discovered, provide appropriate error message
    if measurements_to_audit.is_empty() {
        // Check if we have any commits at all
        if all_commits.is_empty() {
            bail!("No commit at HEAD");
        }
        // Check if any commits have any measurements at all
        let has_any_measurements = all_commits.iter().any(|commit_result| {
            if let Ok(commit) = commit_result {
                !commit.measurements.is_empty()
            } else {
                false
            }
        });

        if !has_any_measurements {
            // No measurements exist in any commits - specific error for this case
            bail!("No measurement for HEAD");
        }
        // Measurements exist but don't match the patterns
        bail!("No measurements found matching the provided patterns");
    }

    let mut failed = false;

    // Phase 3: For each measurement, audit using the pre-loaded commit data
    for measurement in measurements_to_audit {
        let params = resolve_audit_params(
            &measurement,
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

        let result = audit_with_commits(
            &measurement,
            &all_commits,
            params.min_count,
            selectors,
            params.summarize_by,
            params.sigma,
            params.dispersion_method,
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

/// Audits a measurement using pre-loaded commit data.
/// This is more efficient than the old `audit` function when auditing multiple measurements,
/// as it reuses the same commit data instead of walking commits multiple times.
fn audit_with_commits(
    measurement: &str,
    commits: &[Result<Commit>],
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
    dispersion_method: DispersionMethod,
) -> Result<AuditResult> {
    // Convert Vec<Result<Commit>> into an iterator of Result<Commit> by cloning references
    // This is necessary because summarize_measurements expects an iterator of Result<Commit>
    let commits_iter = commits.iter().map(|r| match r {
        Ok(commit) => Ok(Commit {
            commit: commit.commit.clone(),
            measurements: commit.measurements.clone(),
        }),
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    });

    // Filter to only this specific measurement with matching selectors
    let filter_by =
        |m: &MeasurementData| m.name == measurement && m.key_values_is_superset_of(selectors);

    let mut aggregates = measurement_retrieval::take_while_same_epoch(summarize_measurements(
        commits_iter,
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

    // Calculate min and max once for use in both branches
    let min_val = all_measurements
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let max_val = all_measurements
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    // Tiered approach for sparkline display:
    // 1. If tail median is non-zero: use median as baseline, show percentages (default behavior)
    // 2. If tail median is zero: show absolute differences instead
    let tail_median_is_zero = tail_median.abs() < f64::EPSILON;

    let sparkline = if tail_median_is_zero {
        // Median is zero - show absolute range
        format!(
            " [{} – {}] {}",
            min_val,
            max_val,
            spark(all_measurements.as_slice())
        )
    } else {
        // MUTATION POINT: / vs % (Line 140)
        // Median is non-zero - use it as baseline for percentage ranges
        let relative_min = min_val / tail_median - 1.0;
        let relative_max = max_val / tail_median - 1.0;

        format!(
            " [{:+.2}% – {:+.2}%] {}",
            (relative_min * 100.0),
            (relative_max * 100.0),
            spark(all_measurements.as_slice())
        )
    };

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
        let plural_s = if number_measurements == 1 { "" } else { "s" };
        error!("Only {number_measurements} historical measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");

        let mut skip_message = format!(
            "⏭️ '{measurement}'\nOnly {number_measurements} historical measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test."
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
    // Calculate relative deviation - naturally handles infinity when tail_median is zero
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
    // Only show note when the audit would have failed without the threshold
    let threshold_note = if threshold_applied && passed_due_to_threshold && z_score_exceeds_sigma {
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
        // Tests the case where no patterns are provided (empty list)
        // With no patterns, it should succeed (nothing to audit)
        let result = audit_multiple(
            100,
            Some(1),
            &[],
            Some(ReductionFunc::Mean),
            Some(2.0),
            Some(DispersionMethod::StandardDeviation),
            &[], // Empty combined_patterns
            false,
        );

        // Should succeed when no measurements need to be audited
        assert!(
            result.is_ok(),
            "audit_multiple should succeed with empty pattern list"
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
        // COVERS MUTATION: number_measurements > 1 vs ==
        // Test with 0 measurements (should have 's' - grammatically correct)
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
        assert!(message.contains("0 historical measurements found")); // Has 's'
        assert!(!message.contains("0 historical measurement found")); // Should not be singular

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
        assert!(message.contains("1 historical measurement found")); // No 's'

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
        assert!(message.contains("2 historical measurements found")); // Has 's'
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
        use crate::test_helpers::setup_test_env_with_config;

        let config_content = r#"
[measurement."build_time"]
unit = "ms"
"#;
        let (_temp_dir, _dir_guard) = setup_test_env_with_config(config_content);

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

    #[test]
    fn test_threshold_note_only_shown_when_audit_would_fail() {
        // Test that the threshold note is only shown when the audit would have
        // failed without the threshold (i.e., when z_score_exceeds_sigma is true)
        use crate::test_helpers::setup_test_env_with_config;

        let config_content = r#"
[measurement."build_time"]
min_relative_deviation = 10.0
"#;
        let (_temp_dir, _dir_guard) = setup_test_env_with_config(config_content);

        // Case 1: Low z-score AND low relative deviation (threshold is configured but not needed)
        // Should pass without showing the note
        let result = audit_with_data(
            "build_time",
            10.1,                               // Very close to tail values
            vec![10.0, 10.1, 10.0, 10.1, 10.0], // Low variance
            1,
            100.0, // Very high sigma threshold - won't be exceeded
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(audit_result.passed);
        assert!(audit_result.message.contains("✅"));
        // The note should NOT be shown because the audit would have passed anyway
        assert!(
            !audit_result
                .message
                .contains("Note: Passed due to relative deviation"),
            "Note should not appear when audit passes without needing threshold bypass"
        );

        // Case 2: High z-score but low relative deviation (threshold saves the audit)
        // Should pass and show the note
        let result = audit_with_data(
            "build_time",
            1002.0, // High z-score outlier but low relative deviation
            vec![1000.0, 1000.1, 1000.0, 1000.1, 1000.0], // Very low variance
            1,
            0.5, // Low sigma threshold - will be exceeded
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(audit_result.passed);
        assert!(audit_result.message.contains("✅"));
        // The note SHOULD be shown because the audit would have failed without the threshold
        assert!(
            audit_result
                .message
                .contains("Note: Passed due to relative deviation"),
            "Note should appear when audit passes due to threshold bypass. Got: {}",
            audit_result.message
        );

        // Case 3: High z-score AND high relative deviation (threshold doesn't help)
        // Should fail
        let result = audit_with_data(
            "build_time",
            1200.0, // High z-score AND high relative deviation
            vec![1000.0, 1000.1, 1000.0, 1000.1, 1000.0], // Very low variance
            1,
            0.5, // Low sigma threshold - will be exceeded
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        assert!(!audit_result.passed);
        assert!(audit_result.message.contains("❌"));
        // No note shown because the audit still failed
        assert!(
            !audit_result
                .message
                .contains("Note: Passed due to relative deviation"),
            "Note should not appear when audit fails"
        );
    }

    // Integration tests that verify per-measurement config determination
    #[cfg(test)]
    mod integration {
        use super::*;
        use crate::config::{
            audit_aggregate_by, audit_dispersion_method, audit_min_measurements, audit_sigma,
        };
        use crate::test_helpers::setup_test_env_with_config;

        #[test]
        fn test_different_dispersion_methods_per_measurement() {
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config(
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
            let (_temp_dir, _dir_guard) = setup_test_env_with_config("");

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

    #[test]
    fn test_discover_matching_measurements() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        // Create mock commits with various measurements
        let commits = vec![
            Ok(Commit {
                commit: "abc123".to_string(),
                measurements: vec![
                    MeasurementData {
                        epoch: 0,
                        name: "bench_cpu".to_string(),
                        timestamp: 1000.0,
                        val: 100.0,
                        key_values: {
                            let mut map = HashMap::new();
                            map.insert("os".to_string(), "linux".to_string());
                            map
                        },
                    },
                    MeasurementData {
                        epoch: 0,
                        name: "bench_memory".to_string(),
                        timestamp: 1000.0,
                        val: 200.0,
                        key_values: {
                            let mut map = HashMap::new();
                            map.insert("os".to_string(), "linux".to_string());
                            map
                        },
                    },
                    MeasurementData {
                        epoch: 0,
                        name: "test_unit".to_string(),
                        timestamp: 1000.0,
                        val: 50.0,
                        key_values: {
                            let mut map = HashMap::new();
                            map.insert("os".to_string(), "linux".to_string());
                            map
                        },
                    },
                ],
            }),
            Ok(Commit {
                commit: "def456".to_string(),
                measurements: vec![
                    MeasurementData {
                        epoch: 0,
                        name: "bench_cpu".to_string(),
                        timestamp: 1000.0,
                        val: 105.0,
                        key_values: {
                            let mut map = HashMap::new();
                            map.insert("os".to_string(), "mac".to_string());
                            map
                        },
                    },
                    MeasurementData {
                        epoch: 0,
                        name: "other_metric".to_string(),
                        timestamp: 1000.0,
                        val: 75.0,
                        key_values: {
                            let mut map = HashMap::new();
                            map.insert("os".to_string(), "linux".to_string());
                            map
                        },
                    },
                ],
            }),
        ];

        // Test 1: Single filter pattern matching "bench_*"
        let patterns = vec!["bench_.*".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        assert_eq!(discovered.len(), 2);
        assert!(discovered.contains(&"bench_cpu".to_string()));
        assert!(discovered.contains(&"bench_memory".to_string()));
        assert!(!discovered.contains(&"test_unit".to_string()));
        assert!(!discovered.contains(&"other_metric".to_string()));

        // Test 2: Multiple filter patterns (OR behavior)
        let patterns = vec!["bench_cpu".to_string(), "test_.*".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        assert_eq!(discovered.len(), 2);
        assert!(discovered.contains(&"bench_cpu".to_string()));
        assert!(discovered.contains(&"test_unit".to_string()));
        assert!(!discovered.contains(&"bench_memory".to_string()));

        // Test 3: Filter with selectors
        let patterns = vec!["bench_.*".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![("os".to_string(), "linux".to_string())];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        // bench_cpu and bench_memory both have os=linux (in first commit)
        // bench_cpu also has os=mac (in second commit) but selector filters it to only linux
        assert_eq!(discovered.len(), 2);
        assert!(discovered.contains(&"bench_cpu".to_string()));
        assert!(discovered.contains(&"bench_memory".to_string()));

        // Test 4: No matches
        let patterns = vec!["nonexistent.*".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        assert_eq!(discovered.len(), 0);

        // Test 5: Empty filters (should match all)
        let filters = vec![];
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        // Empty filters should match nothing based on the logic
        // Actually, looking at matches_any_filter, empty filters return true
        // So this should discover all measurements
        assert_eq!(discovered.len(), 4);
        assert!(discovered.contains(&"bench_cpu".to_string()));
        assert!(discovered.contains(&"bench_memory".to_string()));
        assert!(discovered.contains(&"test_unit".to_string()));
        assert!(discovered.contains(&"other_metric".to_string()));

        // Test 6: Selector filters out everything
        let patterns = vec!["bench_.*".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![("os".to_string(), "windows".to_string())];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        assert_eq!(discovered.len(), 0);

        // Test 7: Exact match with anchored regex (simulating -m argument)
        let patterns = vec!["^bench_cpu$".to_string()];
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        assert_eq!(discovered.len(), 1);
        assert!(discovered.contains(&"bench_cpu".to_string()));

        // Test 8: Sorted output (verify deterministic ordering)
        let patterns = vec![".*".to_string()]; // Match all
        let filters = crate::filter::compile_filters(&patterns).unwrap();
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        // Should be sorted alphabetically
        assert_eq!(discovered[0], "bench_cpu");
        assert_eq!(discovered[1], "bench_memory");
        assert_eq!(discovered[2], "other_metric");
        assert_eq!(discovered[3], "test_unit");
    }

    #[test]
    fn test_audit_multiple_with_combined_patterns() {
        // This test verifies that combining explicit measurements (-m) and filter patterns (--filter)
        // works correctly with OR behavior. Both should be audited.
        // Note: This is an integration test that uses actual audit_multiple function,
        // but we can't easily test it without a real git repo, so we test the pattern combination
        // and discovery logic instead.

        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        // Create mock commits
        let commits = vec![Ok(Commit {
            commit: "abc123".to_string(),
            measurements: vec![
                MeasurementData {
                    epoch: 0,
                    name: "timer".to_string(),
                    timestamp: 1000.0,
                    val: 10.0,
                    key_values: HashMap::new(),
                },
                MeasurementData {
                    epoch: 0,
                    name: "bench_cpu".to_string(),
                    timestamp: 1000.0,
                    val: 100.0,
                    key_values: HashMap::new(),
                },
                MeasurementData {
                    epoch: 0,
                    name: "memory".to_string(),
                    timestamp: 1000.0,
                    val: 500.0,
                    key_values: HashMap::new(),
                },
            ],
        })];

        // Simulate combining -m timer with --filter "bench_.*"
        // This is what combine_measurements_and_filters does in cli.rs
        let measurements = vec!["timer".to_string()];
        let filter_patterns = vec!["bench_.*".to_string()];
        let combined =
            crate::filter::combine_measurements_and_filters(&measurements, &filter_patterns);

        // combined should have: ["^timer$", "bench_.*"]
        assert_eq!(combined.len(), 2);
        assert_eq!(combined[0], "^timer$");
        assert_eq!(combined[1], "bench_.*");

        // Now compile and discover
        let filters = crate::filter::compile_filters(&combined).unwrap();
        let selectors = vec![];
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        // Should discover both timer (exact match) and bench_cpu (pattern match)
        assert_eq!(discovered.len(), 2);
        assert!(discovered.contains(&"timer".to_string()));
        assert!(discovered.contains(&"bench_cpu".to_string()));
        assert!(!discovered.contains(&"memory".to_string())); // Not in -m or filter

        // Test with multiple explicit measurements and multiple filters
        let measurements = vec!["timer".to_string(), "memory".to_string()];
        let filter_patterns = vec!["bench_.*".to_string(), "test_.*".to_string()];
        let combined =
            crate::filter::combine_measurements_and_filters(&measurements, &filter_patterns);

        assert_eq!(combined.len(), 4);

        let filters = crate::filter::compile_filters(&combined).unwrap();
        let discovered = discover_matching_measurements(&commits, &filters, &selectors);

        // Should discover timer, memory, and bench_cpu (no test_* in commits)
        assert_eq!(discovered.len(), 3);
        assert!(discovered.contains(&"timer".to_string()));
        assert!(discovered.contains(&"memory".to_string()));
        assert!(discovered.contains(&"bench_cpu".to_string()));
    }

    #[test]
    fn test_audit_with_empty_tail() {
        // Test for division by zero bug when tail is empty
        // This test reproduces the bug where tail_median is 0.0 when tail is empty,
        // causing division by zero in sparkline calculation
        let result = audit_with_data(
            "test_measurement",
            10.0,   // head
            vec![], // empty tail - triggers the bug
            2,      // min_count
            2.0,    // sigma
            DispersionMethod::StandardDeviation,
        );

        // Should succeed and skip (not crash with division by zero)
        assert!(result.is_ok(), "Should not crash on empty tail");
        let audit_result = result.unwrap();

        // Should be skipped due to insufficient measurements
        assert!(audit_result.passed);
        assert!(audit_result.message.contains("Skipping test"));

        // The message should not contain inf or NaN
        assert!(!audit_result.message.to_lowercase().contains("inf"));
        assert!(!audit_result.message.to_lowercase().contains("nan"));
    }

    #[test]
    fn test_audit_with_all_zero_tail() {
        // Test for division by zero when all tail measurements are 0.0
        // This tests the edge case where median is 0.0 even with measurements
        let result = audit_with_data(
            "test_measurement",
            5.0,                 // non-zero head
            vec![0.0, 0.0, 0.0], // all zeros in tail
            2,                   // min_count
            2.0,                 // sigma
            DispersionMethod::StandardDeviation,
        );

        // Should succeed (not crash with division by zero)
        assert!(result.is_ok(), "Should not crash when tail median is 0.0");
        let audit_result = result.unwrap();

        // The message should not contain inf or NaN
        assert!(!audit_result.message.to_lowercase().contains("inf"));
        assert!(!audit_result.message.to_lowercase().contains("nan"));
    }

    #[test]
    fn test_tiered_baseline_approach() {
        // Test the tiered approach:
        // 1. Non-zero median → use median, show percentages
        // 2. Zero median → show absolute values

        // Case 1: Median is non-zero - use percentages (default behavior)
        let result = audit_with_data(
            "test_measurement",
            15.0,                   // head
            vec![10.0, 11.0, 12.0], // median=11.0 (non-zero)
            2,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        // Should use median as baseline and show percentage
        assert!(audit_result.message.contains('%'));
        assert!(!audit_result.message.to_lowercase().contains("inf"));

        // Case 2: Median is zero with non-zero head - use absolute values
        let result = audit_with_data(
            "test_measurement",
            5.0,                 // head (non-zero)
            vec![0.0, 0.0, 0.0], // median=0
            2,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        // Should show absolute values instead of percentages
        // The message should contain the sparkline but not percentage symbols
        assert!(!audit_result.message.to_lowercase().contains("inf"));
        assert!(!audit_result.message.to_lowercase().contains("nan"));
        // Check that sparkline exists (contains the dash character)
        assert!(audit_result.message.contains('–') || audit_result.message.contains('-'));

        // Case 3: Everything is zero - show absolute values [0 - 0]
        let result = audit_with_data(
            "test_measurement",
            0.0,                 // head
            vec![0.0, 0.0, 0.0], // median=0
            2,
            2.0,
            DispersionMethod::StandardDeviation,
        );

        assert!(result.is_ok());
        let audit_result = result.unwrap();
        // Should show absolute range [0 - 0]
        assert!(!audit_result.message.to_lowercase().contains("inf"));
        assert!(!audit_result.message.to_lowercase().contains("nan"));
    }
}
