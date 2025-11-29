//! Change point detection using the PELT (Pruned Exact Linear Time) algorithm.
//!
//! This module provides functionality to detect regime shifts in time series data,
//! helping identify when performance characteristics changed significantly.
//!
//! ## Algorithm Details
//!
//! PELT uses a penalty parameter to control sensitivity to change points. The algorithm
//! minimizes: `cost + penalty * number_of_change_points`, where cost measures how well
//! segments fit the data.
//!
//! ## Tuning the Penalty Parameter
//!
//! - **Lower penalty** (e.g., 0.5): More sensitive, detects multiple change points
//! - **Higher penalty** (e.g., 3.0+): Conservative, only detects major changes
//! - **Default**: 0.5 - balanced for detecting multiple significant changes
//!
//! Configure via `.gitperfconfig`:
//! ```toml
//! [change_point]
//! penalty = 0.5  # Global default
//!
//! [change_point."specific_measurement"]
//! penalty = 1.0  # Per-measurement override
//! ```
//!
//! The scaled penalty used internally is: `penalty * log(n) * variance`
//! This adapts to data size and variability automatically.

use std::cmp::Ordering;

use average::Mean;

use crate::stats::aggregate_measurements;

/// Direction of a detected change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDirection {
    /// Performance regression (value increased)
    Increase,
    /// Performance improvement (value decreased)
    Decrease,
}

/// A detected change point in the time series
#[derive(Debug, Clone, PartialEq)]
pub struct ChangePoint {
    /// Position in the time series (0-indexed)
    pub index: usize,
    /// Git commit SHA at this position
    pub commit_sha: String,
    /// Percentage change in mean value
    pub magnitude_pct: f64,
    /// Confidence score [0.0, 1.0]
    pub confidence: f64,
    /// Direction of the change
    pub direction: ChangeDirection,
}

/// Represents an epoch boundary transition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochTransition {
    /// Index in the commit sequence where transition occurs
    pub index: usize,
    /// Epoch number before the transition
    pub from_epoch: u32,
    /// Epoch number after the transition
    pub to_epoch: u32,
}

/// Configuration for change point detection
#[derive(Debug, Clone)]
pub struct ChangePointConfig {
    /// Minimum number of data points required for detection
    pub min_data_points: usize,
    /// Minimum percentage change to consider significant
    pub min_magnitude_pct: f64,
    /// Minimum confidence threshold for reporting
    pub confidence_threshold: f64,
    /// Penalty factor for adding change points (BIC-based)
    pub penalty: f64,
}

impl Default for ChangePointConfig {
    fn default() -> Self {
        Self {
            min_data_points: 10,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.8,
            penalty: 0.5,
        }
    }
}

/// Detect change points in a time series using the PELT algorithm.
///
/// Returns indices where regime shifts are detected.
///
/// # Arguments
/// * `measurements` - Time series data points
/// * `config` - Configuration parameters for detection
///
/// # Returns
/// Vector of indices where change points are detected
pub fn detect_change_points(measurements: &[f64], config: &ChangePointConfig) -> Vec<usize> {
    let n = measurements.len();
    if n < config.min_data_points {
        return vec![];
    }

    // Use BIC-based penalty that scales with data size and variance
    // This prevents over-segmentation in noisy data
    let variance = calculate_variance(measurements);
    // Penalty = base_penalty * log(n) * variance
    // This is a modified BIC criterion that accounts for data variance
    let scaled_penalty = config.penalty * (n as f64).ln() * variance.max(1.0);

    // F[t] = optimal cost for data[0..t]
    // Initialize with -penalty so first segment doesn't double-count
    let mut f = vec![-scaled_penalty; n + 1];
    // cp[t] = last change point before t
    let mut cp = vec![0usize; n + 1];
    // R = candidate set for pruning
    let mut r = vec![0usize];

    for t in 1..=n {
        let (min_cost, best_tau) = r
            .iter()
            .map(|&tau| {
                let cost = f[tau] + segment_cost(measurements, tau, t) + scaled_penalty;
                (cost, tau)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
            .unwrap();

        f[t] = min_cost;
        cp[t] = best_tau;

        // Pruning step: remove candidates that can never be optimal
        r.retain(|&tau| f[tau] + segment_cost(measurements, tau, t) <= min_cost);
        r.push(t);
    }

    // Backtrack to find change points
    let mut result = vec![];
    let mut current = n;
    while cp[current] > 0 {
        result.push(cp[current]);
        current = cp[current];
    }
    result.reverse();
    result
}

/// Calculate the cost of a segment assuming Gaussian distribution.
///
/// Uses sum of squared deviations from the segment mean.
/// Leverages the `average` crate for numerically stable mean calculation.
fn segment_cost(measurements: &[f64], start: usize, end: usize) -> f64 {
    if start >= end {
        return 0.0;
    }

    let segment = &measurements[start..end];
    let mean_calc: Mean = segment.iter().collect();
    let mean = mean_calc.mean();

    segment.iter().map(|x| (x - mean).powi(2)).sum()
}

/// Calculate variance of a dataset.
///
/// Uses the `average` crate's Variance implementation for numerical stability.
fn calculate_variance(measurements: &[f64]) -> f64 {
    if measurements.is_empty() {
        return 0.0;
    }
    let stats = aggregate_measurements(measurements.iter());
    stats.stddev.powi(2) // variance = stddev²
}

/// Convert raw change point indices to enriched ChangePoint structures.
///
/// # Arguments
/// * `indices` - Raw indices from PELT detection
/// * `measurements` - Time series data
/// * `commit_shas` - Git SHAs corresponding to each measurement
/// * `config` - Configuration for filtering
///
/// # Returns
/// Vector of ChangePoint structures with metadata
pub fn enrich_change_points(
    indices: &[usize],
    measurements: &[f64],
    commit_shas: &[String],
    config: &ChangePointConfig,
) -> Vec<ChangePoint> {
    let mut result = vec![];

    for (i, &idx) in indices.iter().enumerate() {
        if idx == 0 || idx >= measurements.len() {
            continue;
        }

        // Calculate mean of the regimen immediately before this change point.
        // This is the segment between the previous change point (or start) and this one.
        // Example: if change points are at indices [10, 20, 30], and we're processing CP at 20:
        //   before_segment = measurements[10..20] (the regimen from previous CP to this CP)
        let before_start = if i > 0 { indices[i - 1] } else { 0 };
        let before_segment = &measurements[before_start..idx];
        let before_mean = if !before_segment.is_empty() {
            let mean_calc: Mean = before_segment.iter().collect();
            mean_calc.mean()
        } else {
            measurements[0]
        };

        // Calculate mean of the regimen immediately after this change point.
        // This is the segment between this change point and the next one (or end).
        // Continuing example: if we're processing CP at 20:
        //   after_segment = measurements[20..30] (the regimen from this CP to next CP)
        let after_end = if i + 1 < indices.len() {
            indices[i + 1]
        } else {
            measurements.len()
        };
        let after_segment = &measurements[idx..after_end];
        let after_mean = if !after_segment.is_empty() {
            let mean_calc: Mean = after_segment.iter().collect();
            mean_calc.mean()
        } else {
            measurements[measurements.len() - 1]
        };

        // Calculate percentage change
        let magnitude_pct = if before_mean.abs() > f64::EPSILON {
            ((after_mean - before_mean) / before_mean) * 100.0
        } else {
            0.0
        };

        // Skip if change is below threshold
        if magnitude_pct.abs() < config.min_magnitude_pct {
            continue;
        }

        // Determine direction
        let direction = if magnitude_pct > 0.0 {
            ChangeDirection::Increase
        } else {
            ChangeDirection::Decrease
        };

        // Calculate confidence based on segment sizes and magnitude
        let confidence = calculate_confidence(idx, measurements.len(), magnitude_pct.abs());

        if confidence < config.confidence_threshold {
            continue;
        }

        let commit_sha = if idx < commit_shas.len() {
            commit_shas[idx].clone()
        } else {
            String::new()
        };

        result.push(ChangePoint {
            index: idx,
            commit_sha,
            magnitude_pct,
            confidence,
            direction,
        });
    }

    result
}

// Confidence calculation constants
/// Minimum segment size threshold for very low confidence (less than this = 0.3 confidence)
const CONFIDENCE_MIN_SEGMENT_VERY_LOW: usize = 3;
/// Minimum segment size threshold for low confidence (less than this = 0.6 confidence)
const CONFIDENCE_MIN_SEGMENT_LOW: usize = 5;
/// Minimum segment size threshold for moderate confidence (less than this = 0.8 confidence)
const CONFIDENCE_MIN_SEGMENT_MODERATE: usize = 10;

/// Confidence value for very small segments (< 3 points on one side)
const CONFIDENCE_FACTOR_VERY_LOW: f64 = 0.3;
/// Confidence value for small segments (3-4 points on one side)
const CONFIDENCE_FACTOR_LOW: f64 = 0.6;
/// Confidence value for moderate segments (5-9 points on one side)
const CONFIDENCE_FACTOR_MODERATE: f64 = 0.8;
/// Confidence value for large segments (10+ points on one side)
const CONFIDENCE_FACTOR_HIGH: f64 = 1.0;

/// Magnitude percentage scale for confidence calculation (50% = max confidence)
const CONFIDENCE_MAGNITUDE_SCALE: f64 = 50.0;

/// Weight for segment size factor in confidence calculation
const CONFIDENCE_WEIGHT_SIZE: f64 = 0.4;
/// Weight for magnitude factor in confidence calculation (more important than size)
const CONFIDENCE_WEIGHT_MAGNITUDE: f64 = 0.6;

/// Calculate confidence score for a change point.
///
/// Based on:
/// - Minimum segment size (at least a few points on each side)
/// - Magnitude of the change (larger = higher confidence)
fn calculate_confidence(index: usize, total_len: usize, magnitude_pct: f64) -> f64 {
    // Minimum segment size factor: ensure at least a few points on each side
    // This is more lenient than balance factor - we just need enough data to be meaningful
    let min_segment = index.min(total_len - index);
    let size_factor = if min_segment < CONFIDENCE_MIN_SEGMENT_VERY_LOW {
        CONFIDENCE_FACTOR_VERY_LOW
    } else if min_segment < CONFIDENCE_MIN_SEGMENT_LOW {
        CONFIDENCE_FACTOR_LOW
    } else if min_segment < CONFIDENCE_MIN_SEGMENT_MODERATE {
        CONFIDENCE_FACTOR_MODERATE
    } else {
        CONFIDENCE_FACTOR_HIGH
    };

    // Magnitude factor: higher magnitude = higher confidence
    // Scale: 10% change = 0.2 confidence, 25% = 0.5, 50% = 1.0
    let magnitude_factor = (magnitude_pct / CONFIDENCE_MAGNITUDE_SCALE).min(1.0);

    // Combine factors: magnitude is more important than segment size
    let confidence =
        CONFIDENCE_WEIGHT_SIZE * size_factor + CONFIDENCE_WEIGHT_MAGNITUDE * magnitude_factor;

    confidence.clamp(0.0, 1.0)
}

/// Detect epoch transitions in a sequence of measurements.
///
/// # Arguments
/// * `epochs` - Vector of epoch numbers for each commit
///
/// # Returns
/// Vector of EpochTransition structures
pub fn detect_epoch_transitions(epochs: &[u32]) -> Vec<EpochTransition> {
    let mut transitions = vec![];

    for i in 1..epochs.len() {
        if epochs[i] != epochs[i - 1] {
            transitions.push(EpochTransition {
                index: i,
                from_epoch: epochs[i - 1],
                to_epoch: epochs[i],
            });
        }
    }

    transitions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_change_point() {
        let data = vec![10.0, 10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0, 20.0];
        let config = ChangePointConfig {
            min_data_points: 5,
            ..Default::default()
        };
        let cps = detect_change_points(&data, &config);
        assert_eq!(cps, vec![5]);
    }

    #[test]
    fn test_no_change_points_stable_data() {
        let data = vec![10.0, 10.1, 9.9, 10.2, 10.0, 10.1, 9.8, 10.3, 10.0, 9.9];
        let config = ChangePointConfig {
            min_data_points: 5,
            penalty: 10.0, // Higher penalty to avoid detecting noise
            ..Default::default()
        };
        let cps = detect_change_points(&data, &config);
        assert!(cps.is_empty());
    }

    #[test]
    fn test_multiple_change_points() {
        let data = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, // First regime
            20.0, 20.0, 20.0, 20.0, 20.0, // Second regime
            30.0, 30.0, 30.0, 30.0, 30.0, // Third regime
        ];
        let config = ChangePointConfig {
            min_data_points: 5,
            penalty: 0.5, // Lower penalty to detect both change points in test data
            ..Default::default()
        };
        let cps = detect_change_points(&data, &config);
        assert_eq!(cps, vec![5, 10]);
    }

    #[test]
    fn test_insufficient_data() {
        let data = vec![10.0, 20.0, 30.0];
        let config = ChangePointConfig::default();
        let cps = detect_change_points(&data, &config);
        assert!(cps.is_empty());
    }

    #[test]
    fn test_segment_cost() {
        let data = vec![10.0, 20.0, 30.0];
        // Mean = 20, deviations: -10, 0, 10
        // Cost = 100 + 0 + 100 = 200
        let cost = segment_cost(&data, 0, 3);
        assert!((cost - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_segment_cost_single_value() {
        let data = vec![10.0];
        let cost = segment_cost(&data, 0, 1);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_enrich_change_points_increase() {
        let measurements = vec![10.0, 10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0, 20.0];
        let commit_shas: Vec<String> = (0..10).map(|i| format!("sha{}", i)).collect();
        let config = ChangePointConfig {
            min_data_points: 5,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.5,
            ..Default::default()
        };

        let indices = vec![5];
        let enriched = enrich_change_points(&indices, &measurements, &commit_shas, &config);

        assert_eq!(enriched.len(), 1);
        assert_eq!(enriched[0].index, 5);
        assert_eq!(enriched[0].commit_sha, "sha5");
        assert!((enriched[0].magnitude_pct - 100.0).abs() < f64::EPSILON);
        assert_eq!(enriched[0].direction, ChangeDirection::Increase);
    }

    #[test]
    fn test_enrich_change_points_decrease() {
        let measurements = vec![20.0, 20.0, 20.0, 20.0, 20.0, 10.0, 10.0, 10.0, 10.0, 10.0];
        let commit_shas: Vec<String> = (0..10).map(|i| format!("sha{}", i)).collect();
        let config = ChangePointConfig {
            min_data_points: 5,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.5,
            ..Default::default()
        };

        let indices = vec![5];
        let enriched = enrich_change_points(&indices, &measurements, &commit_shas, &config);

        assert_eq!(enriched.len(), 1);
        assert_eq!(enriched[0].direction, ChangeDirection::Decrease);
        assert!((enriched[0].magnitude_pct - (-50.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_enrich_filters_small_changes() {
        let measurements = vec![10.0, 10.0, 10.0, 10.0, 10.0, 10.2, 10.2, 10.2, 10.2, 10.2];
        let commit_shas: Vec<String> = (0..10).map(|i| format!("sha{}", i)).collect();
        let config = ChangePointConfig {
            min_data_points: 5,
            min_magnitude_pct: 5.0, // 2% change is below threshold
            confidence_threshold: 0.5,
            ..Default::default()
        };

        let indices = vec![5];
        let enriched = enrich_change_points(&indices, &measurements, &commit_shas, &config);

        assert!(enriched.is_empty());
    }

    #[test]
    fn test_detect_epoch_transitions() {
        let epochs = vec![1, 1, 1, 2, 2, 2, 3, 3];
        let transitions = detect_epoch_transitions(&epochs);

        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].index, 3);
        assert_eq!(transitions[0].from_epoch, 1);
        assert_eq!(transitions[0].to_epoch, 2);
        assert_eq!(transitions[1].index, 6);
        assert_eq!(transitions[1].from_epoch, 2);
        assert_eq!(transitions[1].to_epoch, 3);
    }

    #[test]
    fn test_detect_epoch_transitions_no_changes() {
        let epochs = vec![1, 1, 1, 1];
        let transitions = detect_epoch_transitions(&epochs);
        assert!(transitions.is_empty());
    }

    #[test]
    fn test_calculate_confidence() {
        // Large change with good segment size should have high confidence
        let conf1 = calculate_confidence(50, 100, 50.0);
        assert!(conf1 > 0.9, "conf1 = {}", conf1); // 50% change with 50 points each side

        // Even with fewer points on one side, high magnitude should still be confident
        let conf2 = calculate_confidence(10, 100, 50.0);
        assert!(conf2 > 0.8, "conf2 = {}", conf2); // 10+ points on smaller side

        // Very small segment should have lower confidence
        let conf3 = calculate_confidence(2, 100, 50.0);
        assert!(
            conf3 < conf2,
            "conf3 = {} should be less than conf2 = {}",
            conf3,
            conf2
        );

        // Small magnitude should have lower confidence regardless of balance
        let conf4 = calculate_confidence(50, 100, 5.0);
        assert!(
            conf4 < conf1,
            "conf4 = {} should be less than conf1 = {}",
            conf4,
            conf1
        );
    }

    #[test]
    fn test_full_change_point_detection_workflow() {
        // Simulate a realistic scenario: build times that have two regime shifts
        // Initial regime: ~10s, then regression to ~15s, then improvement to ~12s
        let measurements = vec![
            10.0, 10.2, 9.8, 10.1, 9.9, // Regime 1: ~10s
            15.0, 14.8, 15.2, 15.1, 14.9, // Regime 2: ~15s (50% regression)
            12.0, 11.9, 12.1, 12.0, 11.8, // Regime 3: ~12s (20% improvement)
        ];

        let commit_shas: Vec<String> = (0..15).map(|i| format!("{:040x}", i)).collect();

        let epochs = vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2];

        let config = ChangePointConfig {
            min_data_points: 5,
            min_magnitude_pct: 10.0,
            confidence_threshold: 0.5,
            penalty: 3.0,
        };

        // Detect raw change points
        let raw_cps = detect_change_points(&measurements, &config);
        assert!(!raw_cps.is_empty(), "Should detect change points");

        // Enrich with metadata
        let enriched = enrich_change_points(&raw_cps, &measurements, &commit_shas, &config);

        // Should have detected the major regime shifts
        assert!(
            enriched.iter().any(|cp| cp.magnitude_pct > 0.0),
            "Should detect regression"
        );

        // Detect epoch transitions
        let transitions = detect_epoch_transitions(&epochs);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].index, 10);
        assert_eq!(transitions[0].from_epoch, 1);
        assert_eq!(transitions[0].to_epoch, 2);
    }

    #[test]
    fn test_gradual_drift_not_detected_as_change_point() {
        // Very gradual drift should not be detected as sudden change points
        let measurements: Vec<f64> = (0..20).map(|i| 10.0 + (i as f64 * 0.1)).collect();

        let config = ChangePointConfig {
            min_data_points: 10,
            min_magnitude_pct: 20.0,
            confidence_threshold: 0.8,
            penalty: 10.0, // High penalty to avoid detecting gradual changes
        };

        let cps = detect_change_points(&measurements, &config);

        // With high penalty and strict thresholds, gradual drift shouldn't create change points
        // The actual behavior depends on the penalty tuning
        assert!(
            cps.len() <= 2,
            "Should not detect many change points in gradual drift"
        );
    }

    #[test]
    fn test_change_point_at_boundary() {
        // Test change point detection at the boundary
        // Need enough data points and large enough change to overcome penalty
        let measurements = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0, 20.0, 20.0,
        ];
        let config = ChangePointConfig {
            min_data_points: 10,
            penalty: 1.0, // Lower penalty for small dataset
            ..Default::default()
        };

        let cps = detect_change_points(&measurements, &config);
        assert_eq!(cps, vec![6], "Should detect change point at index 6");
    }

    #[test]
    fn test_enrich_with_empty_sha() {
        let measurements = vec![10.0, 10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0, 20.0];
        let commit_shas: Vec<String> = vec![]; // Empty SHAs
        let config = ChangePointConfig {
            min_data_points: 5,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.5,
            ..Default::default()
        };

        let indices = vec![5];
        let enriched = enrich_change_points(&indices, &measurements, &commit_shas, &config);

        // Should handle empty SHA list gracefully
        assert_eq!(enriched.len(), 1);
        assert_eq!(enriched[0].commit_sha, "");
    }

    #[test]
    fn test_two_distinct_performance_regressions() {
        // Simulates the scenario from the user's chart:
        // ~12ns baseline -> ~17ns (40% regression) -> ~38ns (120% regression from second level)
        let mut measurements = Vec::new();

        // First regime: ~12ns (80 measurements)
        for _ in 0..80 {
            measurements.push(12.0 + rand::random::<f64>() * 0.5 - 0.25);
        }

        // Second regime: ~17ns (80 measurements) - first regression (~40% increase)
        for _ in 0..80 {
            measurements.push(17.0 + rand::random::<f64>() * 0.8 - 0.4);
        }

        // Third regime: ~38ns (80 measurements) - second regression (~120% increase from second)
        for _ in 0..80 {
            measurements.push(38.0 + rand::random::<f64>() * 1.5 - 0.75);
        }

        let config = ChangePointConfig {
            min_data_points: 10,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.7,
            penalty: 0.5, // Default penalty should detect both change points
        };

        let cps = detect_change_points(&measurements, &config);

        // Should detect both change points
        assert!(
            cps.len() >= 2,
            "Expected at least 2 change points, found {}. Change points: {:?}",
            cps.len(),
            cps
        );

        // First change point should be around index 80
        assert!(
            cps[0] > 70 && cps[0] < 90,
            "First change point at {} should be around 80",
            cps[0]
        );

        // Second change point should be around index 160
        assert!(
            cps[1] > 150 && cps[1] < 170,
            "Second change point at {} should be around 160",
            cps[1]
        );
    }

    #[test]
    fn test_penalty_sensitivity_for_multiple_changes() {
        // Test that lower penalty detects more change points
        let data = vec![
            10.0, 10.0, 10.0, 10.0, 10.0, // Regime 1
            15.0, 15.0, 15.0, 15.0, 15.0, // Regime 2 (50% increase)
            20.0, 20.0, 20.0, 20.0, 20.0, // Regime 3 (33% increase)
        ];

        // With default penalty (0.5), should detect both
        let config_low = ChangePointConfig {
            min_data_points: 3,
            penalty: 0.5,
            ..Default::default()
        };
        let cps_low = detect_change_points(&data, &config_low);
        assert_eq!(
            cps_low.len(),
            2,
            "Low penalty should detect 2 change points"
        );

        // With high penalty, might miss the second one
        let config_high = ChangePointConfig {
            min_data_points: 3,
            penalty: 5.0,
            ..Default::default()
        };
        let cps_high = detect_change_points(&data, &config_high);
        assert!(
            cps_high.len() < cps_low.len(),
            "High penalty should detect fewer change points than low penalty"
        );
    }
}
