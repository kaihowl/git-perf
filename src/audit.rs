use std::{fmt::Display, iter};

use git2::Repository;
use itertools::Itertools;

use crate::{
    measurement_retrieval::{self, summarize_measurements, DeserializationError, ReductionFunc},
    serialization::MeasurementData,
    stats,
};

#[derive(Debug)]
pub enum AuditError {
    DeserializationError(DeserializationError),
    NoMeasurementForHead,
    SignificantDifference,
}

impl Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::DeserializationError(e) => write!(f, "failed to read, {e}"),
            AuditError::NoMeasurementForHead => write!(f, "no measurement for HEAD"),
            AuditError::SignificantDifference => {
                write!(f, "HEAD differs significantly from tail measurements")
            }
        }
    }
}

impl From<DeserializationError> for AuditError {
    fn from(e: DeserializationError) -> Self {
        AuditError::DeserializationError(e)
    }
}

impl From<git2::Error> for AuditError {
    fn from(e: git2::Error) -> Self {
        AuditError::DeserializationError(DeserializationError::GitError(e))
    }
}

pub fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<(), AuditError> {
    let repo = Repository::open(".")?;
    let all = measurement_retrieval::walk_commits(&repo, max_count)?;

    let filter_by = |m: &MeasurementData| {
        m.name == measurement
            && selectors
                .iter()
                .all(|s| m.key_values.get(&s.0).map(|v| *v == s.1).unwrap_or(false))
    };

    let mut aggregates = summarize_measurements(all, &summarize_by, &filter_by);

    let head = aggregates
        .next()
        .ok_or(AuditError::NoMeasurementForHead)
        .and_then(|s| {
            eprintln!("Head measurement is: {s:?}");
            match s {
                Ok(cs) => match cs.measurement {
                    Some(m) => Ok(m.val),
                    _ => Err(AuditError::NoMeasurementForHead),
                },
                // TODO(kaihowl) more specific error?
                _ => Err(AuditError::NoMeasurementForHead),
            }
        })?;

    let tail: Vec<_> = aggregates
        .filter_map_ok(|cs| cs.measurement.map(|m| m.val))
        .take(max_count)
        .try_collect()?;

    let head_summary = stats::aggregate_measurements(iter::once(head));
    let tail_summary = stats::aggregate_measurements(tail.into_iter());

    dbg!(&head_summary);
    dbg!(&tail_summary);
    if tail_summary.len < min_count.into() {
        // TODO(kaihowl) handle with explicit return? Print text somewhere else?
        let number_measurements = tail_summary.len;
        let plural_s = if number_measurements > 1 { "s" } else { "" };
        eprintln!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");
        return Ok(());
    }

    if head_summary.significantly_different_from(&tail_summary, sigma) {
        eprintln!("Measurements differ significantly");
        // TODO(kaihowl) print details
        return Err(AuditError::SignificantDifference);
    }

    Ok(())
}
