use std::fmt::Display;

use average::{self, concatenate, Estimate, Mean, Variance};
use itertools::Itertools;

use cli_types::ReductionFunc;

use readable::num::*;

pub trait VecAggregation {
    fn median(&mut self) -> Option<f64>;
}

concatenate!(AggStats, [Mean, mean], [Variance, sample_variance]);

pub fn aggregate_measurements(measurements: impl Iterator<Item = f64>) -> Stats {
    let s: AggStats = measurements.collect();
    Stats {
        mean: s.mean(),
        stddev: s.sample_variance().sqrt(),
        len: s.mean.len() as usize,
    }
}

#[derive(Debug)]
pub struct Stats {
    pub mean: f64,
    pub stddev: f64,
    pub len: usize,
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "μ: {} σ: {} n: {}",
            Float::from(self.mean),
            Float::from(self.stddev),
            Unsigned::from(self.len),
        )
    }
}

impl Stats {
    pub fn significantly_different_from(&self, other: &Stats, sigma: f64) -> bool {
        assert!(self.len == 1);
        assert!(other.len >= 1);
        (self.mean - other.mean).abs() / other.stddev > sigma
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
    use super::*;

    #[test]
    fn no_floating_error() {
        let measurements = (0..100).map(|_| 0.1).collect_vec();
        let stats = aggregate_measurements(measurements.into_iter());
        assert_eq!(stats.mean, 0.1);
        assert_eq!(stats.len, 100);
        let naive_mean = (0..100).map(|_| 0.1).sum::<f64>() / 100.0;
        assert_ne!(naive_mean, 0.1);
    }

    #[test]
    fn single_measurement() {
        let measurements = vec![1.0];
        let stats = aggregate_measurements(measurements.into_iter());
        assert_eq!(stats.len, 1);
        assert_eq!(stats.mean, 1.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn no_measurement() {
        let measurements = vec![];
        let stats = aggregate_measurements(measurements.into_iter());
        assert_eq!(stats.len, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn z_score_with_zero_stddev() {
        let stddev = 0.0;
        let mean = 30.0;
        let higher_val = 50.0;
        let lower_val = 10.0;
        let z_high = ((higher_val - mean) / stddev as f64).abs();
        let z_low = ((lower_val - mean) / stddev as f64).abs();
        assert_eq!(z_high, f64::INFINITY);
        assert_eq!(z_low, f64::INFINITY);
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
}
