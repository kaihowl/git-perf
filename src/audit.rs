use crate::{
    data::{MeasurementData, ReductionFunc},
    measurement_retrieval::{self, summarize_measurements},
    stats,
};
use anyhow::Result;
use git2::Repository;
use itertools::Itertools;
use std::iter;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("no measurement for HEAD")]
    NoMeasurementForHead,

    #[error("HEAD differs significantly from tail measurements")]
    SignificantDifference,
}

pub fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<()> {
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
        return Err(AuditError::SignificantDifference.into());
    }

    Ok(())
}
