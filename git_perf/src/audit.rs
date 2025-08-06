use crate::{
    data::MeasurementData,
    measurement_retrieval::{self, summarize_measurements},
    stats,
};
use anyhow::{anyhow, bail, Result};
use cli_types::ReductionFunc;
use itertools::Itertools;
use log::error;
use sparklines::spark;
use std::iter;

#[derive(Debug, PartialEq)]
struct AuditResult {
    message: String,
    passed: bool,
}

pub fn audit_multiple(
    measurements: &[String],
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<()> {
    let mut failed = false;

    for measurement in measurements {
        let result = audit(
            measurement,
            max_count,
            min_count,
            selectors,
            summarize_by,
            sigma,
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

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<AuditResult> {
    let all = measurement_retrieval::walk_commits(max_count)?;

    let filter_by = |m: &MeasurementData| {
        m.name == measurement
            && selectors
                .iter()
                .all(|s| m.key_values.get(&s.0).map(|v| *v == s.1).unwrap_or(false))
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

    let head_summary = stats::aggregate_measurements(iter::once(&head));
    let tail_summary = stats::aggregate_measurements(tail.iter());

    if tail_summary.len < min_count.into() {
        let number_measurements = tail_summary.len;
        let plural_s = if number_measurements > 1 { "s" } else { "" };
        error!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test.");
        return Ok(AuditResult {
            message: format!("Only {number_measurements} measurement{plural_s} found. Less than requested min_measurements of {min_count}. Skipping test."),
            passed: true,
        });
    }

    let direction = if head_summary.mean > tail_summary.mean {
        "↑"
    } else {
        "↓"
    };

    let all_measurements = tail.into_iter().chain(iter::once(head)).collect::<Vec<_>>();
    let average = all_measurements.iter().sum::<f64>() / all_measurements.len() as f64;
    let relative_min = all_measurements
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / average
        - 1.0;
    let relative_max = all_measurements
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / average
        - 1.0;

    let text_summary = format!(
        "z-score: {direction} {:.2}\nHead: {}\nTail: {}\n [{:+.1}% – {:+.1}%] {}",
        head_summary.z_score(&tail_summary),
        &head_summary,
        &tail_summary,
        (relative_min * 100.0),
        (relative_max * 100.0),
        spark(all_measurements.as_slice()),
    );

    if head_summary.z_score(&tail_summary) > sigma {
        return Ok(AuditResult {
            message: format!(
                "Measurement '{measurement}' failed audit.\nHEAD differs significantly from tail measurements.\n{text_summary}"
            ),
            passed: false,
        });
    }

    Ok(AuditResult {
        message: format!("Measurement '{measurement}' passed audit.\n{text_summary}"),
        passed: true,
    })
}
