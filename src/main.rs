use clap::ValueEnum;
use git::{add_note_line_to_head, fetch, raw_push, reconcile, PruneError, PushPullError};
use git2::Repository;
use itertools::{self, Itertools};
use measurements::{Commit, DeserializationError};
use reporting::ReportError;
use serialization::{serialize_single, MeasurementData};
use stats::NumericReductionFunc;
use std::fmt::Display;
use std::io;
use std::iter;
use std::path::Path;
use std::process::{self, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, str};

mod cli;
mod config;
mod git;
mod measurements;
mod reporting;
mod serialization;
mod stats;

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum ReductionFunc {
    Min,
    Max,
    Median,
    Mean,
}

#[derive(Debug)]
enum CliError {
    Add(AddError),
    PushPull(PushPullError),
    Prune(PruneError),
    Report(ReportError),
    Audit(AuditError),
    BumpError(BumpError),
}

#[derive(Debug)]
struct MeasurementSummary {
    epoch: u32,
    val: f64,
}

#[derive(Debug)]
struct CommitSummary {
    commit: String,
    measurement: Option<MeasurementSummary>,
}

// TODO(kaihowl) oh god naming
trait ReductionFuncIterator<'a>: Iterator<Item = &'a MeasurementData> {
    fn reduce_by(&mut self, fun: ReductionFunc) -> Option<MeasurementSummary>;
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

// TODO(kaihowl) use anyhow / thiserror for error propagation
#[derive(Debug)]
enum AddError {
    Git(git2::Error),
}

impl Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::Git(e) => write!(f, "git error, {e}"),
        }
    }
}

impl From<git2::Error> for AddError {
    fn from(e: git2::Error) -> Self {
        AddError::Git(e)
    }
}

#[derive(Debug)]
enum BumpError {}

impl Display for BumpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unspecified bumping error")
    }
}

// TODO(kaihowl) proper error handling
fn bump_epoch(measurement: &str) -> Result<(), BumpError> {
    let mut conf_str = config::read_config().unwrap_or_default();
    config::bump_epoch_in_conf(measurement, &mut conf_str);
    config::write_config(&conf_str);
    Ok(())
}

fn add(measurement: &str, value: f64, key_values: &[(String, String)]) -> Result<(), AddError> {
    // TODO(kaihowl) configure path
    // TODO(kaihowl) configure
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("TODO(kaihowl)");

    let timestamp = timestamp.as_secs_f64();
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();

    let md = MeasurementData {
        // TODO(hoewelmk)
        epoch: config::determine_epoch_from_config(measurement).unwrap_or(0),
        name: measurement.to_owned(),
        timestamp,
        val: value,
        key_values,
    };

    let serialized = serialize_single(&md);

    add_note_line_to_head(&serialized)?;

    Ok(())
}

fn measure(
    measurement: &str,
    repetitions: u16,
    command: &[String],
    key_values: &[(String, String)],
) -> Result<(), AddError> {
    let exe = command.first().unwrap();
    let args = &command[1..];
    for _ in 0..repetitions {
        let mut process = process::Command::new(exe);
        process.args(args);
        let start = Instant::now();
        let output = process
            .output()
            .expect("Command failed to spawn TODO(kaihowl)");
        output
            .status
            .success()
            .then_some(())
            .ok_or("TODO(kaihowl) running error")
            .expect("TODO(kaihowl)");
        let duration = start.elapsed();
        let duration_usec = duration.as_micros() as f64;
        add(measurement, duration_usec, key_values)?;
    }
    Ok(())
}

fn pull(work_dir: Option<&Path>) -> Result<(), PushPullError> {
    fetch(work_dir)?;
    reconcile()
}

fn push(work_dir: Option<&Path>) -> Result<(), PushPullError> {
    let mut retries = 3;

    // TODO(kaihowl) do actual, random backoff
    // TODO(kaihowl) check transient/permanent error
    while retries > 0 {
        match raw_push(work_dir) {
            Ok(_) => return Ok(()),
            Err(_) => {
                retries -= 1;
                pull(work_dir)?;
            }
        }
    }

    Err(PushPullError::RetriesExceeded)
}

impl From<io::Error> for PruneError {
    fn from(_: io::Error) -> Self {
        PruneError::RawGitError
    }
}

impl Display for PruneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PruneError::RawGitError => write!(f, "git error"),
            PruneError::ShallowRepo => write!(f, "shallow repo"),
        }
    }
}

fn prune() -> Result<(), PruneError> {
    git::prune()
}

#[derive(Debug)]
enum AuditError {
    DeserializationError(DeserializationError),
    NoMeasurementForHead,
    SignificantDifference,
}

impl Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::DeserializationError(e) => write!(f, "failed to read, {e}"),
            AuditError::NoMeasurementForHead => write!(f, "no measurement for HEAD"),
            AuditError::SignificantDifference => {
                write!(f, "HEAD differs significantly from tail measurements")
            }
        }
    }
}

impl From<DeserializationError> for AuditError {
    fn from(e: DeserializationError) -> Self {
        AuditError::DeserializationError(e)
    }
}

impl From<git2::Error> for AuditError {
    fn from(e: git2::Error) -> Self {
        AuditError::DeserializationError(DeserializationError::GitError(e))
    }
}

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    summarize_by: ReductionFunc,
    sigma: f64,
) -> Result<(), AuditError> {
    let repo = Repository::open(".")?;
    let all = measurements::walk_commits(&repo, max_count)?;

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
        return Err(AuditError::SignificantDifference);
    }

    Ok(())
}

fn summarize_measurements<'a, F>(
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

#[cfg(test)]
mod test {
    use std::{env::set_current_dir, fs::read_to_string};

    use git2::Signature;
    use httptest::{
        http::{header::AUTHORIZATION, Uri},
        matchers::{self, request},
        responders::status_code,
        Expectation, Server,
    };
    use tempfile::{tempdir, TempDir};

    use crate::*;

    fn init_repo(dir: &Path) -> Repository {
        let repo = git2::Repository::init(dir).expect("Failed to create repo");
        {
            let tree_oid = repo
                .treebuilder(None)
                .expect("Failed to create tree")
                .write()
                .expect("Failed to write tree");
            let tree = &repo
                .find_tree(tree_oid)
                .expect("Could not find written tree");
            let signature = Signature::now("fake", "fake@example.com").expect("No signature");
            repo.commit(
                Some("refs/notes/perf"),
                &signature,
                &signature,
                "Initial commit",
                tree,
                &[],
            )
            .expect("Failed to create first commit");
        }

        repo
    }

    fn dir_with_repo_and_customheader(origin_url: Uri, extra_header: &str) -> TempDir {
        let tempdir = tempdir().unwrap();
        dbg!(&tempdir);
        dbg!(&extra_header);
        dbg!(&origin_url);

        let url = origin_url.to_string();

        let repo = init_repo(tempdir.path());

        repo.remote("origin", &url).expect("Failed to add remote");

        let mut config = repo.config().expect("Failed to get config");
        let config_key = format!("http.{}.extraHeader", url);
        config
            .set_str(&config_key, extra_header)
            .expect("Failed to set config value");

        let stuff = read_to_string(tempdir.path().join(".git/config")).expect("No config");
        eprintln!("config:\n{}", stuff);

        tempdir
    }

    #[test]
    fn test_customheader_push() {
        let test_server = Server::run();
        let repo_dir =
            dir_with_repo_and_customheader(test_server.url(""), "AUTHORIZATION: sometoken");

        test_server.expect(
            Expectation::matching(request::headers(matchers::contains((
                AUTHORIZATION.as_str(),
                "sometoken",
            ))))
            .times(1..)
            .respond_with(status_code(200)),
        );

        // TODO(kaihowl) not so great test as this fails with/without authorization
        // We only want to verify that a call on the server with the authorization header was
        // received.
        pull(Some(repo_dir.path()))
            .expect_err("We have no valid git http server setup -> should fail");
    }

    #[test]
    fn test_customheader_pull() {
        let test_server = Server::run();
        let repo_dir =
            dir_with_repo_and_customheader(test_server.url(""), "AUTHORIZATION: someothertoken");

        set_current_dir(&repo_dir).expect("Failed to change dir");

        test_server.expect(
            Expectation::matching(request::headers(matchers::contains((
                AUTHORIZATION.as_str(),
                "someothertoken",
            ))))
            .times(1..)
            .respond_with(status_code(200)),
        );

        push(Some(repo_dir.path()))
            .expect_err("We have no valid git http sever setup -> should fail");
    }
}
