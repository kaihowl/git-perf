use crate::{
    data::{Commit, CommitSummary, MeasurementData, MeasurementSummary},
    git::git_interop::{self},
    stats::{NumericReductionFunc, ReductionFunc},
};

use anyhow::Result;

pub trait MeasurementReducer<'a>: Iterator<Item = &'a MeasurementData> {
    fn reduce_by(self, fun: ReductionFunc) -> Option<MeasurementSummary>;
}

pub fn summarize_measurements<'a, F>(
    commits: impl Iterator<Item = Result<Commit>> + 'a,
    summarize_by: &'a ReductionFunc,
    filter_by: &'a F,
) -> impl Iterator<Item = Result<CommitSummary>> + 'a
where
    F: Fn(&MeasurementData) -> bool,
{
    commits.map(move |c| {
        c.map(|c| {
            let measurement = c
                .measurements
                .iter()
                .filter(|m| filter_by(m))
                .reduce_by(*summarize_by);

            CommitSummary {
                commit: c.commit,
                measurement,
            }
        })
    })
}

/// Adapter to take results while the epoch is the same as the first one encountered.
pub fn take_while_same_epoch<I>(iter: I) -> impl Iterator<Item = Result<CommitSummary>>
where
    I: Iterator<Item = Result<CommitSummary>>,
{
    let mut first_epoch: Option<u32> = None;
    iter.take_while(move |m| match m {
        Ok(CommitSummary {
            measurement: Some(m),
            ..
        }) => {
            let prev_epoch = first_epoch;
            first_epoch = Some(m.epoch);
            prev_epoch.unwrap_or(m.epoch) == m.epoch
        }
        _ => true,
    })
}

impl<'a, T> MeasurementReducer<'a> for T
where
    T: Iterator<Item = &'a MeasurementData>,
{
    fn reduce_by(self, fun: ReductionFunc) -> Option<MeasurementSummary> {
        let mut peekable = self.peekable();
        let expected_epoch = peekable.peek().map(|m| m.epoch);
        let mut vals = peekable.map(|m| {
            debug_assert_eq!(Some(m.epoch), expected_epoch);
            m.val
        });

        let aggregate_val = vals.aggregate_by(fun);

        Some(MeasurementSummary {
            epoch: expected_epoch?,
            val: aggregate_val?,
        })
    }
}

// Copying all measurements is currently necessary due to deserialization from git notes.
// Optimizing this would require a change in the storage or deserialization model.
pub fn walk_commits_from(
    start_commit: &str,
    num_commits: usize,
) -> Result<impl Iterator<Item = Result<Commit>>> {
    let vec = git_interop::walk_commits_from(start_commit, num_commits)?;
    Ok(vec
        .into_iter()
        .take(num_commits)
        .map(|(commit_id, lines)| -> Result<Commit> {
            let measurements = crate::serialization::deserialize(&lines.join("\n"));
            Ok(Commit {
                commit: commit_id,
                measurements,
            })
        }))
    // When this fails it is due to a shallow clone.
}

pub fn walk_commits(num_commits: usize) -> Result<impl Iterator<Item = Result<Commit>>> {
    walk_commits_from("HEAD", num_commits)
}
