use std::fmt::Display;

use crate::serialization::MeasurementData;

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
