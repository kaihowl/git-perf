use crate::{
    config,
    data::MeasurementData,
    measurement_retrieval::{self, summarize_measurements},
    stats::{self, VecAggregation},
};
use anyhow::{anyhow, bail, Result};
use git_perf_cli_types::ReductionFunc;
use itertools::Itertools;
use log::error;
use sparklines::spark;
use std::cmp::Ordering;
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

    let direction = match head_summary.mean.partial_cmp(&tail_summary.mean).unwrap() {
        Ordering::Greater => "↑",
        Ordering::Less => "↓",
        Ordering::Equal => "→",
    };

    let mut tail_measurements = tail.clone();
    let tail_median = tail_measurements.median().unwrap_or(0.0);

    let all_measurements = tail.into_iter().chain(iter::once(head)).collect::<Vec<_>>();
    let relative_min = all_measurements
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / tail_median
        - 1.0;
    let relative_max = all_measurements
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        / tail_median
        - 1.0;

    // Calculate relative deviation of HEAD measurement using tail median
    let head_relative_deviation = (head / tail_median - 1.0).abs() * 100.0;

    // Check if we have a minimum relative deviation threshold configured
    let min_relative_deviation = config::audit_min_relative_deviation(measurement);
    let threshold_applied = min_relative_deviation.is_some();
    let passed_due_to_threshold = min_relative_deviation
        .map(|threshold| head_relative_deviation < threshold)
        .unwrap_or(false);

    let text_summary = format!(
        "z-score: {direction} {:.2}\nHead: {}\nTail: {}\n [{:+.1}% – {:+.1}%] {}",
        head_summary.z_score(&tail_summary),
        &head_summary,
        &tail_summary,
        (relative_min * 100.0),
        (relative_max * 100.0),
        spark(all_measurements.as_slice()),
    );

    // Check if HEAD measurement exceeds sigma threshold
    let z_score_exceeds_sigma = head_summary.z_score(&tail_summary) > sigma;

    // Determine if audit passes
    let passed = !z_score_exceeds_sigma || passed_due_to_threshold;

    // Add threshold information to output if applicable
    let threshold_note = if threshold_applied && passed_due_to_threshold {
        format!(
            "\nNote: Passed due to relative deviation ({:.1}%) being below threshold ({:.1}%)",
            head_relative_deviation,
            min_relative_deviation.unwrap()
        )
    } else {
        String::new()
    };

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
