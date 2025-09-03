use std::fmt::Display;

use average::{self, concatenate, Estimate, Mean, Variance};
use itertools::Itertools;

use readable::num::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReductionFunc {
    Min,
    Max,
    Median,
    Mean,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DispersionMethod {
    StandardDeviation,
    MedianAbsoluteDeviation,
}

// Conversion from CLI types to stats types
impl From<git_perf_cli_types::ReductionFunc> for ReductionFunc {
    fn from(func: git_perf_cli_types::ReductionFunc) -> Self {
        match func {
            git_perf_cli_types::ReductionFunc::Min => ReductionFunc::Min,
            git_perf_cli_types::ReductionFunc::Max => ReductionFunc::Max,
            git_perf_cli_types::ReductionFunc::Median => ReductionFunc::Median,
            git_perf_cli_types::ReductionFunc::Mean => ReductionFunc::Mean,
        }
    }
}

impl From<git_perf_cli_types::DispersionMethod> for DispersionMethod {
    fn from(method: git_perf_cli_types::DispersionMethod) -> Self {
        match method {
            git_perf_cli_types::DispersionMethod::StandardDeviation => {
                DispersionMethod::StandardDeviation
            }
            git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation => {
                DispersionMethod::MedianAbsoluteDeviation
            }
        }
    }
}

pub trait VecAggregation {
    fn median(&mut self) -> Option<f64>;
}

concatenate!(AggStats, [Mean, mean], [Variance, sample_variance]);

pub fn aggregate_measurements<'a>(measurements: impl Iterator<Item = &'a f64>) -> Stats {
    let measurements_vec: Vec<f64> = measurements.cloned().collect();
    let s: AggStats = measurements_vec.iter().collect();
    Stats {
        mean: s.mean(),
        stddev: s.sample_variance().sqrt(),
        mad: calculate_mad(&measurements_vec),
        len: s.mean.len() as usize,
    }
}

pub fn calculate_mad(measurements: &[f64]) -> f64 {
    if measurements.is_empty() {
        return 0.0;
    }

    // Calculate median without modifying original data
    let mut measurements_copy = measurements.to_vec();
    let median = measurements_copy.median().unwrap();

    // Calculate absolute deviations
    let mut abs_deviations: Vec<f64> = measurements.iter().map(|&x| (x - median).abs()).collect();

    // Calculate median of absolute deviations
    abs_deviations.median().unwrap()
}

#[derive(Debug)]
pub struct Stats {
    pub mean: f64,
    pub stddev: f64,
    pub mad: f64,
    pub len: usize,
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "μ: {} σ: {} MAD: {} n: {}",
            Float::from(self.mean),
            Float::from(self.stddev),
            Float::from(self.mad),
            Unsigned::from(self.len),
        )
    }
}

impl Stats {
    pub fn z_score(&self, other: &Stats) -> f64 {
        self.z_score_with_method(other, DispersionMethod::StandardDeviation)
    }

    pub fn z_score_with_method(&self, other: &Stats, method: DispersionMethod) -> f64 {
        assert!(self.len == 1);
        assert!(other.len >= 1);

        let dispersion = match method {
            DispersionMethod::StandardDeviation => other.stddev,
            DispersionMethod::MedianAbsoluteDeviation => other.mad,
        };

        // Division by zero is an expected case here: For measurements with no variance
        (self.mean - other.mean).abs() / dispersion
    }
}

impl VecAggregation for Vec<f64> {
    fn median(&mut self) -> Option<f64> {
        self.sort_by(f64::total_cmp);
        match self.len() {
            0 => None,
            even if even % 2 == 0 => {
                let left = self[even / 2 - 1];
                let right = self[even / 2];
                Some((left + right) / 2.0)
            }
            odd => Some(self[odd / 2]),
        }
    }
}

pub trait NumericReductionFunc: Iterator<Item = f64> {
    fn aggregate_by(&mut self, fun: ReductionFunc) -> Option<Self::Item> {
        match fun {
            ReductionFunc::Min => self.reduce(f64::min),
            ReductionFunc::Max => self.reduce(f64::max),
            ReductionFunc::Median => self.collect_vec().median(),
            ReductionFunc::Mean => {
                let stats: AggStats = self.collect();
                if stats.mean.is_empty() {
                    None
                } else {
                    Some(stats.mean())
                }
            }
        }
    }
}

impl<T> NumericReductionFunc for T where T: Iterator<Item = f64> {}

#[cfg(test)]
mod test {
    use average::assert_almost_eq;

    use super::*;

    #[test]
    fn no_floating_error() {
        let measurements = (0..100).map(|_| 0.1).collect_vec();
        let stats = aggregate_measurements(measurements.iter());
        assert_eq!(stats.mean, 0.1);
        assert_eq!(stats.len, 100);
        let naive_mean = (0..100).map(|_| 0.1).sum::<f64>() / 100.0;
        assert_ne!(naive_mean, 0.1);
    }

    #[test]
    fn single_measurement() {
        let measurements = vec![1.0];
        let stats = aggregate_measurements(measurements.iter());
        assert_eq!(stats.len, 1);
        assert_eq!(stats.mean, 1.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn no_measurement() {
        let measurements = vec![];
        let stats = aggregate_measurements(measurements.iter());
        assert_eq!(stats.len, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn z_score_with_zero_stddev() {
        let tail = Stats {
            mean: 30.0,
            stddev: 0.0,
            mad: 0.0,
            len: 40,
        };

        let head_normal = Stats {
            mean: 30.0,
            stddev: 0.0,
            mad: 0.0,
            len: 1,
        };

        let head_low = Stats {
            mean: 20.0,
            stddev: 0.0,
            mad: 0.0,
            len: 1,
        };

        let z_normal = head_normal.z_score(&tail);
        assert!(z_normal.is_nan());

        let z_low = head_low.z_score(&tail);
        assert!(z_low.is_infinite());
    }

    #[test]
    fn verify_stats() {
        let empty_vec = [];
        assert_eq!(None, empty_vec.into_iter().aggregate_by(ReductionFunc::Min));
        assert_eq!(None, empty_vec.into_iter().aggregate_by(ReductionFunc::Max));
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(ReductionFunc::Median)
        );
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(ReductionFunc::Mean)
        );

        let single_el_vec = [3.0];
        assert_eq!(
            Some(3.0),
            single_el_vec.into_iter().aggregate_by(ReductionFunc::Min)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec.into_iter().aggregate_by(ReductionFunc::Max)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec
                .into_iter()
                .aggregate_by(ReductionFunc::Median)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec.into_iter().aggregate_by(ReductionFunc::Mean)
        );

        let two_el_vec = [3.0, 1.0];
        assert_eq!(
            Some(1.0),
            two_el_vec.into_iter().aggregate_by(ReductionFunc::Min)
        );
        assert_eq!(
            Some(3.0),
            two_el_vec.into_iter().aggregate_by(ReductionFunc::Max)
        );
        assert_eq!(
            Some(2.0),
            two_el_vec.into_iter().aggregate_by(ReductionFunc::Median)
        );
        assert_eq!(
            Some(2.0),
            two_el_vec.into_iter().aggregate_by(ReductionFunc::Mean)
        );

        let three_el_vec = [2.0, 6.0, 1.0];
        assert_eq!(
            Some(1.0),
            three_el_vec.into_iter().aggregate_by(ReductionFunc::Min)
        );
        assert_eq!(
            Some(6.0),
            three_el_vec.into_iter().aggregate_by(ReductionFunc::Max)
        );
        assert_eq!(
            Some(2.0),
            three_el_vec.into_iter().aggregate_by(ReductionFunc::Median)
        );
        assert_eq!(
            Some(3.0),
            three_el_vec.into_iter().aggregate_by(ReductionFunc::Mean)
        );
    }

    #[test]
    fn test_calculate_mad() {
        // Test empty array
        assert_eq!(calculate_mad(&[]), 0.0);

        // Test single value
        assert_eq!(calculate_mad(&[5.0]), 0.0);

        // Test two values
        assert_eq!(calculate_mad(&[1.0, 3.0]), 1.0);

        // Test three values
        assert_eq!(calculate_mad(&[1.0, 2.0, 3.0]), 1.0);

        // Test with outliers
        let data = [1.0, 2.0, 3.0, 100.0];
        let mad = calculate_mad(&data);
        assert_almost_eq!(mad, 1.0, 0.001);
        // assert!(mad > 0.0);
        // assert!(mad < 50.0); // Should be robust to outliers

        // Test with known MAD value
        let data = [1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0];
        let mad = calculate_mad(&data);
        assert_almost_eq!(mad, 1.0, 0.001);
    }

    #[test]
    fn test_mad_in_aggregate_measurements() {
        let measurements = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = aggregate_measurements(measurements.iter());

        assert_eq!(stats.len, 5);
        assert_eq!(stats.mean, 3.0);
        assert!(stats.mad > 0.0);
        assert!(stats.stddev > 0.0);

        // MAD should be less than stddev for normal distributions
        assert!(stats.mad < stats.stddev);
    }

    #[test]
    fn test_z_score_with_mad() {
        let tail = Stats {
            mean: 30.0,
            stddev: 5.0,
            mad: 3.0,
            len: 40,
        };

        let head = Stats {
            mean: 35.0,
            stddev: 0.0,
            mad: 0.0,
            len: 1,
        };

        let z_score_stddev = head.z_score_with_method(&tail, DispersionMethod::StandardDeviation);
        let z_score_mad =
            head.z_score_with_method(&tail, DispersionMethod::MedianAbsoluteDeviation);

        assert_eq!(z_score_stddev, 1.0); // (35-30)/5 = 1.0
        assert_eq!(z_score_mad, 5.0 / 3.0); // (35-30)/3 ≈ 1.67

        // MAD z-score should be different from stddev z-score
        assert_ne!(z_score_stddev, z_score_mad);
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that existing z_score method still works
        let tail = Stats {
            mean: 30.0,
            stddev: 5.0,
            mad: 3.0,
            len: 40,
        };

        let head = Stats {
            mean: 35.0,
            stddev: 0.0,
            mad: 0.0,
            len: 1,
        };

        let z_score_old = head.z_score(&tail);
        let z_score_new = head.z_score_with_method(&tail, DispersionMethod::StandardDeviation);

        assert_eq!(z_score_old, z_score_new);
    }

    #[test]
    fn test_display_with_mad() {
        let stats = Stats {
            mean: 10.0,
            stddev: 2.0,
            mad: 1.5,
            len: 5,
        };

        let display = format!("{}", stats);
        assert!(display.contains("μ: 10"));
        assert!(display.contains("σ: 2"));
        assert!(display.contains("MAD: 1.5"));
        assert!(display.contains("n: 5"));
    }
}
