use crate::{
    data::{CommitSummary, MeasurementData, MeasurementSummary, ReductionFunc},
    git_interop::{self},
    stats::NumericReductionFunc,
};

use anyhow::Result;

// TODO(kaihowl) oh god naming
pub trait ReductionFuncIterator<'a>: Iterator<Item = &'a MeasurementData> {
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
    let measurements = commits.map(move |c| {
        c.map(|c| {
            // dbg!(&c.commit);
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
    });

    let mut first_epoch = None;

    // TODO(kaihowl) this is a second repsonsibility, move out? "EpochClearing"
    measurements
        // .inspect(move |m| {
        // dbg!(summarize_by);
        // dbg!(m);
        // })
        .take_while(move |m| match &m {
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

impl<'a, T> ReductionFuncIterator<'a> for T
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

#[derive(Debug, PartialEq)]
pub struct Commit {
    pub commit: String,
    pub measurements: Vec<MeasurementData>,
}

// TODO(hoewelmk) copies all measurements, expensive...
// TODO(kaihowl) missing check for shallow clone marker!
pub fn walk_commits(num_commits: usize) -> Result<impl Iterator<Item = Result<Commit>>> {
    let vec = git_interop::walk_commits(num_commits)?;
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
    // TODO(kaihowl) proper shallow clone support
    // https://github.com/libgit2/libgit2/issues/3058 tracks that we fail to revwalk the
    // last commit because the parent cannot be loooked up.
}
