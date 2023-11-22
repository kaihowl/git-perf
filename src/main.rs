use clap::ValueEnum;
use git::{add_note_line_to_head, fetch, raw_push, reconcile, PruneError, PushPullError};
use git2::Repository;
use itertools::{self, Itertools};
use plotly::{
    common::{Font, LegendGroupTitle, Title},
    layout::Axis,
    BoxPlot, Layout, Plot,
};
use serde::Serialize;
use serialization::{deserialize, serialize_single, MeasurementData};
use stats::NumericReductionFunc;
use std::fmt::Display;
use std::io::{self, Write};
use std::iter;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, fs::File, str};

mod cli;
mod config;
mod git;
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
    let all = walk_commits(&repo, max_count)?;

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

// TODO(kaihowl) make all of these pretty printed for `main`
#[derive(Debug)]
enum ReportError {
    DeserializationError(DeserializationError),
    InvalidSeparateBy,
    // Report would not contain any measurements
    NoMeasurements,
    InvalidOutputFormat,
}

impl Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::DeserializationError(e) => write!(f, "failed to read, {e}"),
            ReportError::InvalidSeparateBy => write!(f, "invalid separator supplied"),
            ReportError::NoMeasurements => write!(f, "no performance measurements found"),
            ReportError::InvalidOutputFormat => write!(f, "could not infer output format"),
        }
    }
}

impl From<DeserializationError> for ReportError {
    fn from(e: DeserializationError) -> Self {
        ReportError::DeserializationError(e)
    }
}

trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);
    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        group_value: Option<&String>,
    );
    fn as_bytes(&self) -> Vec<u8>;
}

struct PlotlyReporter {
    plot: Plot,
}

impl PlotlyReporter {
    fn new() -> PlotlyReporter {
        PlotlyReporter { plot: Plot::new() }
    }
}

impl<'a> Reporter<'a> for PlotlyReporter {
    fn add_commits(&mut self, commits: &'a [Commit]) {
        let enumerated_commits = commits.iter().enumerate();

        let (commit_nrs, short_hashes): (Vec<_>, Vec<_>) = enumerated_commits
            .map(|(n, c)| (n as f64, c.commit[..6].to_owned()))
            .unzip();
        let x_axis = Axis::new()
            .tick_values(commit_nrs)
            .tick_text(short_hashes)
            .tick_font(Font::new().family("monospace"));
        let layout = Layout::new()
            .title(Title::new("Something, something"))
            .x_axis(x_axis);
        self.plot.set_layout(layout);
    }

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        group_value: Option<&String>,
    ) {
        let mut measurement_name = None;
        let (x, y): (Vec<usize>, Vec<f64>) = indexed_measurements
            .into_iter()
            .inspect(|(_, md)| {
                // TODO(kaihowl)
                assert!(true);
                measurement_name = Some(&md.name);
            })
            .map(|(i, m)| (i, m.val))
            .unzip();

        let measurement_name = measurement_name.expect("No measurements supplied for trace");
        let trace = if let Some(group_value) = group_value {
            BoxPlot::new_xy(x, y)
                .name(group_value)
                .legend_group(measurement_name)
                .legend_group_title(LegendGroupTitle::new(measurement_name))
        } else {
            BoxPlot::new_xy(x, y).name(measurement_name)
        };
        self.plot.add_trace(trace);
    }

    fn as_bytes(&self) -> Vec<u8> {
        self.plot.to_html().as_bytes().to_vec()
    }
}

struct CsvReporter<'a> {
    hashes: Vec<String>,
    indexed_measurements: Vec<(usize, &'a MeasurementData)>,
}

impl CsvReporter<'_> {
    fn new() -> Self {
        CsvReporter {
            hashes: Vec::new(),
            indexed_measurements: Vec::new(),
        }
    }
}

struct ReporterFactory {}

impl ReporterFactory {
    fn from_file_name<'a, 'b: 'a>(path: &'b Path) -> Option<Box<dyn Reporter + 'a>> {
        if path == Path::new("-") {
            return Some(Box::new(CsvReporter::new()) as Box<dyn Reporter + 'a>);
        }
        let mut res = None;
        if let Some(ext) = path.extension() {
            let extension = ext.to_ascii_lowercase().into_string().unwrap();
            res = match extension.as_str() {
                "html" => Some(Box::new(PlotlyReporter::new()) as Box<dyn Reporter>),
                "csv" => Some(Box::new(CsvReporter::new()) as Box<dyn Reporter + 'a>),
                _ => None,
            }
        }
        res
    }
}

#[derive(Serialize)]
struct HashAndMeasurement<'a> {
    commit: &'a str,
    measurement: &'a MeasurementData,
}

impl<'a> Reporter<'a> for CsvReporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]) {
        self.hashes = hashes.iter().map(|c| c.commit.to_owned()).collect();
    }

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        _group_value: Option<&String>,
    ) {
        self.indexed_measurements
            .extend_from_slice(indexed_measurements.as_slice());
    }

    fn as_bytes(&self) -> Vec<u8> {
        // TODO(kaihowl) write to path directly instead?
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_writer(vec![]);

        for (index, measurement_data) in &self.indexed_measurements {
            writer
                .serialize(HashAndMeasurement {
                    commit: &self.hashes[*index],
                    measurement: measurement_data,
                })
                .expect("inner serialization error TODO(kaihowl)")
        }
        writer
            .into_inner()
            .expect("serialization error TODO(kaihowl)")
    }
}

impl From<git2::Error> for ReportError {
    fn from(value: git2::Error) -> Self {
        ReportError::DeserializationError(DeserializationError::GitError(value))
    }
}

fn walk_measurements<'a>(
    repo: &'a Repository,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
) -> Result<
    impl Iterator<
            Item = Result<(String, impl Iterator<Item = MeasurementData>), DeserializationError>,
        > + 'a,
    DeserializationError,
> {
    let commits = walk_commits(repo, num_commits)?;
    // TODO(kaihowl) ok to remove not ok result?
    Ok(commits
        .into_iter()
        .map_ok(|c| (c.commit, c.measurements.into_iter())))
}

// TODO(kaihowl) needs more fine grained output e2e tests
fn report(
    output: PathBuf,
    separate_by: Option<String>,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
) -> Result<(), ReportError> {
    let repo = Repository::open(".")?;
    let commits: Vec<Commit> = walk_commits(&repo, num_commits)?.try_collect()?;

    let mut plot =
        ReporterFactory::from_file_name(&output).ok_or(ReportError::InvalidOutputFormat)?;

    plot.add_commits(&commits);

    // TODO(kaihowl) duplication with audit
    // new function to filter measurements with relevant data
    // get commits from walk_commits with walk_measurements instead -> Iter<Item=(Commit,
    // MeasurementData)>
    // fn filter(Iter<Commit>
    let relevant = |m: &MeasurementData| {
        if !measurement_names.is_empty() && !measurement_names.contains(&m.name) {
            return false;
        }
        // TODO(kaihowl) express this and the audit-fn equivalent as subset relations
        key_values
            .iter()
            .all(|(k, v)| m.key_values.get(k).map(|mv| v == mv).unwrap_or(false))
    };

    let indexed_measurements = commits.iter().enumerate().flat_map(|(index, commit)| {
        commit
            .measurements
            .iter()
            .map(move |m| (index, m))
            .filter(|(_, m)| relevant(m))
    });

    let unique_measurement_names: Vec<_> = indexed_measurements
        .clone()
        .map(|(_, m)| &m.name)
        .unique()
        .collect();

    if unique_measurement_names.is_empty() {
        return Err(ReportError::NoMeasurements);
    }

    for measurement_name in unique_measurement_names {
        let filtered_measurements = indexed_measurements
            .clone()
            .filter(|(_i, m)| m.name == *measurement_name);

        let group_values = if let Some(separate_by) = &separate_by {
            filtered_measurements
                .clone()
                .flat_map(|(_, m)| {
                    m.key_values
                        .iter()
                        .filter(|kv| kv.0 == separate_by)
                        .map(|kv| kv.1)
                })
                .unique()
                .map(|val| (Some(separate_by), Some(val)))
                .collect_vec()
        } else {
            vec![(None, None)]
        };

        if group_values.is_empty() {
            return Err(ReportError::InvalidSeparateBy);
        }

        for (group_key, group_value) in group_values {
            let trace_measurements: Vec<_> = filtered_measurements
                .clone()
                .filter(|(_, m)| {
                    group_key
                        .map(|key| m.key_values.get(key) == group_value)
                        .unwrap_or(true)
                })
                .collect();
            plot.add_trace(trace_measurements, group_value);
        }
    }

    // TODO(kaihowl) fewer than the -n specified measurements appear in plot (old problem, even in
    // python)

    if output == Path::new("-") {
        io::stdout()
            .write_all(&plot.as_bytes())
            .expect("Could not write to stdout");
    } else {
        File::create(&output)
            .expect("Cannot open file")
            .write_all(&plot.as_bytes())
            .expect("Could not write file");
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
struct Commit {
    commit: String,
    measurements: Vec<MeasurementData>,
}

#[derive(Debug)]
enum DeserializationError {
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
fn walk_commits(
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
                Ok(note) => deserialize(note.message().unwrap_or("")),
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
