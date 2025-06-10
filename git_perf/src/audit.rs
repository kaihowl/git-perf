use crate::{
    data::MeasurementData,
    measurement_retrieval::{self, summarize_measurements},
    stats,
};
use anyhow::{anyhow, bail, Result};
use cli_types::ReductionFunc;
use itertools::Itertools;
use log::error;
use std::iter;

pub fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<()> {
    let all = measurement_retrieval::walk_commits(max_count)?;

    let filter_by = |m: &MeasurementData| {
        m.name == measurement
            && selectors
                .iter()
                .all(|s| m.key_values.get(&s.0).map(|v| *v == s.1).unwrap_or(false))
    };

    let mut aggregates = summarize_measurements(all, &summarize_by, &filter_by);

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

    let head_summary = stats::aggregate_measurements(iter::once(head));
    let tail_summary = stats::aggregate_measurements(tail.into_iter());

    if tail_summary.len < min_count.into() {
        let number_measurements = tail_summary.len;
        let plural_s = if number_measurements > 1 { "s" } else { "" };
        error!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");
        return Ok(());
    }

    if head_summary.significantly_different_from(&tail_summary, sigma) {
        bail!(
            "HEAD differs significantly from tail measurements.\nHead: {}\nTail: {}",
            &head_summary,
            &tail_summary
        );
    }

    Ok(())
}
