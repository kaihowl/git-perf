use std::collections::HashMap;

use anyhow::{bail, Result};
use readable::num::Float;

use crate::{
    data::MeasurementData,
    measurement_retrieval,
    stats::{aggregate_measurements, NumericReductionFunc, ReductionFunc},
};

pub struct Recommendations {
    pub dispersion_method: &'static str,
    pub aggregate_by: &'static str,
    pub sigma: f64,
    pub min_measurements: u16,
    pub min_relative_deviation: f64,
    pub max_cov: f64,
}

struct GroupedStudy {
    /// Per-group aggregated values (one per independent runner instance)
    group_aggregates: Vec<f64>,
    /// Total raw measurement count across all groups
    total_raw: usize,
    /// Whether grouping by key succeeded (vs. raw fallback)
    grouped_by_key: bool,
}

fn group_measurements(measurements: &[&MeasurementData], key: &str) -> GroupedStudy {
    let total_raw = measurements.len();
    let mut groups: HashMap<String, Vec<f64>> = HashMap::new();
    for m in measurements {
        let group_val = m.key_values.get(key).cloned().unwrap_or_default();
        groups.entry(group_val).or_default().push(m.val);
    }

    // Only use grouped CoV if the key is actually present and produces ≥2 groups
    // (a single group keyed by "" means no runner tagged their measurements).
    let non_empty_key_groups = groups.keys().filter(|k| !k.is_empty()).count();
    if non_empty_key_groups >= 2 {
        let group_aggregates = groups
            .iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(_, vals)| {
                vals.iter()
                    .cloned()
                    .aggregate_by(ReductionFunc::Min)
                    .unwrap_or(f64::NAN)
            })
            .collect();
        GroupedStudy {
            group_aggregates,
            total_raw,
            grouped_by_key: true,
        }
    } else {
        // Fallback: treat each raw measurement as an independent sample
        let group_aggregates = measurements.iter().map(|m| m.val).collect();
        GroupedStudy {
            group_aggregates,
            total_raw,
            grouped_by_key: false,
        }
    }
}

/// Returns true if MAD is preferred over stddev as the dispersion method.
/// Requires a low MAD/σ ratio (outliers present) AND at least 5 data points.
fn is_mad_preferred(mad_sigma_ratio: f64, n: usize) -> bool {
    mad_sigma_ratio < 0.7 && n >= 5
}

/// Compute recommendations from between-group aggregate values.
/// Returns None if there are fewer than 3 data points.
#[must_use]
pub fn compute_recommendations(aggregates: &[f64]) -> Option<Recommendations> {
    if aggregates.len() < 3 {
        return None;
    }
    let stats = aggregate_measurements(aggregates.iter());
    if stats.mean.abs() <= f64::EPSILON || stats.mean.is_nan() {
        return None;
    }

    let cov = stats.stddev / stats.mean * 100.0;
    let mad_sigma_ratio = if stats.stddev > f64::EPSILON {
        stats.mad / stats.stddev
    } else {
        1.0
    };

    let dispersion_method = if is_mad_preferred(mad_sigma_ratio, aggregates.len()) {
        "mad"
    } else {
        "stddev"
    };
    let aggregate_by = if cov > 10.0 { "median" } else { "min" };
    let sigma = if cov > 5.0 { 3.5_f64 } else { 4.0_f64 };
    let min_measurements: u16 = if cov > 10.0 { 5 } else { 3 };
    // Round up to nearest 0.5 for readability
    let min_relative_deviation = (cov * 1.5 * 2.0).ceil() / 2.0;
    let max_cov = (cov * 2.0 * 2.0).ceil() / 2.0;

    Some(Recommendations {
        dispersion_method,
        aggregate_by,
        sigma,
        min_measurements,
        min_relative_deviation,
        max_cov,
    })
}

/// Compute between-group CoV as a percentage. Returns NaN when mean is near zero.
fn compute_cov_pct(stddev: f64, mean: f64) -> f64 {
    if mean.abs() > f64::EPSILON && !mean.is_nan() {
        stddev / mean * 100.0
    } else {
        f64::NAN
    }
}

/// Compute MAD as a percentage of the mean. Returns NaN when mean is near zero.
fn compute_mad_pct(mad: f64, mean: f64) -> f64 {
    if mean.abs() > f64::EPSILON && !mean.is_nan() {
        mad / mean.abs() * 100.0
    } else {
        f64::NAN
    }
}

/// Compute MAD/σ ratio. Returns NaN when stddev is near zero.
fn compute_mad_sigma_ratio(mad: f64, stddev: f64) -> f64 {
    if stddev > f64::EPSILON {
        mad / stddev
    } else {
        f64::NAN
    }
}

pub(crate) fn format_output(
    name: &str,
    aggregates: &[f64],
    grouped_by_key: bool,
    total_raw: usize,
    group_by: &str,
    max_cov_threshold: Option<f64>,
) -> String {
    let stats = aggregate_measurements(aggregates.iter());
    let n = aggregates.len();
    let cov = compute_cov_pct(stats.stddev, stats.mean);
    let mad_pct = compute_mad_pct(stats.mad, stats.mean);
    let mad_sigma_ratio = compute_mad_sigma_ratio(stats.mad, stats.stddev);

    let cov_label = if grouped_by_key {
        "Between-group CoV"
    } else {
        "Overall CoV (no group key found)"
    };

    let grouping_note = if grouped_by_key {
        format!(
            "{n} groups × {} reps (grouped by: {group_by})",
            total_raw / n
        )
    } else {
        format!(
            "{total_raw} raw measurements (no '{group_by}' key found — \
             tag runners with --key-value {group_by}=<instance> for between-runner CoV)"
        )
    };

    let cov_str = if cov.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:.1}%", cov)
    };
    let mad_pct_str = if mad_pct.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:.1}%", mad_pct)
    };
    let mad_sigma_str = if mad_sigma_ratio.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:.2}", mad_sigma_ratio)
    };

    let mut out = format!(
        "📊 '{}' — {}\n  μ: {} | σ: {} | MAD: {}\n  {}: {} | MAD%: {} | MAD/σ: {}\n",
        name,
        grouping_note,
        Float::from(stats.mean),
        Float::from(stats.stddev),
        Float::from(stats.mad),
        cov_label,
        cov_str,
        mad_pct_str,
        mad_sigma_str,
    );

    // CoV verdict
    if !cov.is_nan() {
        let verdict = if let Some(threshold) = max_cov_threshold {
            if cov > threshold {
                format!(
                    "\n  ⚠️  CoV {:.1}% exceeds threshold {:.1}% — \
                     benchmark may produce unreliable CI results.\n\
                     Consider: increasing workload size, adding warmup, \
                     or reducing setup variance.",
                    cov, threshold
                )
            } else {
                format!(
                    "\n  ✅ CoV {:.1}% is within threshold {:.1}%.",
                    cov, threshold
                )
            }
        } else if cov > 20.0 {
            "\n  ⚠️  CoV > 20%: benchmark is too noisy for reliable regression detection.\n\
             Consider increasing workload size or reducing setup variance."
                .to_string()
        } else if cov > 10.0 {
            "\n  ⚠️  CoV 10–20%: moderate noise. Monitor with max_cov in config.".to_string()
        } else {
            "\n  ✅ CoV < 10%: benchmark is stable.".to_string()
        };
        out.push_str(&verdict);
    }

    // Recommended config
    if let Some(recs) = compute_recommendations(aggregates) {
        out.push_str(&format!(
            "\n\n  Recommended .gitperfconfig:\n\
             \n  [measurement.\"{name}\"]\n\
             \n  dispersion_method = \"{method}\"",
            method = recs.dispersion_method,
        ));
        if is_mad_preferred(mad_sigma_ratio, n) && !mad_sigma_ratio.is_nan() {
            out.push_str(&format!(
                "  # MAD/σ = {:.2} — outliers between runners detected",
                mad_sigma_ratio
            ));
        }
        out.push_str(&format!("\n  sigma = {}", recs.sigma));
        if cov > 5.0 {
            out.push_str("  # tightened threshold for CoV > 5%");
        }
        out.push_str(&format!("\n  aggregate_by = \"{}\"", recs.aggregate_by));
        if cov > 10.0 {
            out.push_str("  # CoV > 10% → median more stable than min");
        }
        out.push_str(&format!("\n  min_measurements = {}", recs.min_measurements));
        if cov > 10.0 {
            out.push_str("  # CoV > 10% → need more history");
        }
        out.push_str(&format!(
            "\n  min_relative_deviation = {}  # 1.5× between-group CoV — noise floor",
            recs.min_relative_deviation
        ));
        out.push_str(&format!(
            "\n  max_cov = {}  # warn if noise grows to 2× current level\n",
            recs.max_cov
        ));
    } else {
        out.push_str(
            "\n\n  Not enough data points for recommendations (need ≥ 3 groups).\n\
             Run the benchmark on more independent runner instances.",
        );
    }

    out
}

pub fn run_study(
    commit: &str,
    max_count: usize,
    name: &str,
    max_cov_threshold: Option<f64>,
    group_by: &str,
) -> Result<()> {
    let commits: Vec<_> =
        measurement_retrieval::walk_commits_from(commit, max_count, None, None)?.collect();

    let head_measurements: Vec<&MeasurementData> = commits
        .first()
        .and_then(|r| r.as_ref().ok())
        .map(|c| c.measurements.iter().filter(|m| m.name == name).collect())
        .unwrap_or_default();

    if head_measurements.is_empty() {
        bail!(
            "No measurements found for '{}' at HEAD.\n\
             Have you run 'git-perf measure' or 'git-perf push && git-perf pull'?",
            name
        );
    }

    let GroupedStudy {
        group_aggregates,
        total_raw,
        grouped_by_key,
    } = group_measurements(&head_measurements, group_by);

    if group_aggregates.len() < 3 {
        bail!(
            "Need at least 3 independent data points for a reliable study (found {}).\n\
             Run the benchmark on more independent runner instances and tag each with:\n\
               --key-value {}=<instance_number>",
            group_aggregates.len(),
            group_by
        );
    }

    let output = format_output(
        name,
        &group_aggregates,
        grouped_by_key,
        total_raw,
        group_by,
        max_cov_threshold,
    );
    print!("{}", output);

    if let Some(threshold) = max_cov_threshold {
        let stats = aggregate_measurements(group_aggregates.iter());
        let cov = compute_cov_pct(stats.stddev, stats.mean);
        // NaN > threshold is false per IEEE 754, so near-zero means safely skip the gate
        if cov > threshold {
            bail!("CoV {:.1}% exceeds threshold {:.1}%", cov, threshold);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uniform(vals: &[f64]) -> Vec<f64> {
        vals.to_vec()
    }

    #[test]
    fn test_recommend_low_cov() {
        // Very tight data → low CoV → stddev, min, sigma=4.0, min_measurements=3
        let data = make_uniform(&[100.0, 101.0, 100.5, 99.5, 100.2, 100.8]);
        let recs = compute_recommendations(&data).expect("should have recommendations");
        assert_eq!(recs.dispersion_method, "stddev");
        assert_eq!(recs.aggregate_by, "min");
        assert!((recs.sigma - 4.0).abs() < f64::EPSILON);
        assert_eq!(recs.min_measurements, 3);
        // min_relative_deviation should be small (CoV < 5%)
        assert!(recs.min_relative_deviation < 10.0);
    }

    #[test]
    fn test_recommend_high_cov() {
        // Wide spread → high CoV > 10% → mad, median, sigma=3.5, min_measurements=5
        let data: Vec<f64> = vec![100.0, 115.0, 90.0, 120.0, 85.0, 110.0, 95.0, 125.0];
        let recs = compute_recommendations(&data).expect("should have recommendations");
        // High CoV should trigger median and more min_measurements
        assert_eq!(recs.aggregate_by, "median");
        assert_eq!(recs.min_measurements, 5);
        assert!((recs.sigma - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recommend_insufficient_data() {
        assert!(compute_recommendations(&[100.0]).is_none());
        assert!(compute_recommendations(&[100.0, 101.0]).is_none());
        assert!(compute_recommendations(&[]).is_none());
    }

    #[test]
    fn test_recommend_rounding() {
        // data: [100.0, 108.0, 100.5, 107.5, 99.5, 108.5]
        // mean=104, stddev=sqrt(97/5)≈4.405, cov≈4.235%
        // min_relative_deviation = ceil(4.235 * 1.5 * 2.0) / 2.0 = ceil(12.705) / 2 = 6.5
        // max_cov              = ceil(4.235 * 2.0 * 2.0) / 2.0 = ceil(16.94)  / 2 = 8.5
        let recs = compute_recommendations(&[100.0, 108.0, 100.5, 107.5, 99.5, 108.5]).unwrap();
        // Check exact values so mutations to the rounding formula are caught
        assert!(
            (recs.min_relative_deviation - 6.5).abs() < 0.01,
            "expected min_relative_deviation=6.5, got {}",
            recs.min_relative_deviation
        );
        assert!(
            (recs.max_cov - 8.5).abs() < 0.01,
            "expected max_cov=8.5, got {}",
            recs.max_cov
        );
        // Result should also be a multiple of 0.5
        let scaled = recs.min_relative_deviation * 2.0;
        assert!(
            (scaled - scaled.round()).abs() < f64::EPSILON,
            "min_relative_deviation should be a multiple of 0.5"
        );
    }

    #[test]
    fn test_recommend_dispersion_method_boundary() {
        // n=4 with one extreme outlier: MAD/σ ≈ 0, but n < 5 → "stddev"
        let data_4 = vec![100.0, 100.0, 100.0, 200.0];
        let recs_4 = compute_recommendations(&data_4).unwrap();
        assert_eq!(
            recs_4.dispersion_method, "stddev",
            "n=4 should use stddev regardless of MAD/σ"
        );

        // n=5 same pattern: MAD/σ ≈ 0 and n >= 5 → "mad"
        let data_5 = vec![100.0, 100.0, 100.0, 100.0, 200.0];
        let recs_5 = compute_recommendations(&data_5).unwrap();
        assert_eq!(
            recs_5.dispersion_method, "mad",
            "n=5 with low MAD/σ should use mad"
        );
    }

    #[test]
    fn test_compute_helpers() {
        // compute_cov_pct: normal input
        let cov = compute_cov_pct(10.0, 100.0);
        assert!((cov - 10.0).abs() < 1e-9, "cov should be 10%, got {cov}");

        // compute_cov_pct: near-zero mean → NaN
        assert!(compute_cov_pct(1.0, 0.0).is_nan());

        // compute_cov_pct: exact-EPSILON mean → NaN (EPSILON > EPSILON is false)
        assert!(compute_cov_pct(1.0, f64::EPSILON).is_nan());

        // compute_mad_pct: normal input
        let mp = compute_mad_pct(5.0, 100.0);
        assert!((mp - 5.0).abs() < 1e-9, "mad_pct should be 5%, got {mp}");

        // compute_mad_pct: near-zero mean → NaN
        assert!(compute_mad_pct(1.0, 0.0).is_nan());

        // compute_mad_sigma_ratio: normal input
        let r = compute_mad_sigma_ratio(3.0, 6.0);
        assert!((r - 0.5).abs() < 1e-9, "ratio should be 0.5, got {r}");

        // compute_mad_sigma_ratio: near-zero stddev → NaN
        assert!(compute_mad_sigma_ratio(1.0, 0.0).is_nan());
    }

    #[test]
    fn test_format_output_verdict_low_cov() {
        // CoV < 10% → "stable" verdict
        let aggregates = vec![100.0, 100.5, 99.5, 100.2, 100.8, 99.8];
        let out = format_output("bench", &aggregates, true, 6, "group", None);
        assert!(
            out.contains("stable"),
            "low CoV should give stable verdict:\n{out}"
        );
        assert!(out.contains("Between-group CoV"), "grouped output:\n{out}");
    }

    #[test]
    fn test_format_output_verdict_moderate_cov() {
        // CoV 10–20%: values with ~13% spread
        let aggregates = vec![100.0, 112.0, 88.0, 115.0, 85.0, 110.0];
        let out = format_output("bench", &aggregates, true, 60, "group", None);
        // Moderate verdict contains "10–20%" and does not contain the stable/noisy verdicts
        assert!(
            out.contains("10\u{2013}20%"),
            "moderate CoV should show 10–20% range:\n{out}"
        );
        assert!(
            !out.contains("benchmark is stable"),
            "moderate should NOT say 'benchmark is stable':\n{out}"
        );
        assert!(
            !out.contains("too noisy"),
            "moderate should NOT say too noisy:\n{out}"
        );
    }

    #[test]
    fn test_format_output_verdict_high_cov() {
        // CoV > 20%: wide spread
        let aggregates = vec![100.0, 130.0, 70.0, 145.0, 60.0, 125.0, 75.0];
        let out = format_output("bench", &aggregates, true, 700, "group", None);
        assert!(
            out.contains("too noisy"),
            "high CoV should give too noisy verdict:\n{out}"
        );
        assert!(
            !out.contains("benchmark is stable"),
            "high CoV should NOT say 'benchmark is stable':\n{out}"
        );
    }

    #[test]
    fn test_format_output_threshold_exceeded() {
        // CoV > threshold → "exceeds threshold"
        let aggregates = vec![100.0, 115.0, 90.0, 120.0, 85.0, 110.0];
        let out = format_output("bench", &aggregates, true, 60, "group", Some(5.0));
        assert!(
            out.contains("exceeds threshold"),
            "high CoV with low threshold:\n{out}"
        );
    }

    #[test]
    fn test_format_output_threshold_passed() {
        // CoV < threshold → "within threshold"
        let aggregates = vec![100.0, 100.5, 99.5, 100.2, 100.8, 99.8];
        let out = format_output("bench", &aggregates, true, 6, "group", Some(50.0));
        assert!(
            out.contains("within threshold"),
            "low CoV with high threshold:\n{out}"
        );
    }

    #[test]
    fn test_format_output_mad_outlier_comment() {
        // n=5, outlier → low MAD/σ → "outliers" comment in config block
        let aggregates = vec![100.0, 100.0, 100.0, 100.0, 200.0];
        let out = format_output("bench", &aggregates, true, 5, "group", None);
        assert!(
            out.contains("outliers"),
            "low MAD/σ with n>=5 should show outliers comment:\n{out}"
        );
    }

    #[test]
    fn test_format_output_no_mad_comment_n_lt_5() {
        // n=4, same pattern → no "outliers" comment because n < 5
        let aggregates = vec![100.0, 100.0, 100.0, 200.0];
        let out = format_output("bench", &aggregates, true, 4, "group", None);
        assert!(
            !out.contains("outliers"),
            "n=4 should NOT show outliers comment:\n{out}"
        );
    }

    #[test]
    fn test_format_output_cov_numeric_in_output() {
        // For tight data, CoV should appear as a small % (< 2%), not thousands%
        // This catches mutations to the CoV division formula
        let aggregates = vec![100.0, 100.5, 99.5, 100.2, 100.8, 99.8];
        let out = format_output("bench", &aggregates, false, 6, "group", None);
        assert!(out.contains("Overall CoV"), "fallback label:\n{out}");
        // "stable" verdict only appears when CoV < 10%; mutating / to * gives CoV ≈ 5000%
        assert!(
            out.contains("stable"),
            "small CoV should give stable:\n{out}"
        );
        // MAD% and MAD/σ should be present (not N/A) for valid data
        assert!(
            !out.contains("MAD%: N/A"),
            "MAD% should not be N/A for valid data:\n{out}"
        );
        assert!(
            !out.contains("MAD/σ: N/A"),
            "MAD/σ should not be N/A for valid data:\n{out}"
        );
    }

    #[test]
    fn test_format_output_fallback_grouping_note() {
        let aggregates = vec![100.0, 105.0, 95.0];
        let out = format_output("bench", &aggregates, false, 3, "group", None);
        assert!(out.contains("raw measurements"), "fallback note:\n{out}");
        assert!(out.contains("no 'group' key"), "missing key note:\n{out}");
    }

    #[test]
    fn test_group_by_key() {
        let mut m1 = MeasurementData {
            epoch: 0,
            name: "t".to_string(),
            timestamp: 0.0,
            val: 100.0,
            key_values: std::collections::HashMap::new(),
        };
        m1.key_values.insert("group".to_string(), "1".to_string());

        let mut m2 = m1.clone();
        m2.val = 200.0;
        m2.key_values.insert("group".to_string(), "2".to_string());

        let mut m3 = m1.clone();
        m3.val = 300.0;
        m3.key_values.insert("group".to_string(), "3".to_string());

        // Add a second rep to group 1 (lower value — should be min)
        let mut m1b = m1.clone();
        m1b.val = 90.0;

        let measurements = vec![&m1, &m1b, &m2, &m3];
        let grouped = group_measurements(&measurements, "group");

        assert!(grouped.grouped_by_key);
        assert_eq!(grouped.group_aggregates.len(), 3);
        assert_eq!(grouped.total_raw, 4);

        // Group 1 should have min(100, 90) = 90
        assert!(grouped.group_aggregates.contains(&90.0));
        assert!(grouped.group_aggregates.contains(&200.0));
        assert!(grouped.group_aggregates.contains(&300.0));
    }

    #[test]
    fn test_group_by_key_fallback() {
        // No group key → fallback to raw values
        let m1 = MeasurementData {
            epoch: 0,
            name: "t".to_string(),
            timestamp: 0.0,
            val: 100.0,
            key_values: std::collections::HashMap::new(),
        };
        let m2 = MeasurementData {
            val: 110.0,
            ..m1.clone()
        };
        let m3 = MeasurementData {
            val: 95.0,
            ..m1.clone()
        };

        let measurements = vec![&m1, &m2, &m3];
        let grouped = group_measurements(&measurements, "group");

        assert!(!grouped.grouped_by_key);
        assert_eq!(grouped.group_aggregates.len(), 3);
        assert_eq!(grouped.total_raw, 3);
    }

    #[test]
    fn test_is_mad_preferred_boundary() {
        // At the exact literal boundary 0.7: strict < is false, <= would be true — kills < vs <= mutant
        assert!(!is_mad_preferred(0.7, 5));
        // Below threshold: < and <= both true
        assert!(is_mad_preferred(0.6, 5));
        // Above threshold: < and <= both false
        assert!(!is_mad_preferred(0.8, 5));
        // n=4 boundary: never preferred regardless of ratio
        assert!(!is_mad_preferred(0.5, 4));
        // n=5 with low ratio: preferred
        assert!(is_mad_preferred(0.5, 5));
    }
}
