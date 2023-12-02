use audit::AuditError;
use config::BumpError;
use git::{PruneError, PushPullError};

use measurement_storage::AddError;
use reporting::ReportError;

use std::fmt::Display;
use std::process::ExitCode;

mod audit;
mod cli;
mod config;
mod git;
mod measurement_retrieval;
mod measurement_storage;
mod reporting;
mod serialization;
mod stats;

#[derive(Debug)]
enum CliError {
    Add(AddError),
    PushPull(PushPullError),
    Prune(PruneError),
    Report(ReportError),
    Audit(AuditError),
    BumpError(BumpError),
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Add(e) => write!(f, "During add: {e}"),
            CliError::PushPull(e) => write!(f, "During push/pull: {e}"),
            CliError::Report(e) => write!(f, "During report: {e}"),
            CliError::Audit(e) => write!(f, "During audit: {e}"),
            CliError::Prune(e) => write!(f, "During prune: {e}"),
            CliError::BumpError(e) => write!(f, "During bumping of epoch: {e}"),
        }
    }
}

impl From<AddError> for CliError {
    fn from(e: AddError) -> Self {
        CliError::Add(e)
    }
}

impl From<ReportError> for CliError {
    fn from(e: ReportError) -> Self {
        CliError::Report(e)
    }
}

impl From<AuditError> for CliError {
    fn from(e: AuditError) -> Self {
        CliError::Audit(e)
    }
}

impl From<PushPullError> for CliError {
    fn from(e: PushPullError) -> Self {
        CliError::PushPull(e)
    }
}

impl From<PruneError> for CliError {
    fn from(e: PruneError) -> Self {
        CliError::Prune(e)
    }
}

impl From<BumpError> for CliError {
    fn from(e: BumpError) -> Self {
        CliError::BumpError(e)
    }
}

fn main() -> ExitCode {
    match cli::handle_calls() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Failed: {e}");
            ExitCode::FAILURE
        }
    }
}
