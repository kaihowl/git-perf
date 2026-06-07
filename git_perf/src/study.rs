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

    let dispersion_method = if mad_sigma_ratio < 0.7 && aggregates.len() >= 5 {
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

fn format_output(
    name: &str,
    aggregates: &[f64],
    grouped_by_key: bool,
    total_raw: usize,
    group_by: &str,
    max_cov_threshold: Option<f64>,
) -> String {
    let stats = aggregate_measurements(aggregates.iter());
    let n = aggregates.len();
    let cov = if stats.mean.abs() > f64::EPSILON && !stats.mean.is_nan() {
        stats.stddev / stats.mean * 100.0
    } else {
        f64::NAN
    };
    let mad_pct = if stats.mean.abs() > f64::EPSILON && !stats.mean.is_nan() {
        stats.mad / stats.mean.abs() * 100.0
    } else {
        f64::NAN
    };
    let mad_sigma_ratio = if stats.stddev > f64::EPSILON {
        stats.mad / stats.stddev
    } else {
        f64::NAN
    };

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
        if mad_sigma_ratio < 0.7 && n >= 5 && !mad_sigma_ratio.is_nan() {
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
        let cov = if stats.mean.abs() > f64::EPSILON {
            stats.stddev / stats.mean * 100.0
        } else {
            0.0
        };
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
        // CoV = 8.4% → min_relative_deviation = ceil(8.4 * 1.5 * 2) / 2 = ceil(25.2) / 2 = 26/2 = 13.0
        // We can't easily control exact CoV, but check that rounding is to nearest 0.5
        let recs = compute_recommendations(&[100.0, 108.0, 100.5, 107.5, 99.5, 108.5]).unwrap();
        // Result should be a multiple of 0.5
        let scaled = recs.min_relative_deviation * 2.0;
        assert!(
            (scaled - scaled.round()).abs() < f64::EPSILON,
            "min_relative_deviation should be a multiple of 0.5"
        );
        let scaled_cov = recs.max_cov * 2.0;
        assert!(
            (scaled_cov - scaled_cov.round()).abs() < f64::EPSILON,
            "max_cov should be a multiple of 0.5"
        );
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
}
