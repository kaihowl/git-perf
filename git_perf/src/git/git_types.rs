use std::io;

use super::git_definitions::GIT_PERF_REMOTE;

#[derive(Debug)]
pub(super) struct GitOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum GitError {
    #[error("A ref failed to be pushed:\n{0}\n{1}", output.stdout, output.stderr)]
    RefFailedToPush { output: GitOutput },

    #[error("Missing HEAD for {reference}")]
    MissingHead { reference: String },

    #[error("A ref failed to be locked:\n{0}\n{1}", output.stdout, output.stderr)]
    RefFailedToLock { output: GitOutput },

    #[error("Shallow repository. Refusing operation.")]
    ShallowRepository,

    #[error("This repo does not have any measurements.")]
    MissingMeasurements,

    #[error("A concurrent change to the ref occurred:\n{0}\n{1}", output.stdout, output.stderr)]
    RefConcurrentModification { output: GitOutput },

    #[error("Git failed to execute.\n\nstdout:\n{0}\nstderr:\n{1}", output.stdout, output.stderr)]
    ExecError { command: String, output: GitOutput },

    #[error("Remote repository is empty or has never been pushed to. Please push some measurements first.\n{0}\n{1}", output.stdout, output.stderr)]
    NoRemoteMeasurements { output: GitOutput },

    #[error("No upstream found. Consider setting origin or {}.", GIT_PERF_REMOTE)]
    NoUpstream {},

    #[error("Failed to execute git command")]
    IoError(#[from] io::Error),
}

#[derive(Debug, PartialEq)]
pub(super) struct Reference {
    pub refname: String,
    pub oid: String,
}
