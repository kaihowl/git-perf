use std::fmt::Display;

use crate::{
    data::{CommitSummary, MeasurementData, MeasurementSummary, ReductionFunc},
    stats::NumericReductionFunc,
};

// TODO(kaihowl) oh god naming
trait ReductionFuncIterator<'a>: Iterator<Item = &'a MeasurementData> {
    fn reduce_by(&mut self, fun: ReductionFunc) -> Option<MeasurementSummary>;
}

pub fn summarize_measurements<'a, F>(
    commits: impl Iterator<Item = Result<Commit, DeserializationError>> + 'a,
    summarize_by: &'a ReductionFunc,
    filter_by: &'a F,
) -> impl Iterator<Item = Result<CommitSummary, DeserializationError>> + 'a
where
    F: Fn(&MeasurementData) -> bool,
{
    let measurements = commits.map(move |c| {
        c.map(|c| {
            dbg!(&c.commit);
            let measurement = c
                .measurements
                .iter()
                .filter(|m| filter_by(m))
                .inspect(|m| {
                    dbg!(m);
                })
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
        .inspect(move |m| {
            dbg!(summarize_by);
            dbg!(m);
        })
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
    fn reduce_by(&mut self, fun: ReductionFunc) -> Option<MeasurementSummary> {
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

#[derive(Debug)]
pub enum DeserializationError {
    GitError(git2::Error),
}

impl Display for DeserializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserializationError::GitError(e) => {
                write!(f, "git error (maybe shallow clone not deep enough?), {e}")
            }
        }
    }
}

impl From<git2::Error> for DeserializationError {
    fn from(value: git2::Error) -> Self {
        DeserializationError::GitError(value)
    }
}

// TODO(hoewelmk) copies all measurements, expensive...
pub fn walk_commits(
    repo: &git2::Repository,
    num_commits: usize,
) -> Result<impl Iterator<Item = Result<Commit, DeserializationError>> + '_, DeserializationError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.simplify_first_parent()?;
    Ok(revwalk
        .take(num_commits)
        .map(|commit_oid| -> Result<Commit, DeserializationError> {
            let commit_id = commit_oid?;
            let measurements = match repo.find_note(Some("refs/notes/perf"), commit_id) {
                // TODO(kaihowl) remove unwrap_or
                Ok(note) => crate::serialization::deserialize(note.message().unwrap_or("")),
                Err(_) => [].into(),
            };
            Ok(Commit {
                commit: commit_id.to_string(),
                measurements,
            })
        }))
    // When this fails it is due to a shallow clone.
    // TODO(kaihowl) proper shallow clone support
    // https://github.com/libgit2/libgit2/issues/3058 tracks that we fail to revwalk the
    // last commit because the parent cannot be loooked up.
}
