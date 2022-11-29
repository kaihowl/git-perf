use std::convert::identity;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::{collections::HashMap, fs::File, path::PathBuf, str};

use git2::{Index, IndexEntry, IndexEntryExtendedFlag, MergeOptions, Repository};
use itertools::{self, EitherOrBoth, Itertools};

use clap::{
    error::ErrorKind::ArgumentConflict, Args, CommandFactory, Parser, Subcommand, ValueEnum,
};

use plotly::common::{Font, LegendGroupTitle};
use plotly::layout::Axis;
use plotly::BoxPlot;
use plotly::{common::Title, Layout, Plot};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};

use average::{self, concatenate, Estimate, Mean, Variance};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct CliMeasurement {
    /// Name of the measurement
    #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string)]
    name: String,

    /// Key-value pairs separated by '='
    #[arg(short, long, value_parser=parse_key_value)]
    key_value: Vec<(String, String)>,
}

#[derive(Args)]
struct CliReportHistory {
    /// Limit the number of previous commits considered
    #[arg(short = 'n', long, default_value = "40")]
    max_count: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Measure the runtime of the supplied command
    Measure {
        /// Repetitions
        #[arg(short = 'n', long, value_parser=clap::value_parser!(u16).range(1..), default_value = "1")]
        repetitions: u16,

        /// Command to measure
        command: Vec<String>,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Add single measurement
    Add {
        /// Measured value to be added
        value: f64,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Publish performance results to remote
    Push {},

    /// Pull performance results from remote
    Pull {},

    /// Create an HTML performance report
    Report {
        /// HTML output file
        #[arg(short, long, default_value = "output.html")]
        output: PathBuf,

        #[command(flatten)]
        report_history: CliReportHistory,

        /// Select an individual measurements instead of all
        #[arg(short, long)]
        measurement: Vec<String>,

        /// Key-value pairs separated by '=', select only matching measurements
        #[arg(short, long, value_parser=parse_key_value)]
        key_value: Vec<(String, String)>,

        /// Create individual traces in the graph by grouping with the value of this selector
        #[arg(short, long, value_parser=parse_spaceless_string)]
        separate_by: Option<String>,
    },

    /// For a given measurement, check perfomance deviations of the HEAD commit
    /// against <n> previous commits. Group previous results and aggregate their
    /// results before comparison.
    Audit {
        #[arg(short, long, value_parser=parse_spaceless_string)]
        measurement: String,

        #[command(flatten)]
        report_history: CliReportHistory,

        /// Key-value pair separated by "=" with no whitespaces to subselect measurements
        #[arg(short, long, value_parser=parse_key_value)]
        selectors: Vec<(String, String)>,

        /// Minimum number of measurements needed. If less, pass test and assume
        /// more measurements are needed.
        #[arg(long, value_parser=clap::value_parser!(u16).range(1..), default_value="1")]
        min_measurements: u16,

        /// What to aggregate the measurements in each group with
        #[arg(short, long, default_value = "min")]
        aggregate_by: AggregationFunc,

        /// Multiple of the stddev after which a outlier is detected.
        /// If the HEAD measurement is within [mean-<d>*sigma; mean+<d>*sigma],
        /// it is considered acceptable.
        #[arg(short = 'd', long, default_value = "4.0")]
        sigma: f64,
    },

    /// Accept HEAD commit's measurement for audit, even if outside of range.
    /// This is allows to accept expected performance changes.
    /// It will copy the current HEAD's measurements to the amended HEAD commit.
    Good {
        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Remove all performance measurements for non-existent/unreachable objects.
    /// Will refuse to work if run on a shallow clone.
    Prune {},

    /// Generate the manpage content
    #[command(hide = true)]
    Manpage {},
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum AggregationFunc {
    Min,
    Max,
    Median,
    Mean,
}

trait VecAggregation {
    fn median(&mut self) -> Option<f64>;
}

concatenate!(AggStats, [Mean, mean], [Variance, sample_variance]);

impl VecAggregation for Vec<f64> {
    fn median(&mut self) -> Option<f64> {
        self.sort_by(f64::total_cmp);
        match self.len() {
            0 => None,
            even if even % 2 == 0 => {
                let left = self[even / 2 - 1];
                let right = self[even / 2];
                Some((left + right) / 2.0)
            }
            odd => Some(self[odd / 2]),
        }
    }
}

trait ReductionFunc: Iterator<Item = f64> {
    // fn aggregate_by() {}
    fn aggregate_by(self, fun: AggregationFunc) -> Option<Self::Item>
    where
        Self: Sized,
    {
        match fun {
            AggregationFunc::Min => self.reduce(f64::min),
            AggregationFunc::Max => self.reduce(f64::max),
            AggregationFunc::Median => self.collect_vec().median(),
            AggregationFunc::Mean => {
                let stats: AggStats = self.collect();
                if stats.mean.is_empty() {
                    None
                } else {
                    Some(stats.mean())
                }
            }
        }
    }
}

impl<T: ?Sized> ReductionFunc for T where T: Iterator<Item = f64> {}

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value: no '=' found in '{}'", s))?;
    let key = parse_spaceless_string(&s[..pos])?;
    let value = parse_spaceless_string(&s[pos + 1..])?;
    Ok((key, value))
}

fn parse_spaceless_string(s: &str) -> Result<String, String> {
    if s.split_whitespace().count() > 1 {
        Err(format!("invalid string/key/value: found space in '{}'", s))
    } else {
        Ok(String::from(s))
    }
}

#[derive(Debug)]
enum CliError {
    Add(AddError),
    PushPull(PushPullError),
    Report(ReportError),
    Audit(AuditError),
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Add(e) => write!(f, "During add: {e}"),
            CliError::PushPull(e) => write!(f, "During push/pull: {e}"),
            CliError::Report(e) => write!(f, "During report: {e}"),
            CliError::Audit(e) => write!(f, "During audit: {e}"),
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

fn handle_calls() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Measure {
            repetitions,
            command,
            measurement,
        } => {
            println!(
                "Measurement: {}, Repetitions: {}, command: {:?}, key-values: {:?}",
                measurement.name, repetitions, command, measurement.key_value
            );
            todo!()
        }
        Commands::Add { value, measurement } => {
            Ok(add(&measurement.name, value, &measurement.key_value)?)
        }
        Commands::Push {} => Ok(push()?),
        Commands::Pull {} => Ok(pull()?),
        Commands::Report {
            output,
            separate_by,
            report_history,
            measurement,
            key_value,
        } => Ok(report(
            output,
            separate_by,
            report_history.max_count,
            &measurement,
            &key_value,
        )?),
        Commands::Audit {
            measurement,
            report_history,
            selectors,
            min_measurements,
            aggregate_by,
            sigma,
        } => {
            if report_history.max_count < min_measurements.into() {
                Cli::command().error(ArgumentConflict, format!("The minimal number of measurements ({}) cannot be more than the maximum number of measurements ({})", min_measurements, report_history.max_count)).exit()
            }
            Ok(audit(
                &measurement,
                report_history.max_count,
                min_measurements,
                &selectors,
                aggregate_by,
                sigma,
            )?)
        }
        Commands::Good { measurement: _ } => todo!(),
        Commands::Prune {} => todo!(),
        Commands::Manpage {} => {
            generate_manpage().expect("Man page generation failed");
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match handle_calls() {
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

fn add(measurement: &str, value: f64, key_values: &[(String, String)]) -> Result<(), AddError> {
    // TODO(kaihowl) configure path
    let repo = Repository::open(".")?;
    let head = repo.head()?;
    let head = head.peel_to_commit()?;
    let author = repo.signature()?;
    // TODO(kaihowl) configure
    let timestamp = author.when().seconds() as f32;
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();

    let md = MeasurementData {
        name: measurement.to_owned(),
        timestamp,
        val: value,
        key_values,
    };

    let serialized = serialize_single(&md);
    let body;

    if let Ok(existing_note) = repo.find_note(Some("refs/notes/perf"), head.id()) {
        // TODO(kaihowl) check empty / not-utf8
        let existing_measurements = existing_note.message().expect("Message is not utf-8");
        // TODO(kaihowl) what about missing trailing new lines?
        body = format!("{}{}", existing_measurements, serialized);
    } else {
        body = serialized;
    }

    repo.note(
        &author,
        &author,
        Some("refs/notes/perf"),
        head.id(),
        &body,
        true,
    )
    .expect("TODO(kaihowl) note failed");

    Ok(())
}

#[derive(Debug)]
enum PushPullError {
    Git(git2::Error),
}

// TODO(kaihowl) code repetition with other git-only errors
impl Display for PushPullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushPullError::Git(e) => write!(f, "git error, {e}"),
        }
    }
}

impl From<git2::Error> for PushPullError {
    fn from(e: git2::Error) -> Self {
        PushPullError::Git(e)
    }
}

/// Resolve conflicts between two measurement runs on the same commit by
/// sorting and deduplicating lines.
/// This emulates the cat_sort_uniq merge strategy for git notes.
fn resolve_conflicts(ours: impl AsRef<str>, theirs: impl AsRef<str>) -> String {
    ours.as_ref()
        .lines()
        .chain(theirs.as_ref().lines())
        .sorted()
        .dedup()
        .join("\n")
}

fn pull() -> Result<(), PushPullError> {
    // TODO(kaihowl) missing conflict resolution
    let repo = Repository::open(".")?;
    let mut remote = repo
        .find_remote("origin")
        .expect("Did not find remote 'origin'");
    // TODO(kaihowl) missing ssh support
    // TODO(kaihowl) silently fails to update the local ref
    remote.fetch(&[&"refs/notes/perf:refs/notes/perf"], None, None)?;
    let notes = repo.find_reference("refs/notes/perf")?;
    let notes = notes.peel_to_commit()?;
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_head = fetch_head.peel_to_commit()?;
    let index = repo.merge_commits(&notes, &fetch_head, None)?;
    let index_paths = index
        .iter()
        .map(|i| String::from_utf8(i.path).unwrap())
        .collect_vec();

    let mut out_index = Index::new()?;
    let mut conflict_entries = Vec::new();

    if let Ok(conflicts) = index.conflicts() {
        conflict_entries = conflicts.try_collect()?;
    }

    for entry in index.iter() {
        if conflict_entries.iter().any(|c| {
            // TODO(kaihowl) think harder about this
            let conflict_entry = if let Some(our) = &c.our {
                our
            } else {
                c.their.as_ref().expect("Both our and their unset")
            };

            conflict_entry.path == entry.path
        }) {
            continue;
        }
        out_index.add(&entry).expect("failing entry in new index");
    }
    for conflict in conflict_entries {
        // TODO(kaihowl) no support for deleted / pruned measurements
        let our = conflict.our.unwrap();
        let our_oid = our.id;
        let our_content = String::from_utf8(repo.find_blob(our_oid)?.content().to_vec())
            .expect("UTF-8 error for our content");
        let their_oid = conflict.their.unwrap().id;
        let their_content = String::from_utf8(repo.find_blob(their_oid)?.content().to_vec())
            .expect("UTF-8 error for their content");
        let resolved_content = resolve_conflicts(&our_content, &their_content);
        // TODO(kaihowl) what should this be set to instead of copied from?
        let blob = repo.blob(resolved_content.as_bytes())?;
        let mut entry = our;
        // Missing bindings for resolving conflict in libgit2-rs. Therefore, manually overwrite.
        entry.flags = 0;
        entry.flags_extended = 0;
        entry.id = blob;

        out_index.add(&entry).expect("Could not add");
    }
    let out_index_paths = out_index
        .iter()
        .map(|i| String::from_utf8(i.path).unwrap())
        .collect_vec();
    println!("TODO(kaihowl) out_index paths: {:?}", out_index_paths);
    println!(
        "TODO(kaihowl) out_index has_conflicts: {} and size: {}",
        out_index.has_conflicts(),
        out_index.len()
    );
    let merged_tree = repo.find_tree(out_index.write_tree_to(&repo)?)?;
    // TODO(kaihowl) make this conditional on the conflicts.
    let signature = repo.signature()?;
    repo.commit(
        Some("refs/notes/perf"),
        &signature,
        &signature,
        "Merge it",
        &merged_tree,
        &[&notes, &fetch_head],
    )?;
    // repo.merge
    Ok(())
}

fn push() -> Result<(), PushPullError> {
    let repo = Repository::open(".")?;
    // TODO(kaihowl) configure remote?
    let mut remote = repo
        .find_remote("origin")
        .expect("Did not find remote 'origin'");
    remote.push(&[&"refs/notes/perf:refs/notes/perf"], None)?;
    Ok(())
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

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &[(String, String)],
    aggregate_by: AggregationFunc,
    sigma: f64,
) -> Result<(), AuditError> {
    let all = retrieve_measurements_by_commit(max_count + 1)?; // include HEAD

    let filter_by = |m: &&MeasurementData| {
        m.name == measurement
            && selectors
                .iter()
                .all(|s| m.key_values.get(&s.0).map(|v| *v == s.1).unwrap_or(false))
    };

    let head_summary = aggregate_measurements(all.iter().take(1), aggregate_by, &filter_by);
    let tail_summary = aggregate_measurements(all.iter().skip(1), aggregate_by, &filter_by);
    println!("head: {:?}, tail: {:?}", head_summary, tail_summary);

    if head_summary.len == 0 {
        return Err(AuditError::NoMeasurementForHead);
    }

    if tail_summary.len < min_count.into() {
        // TODO(kaihowl) handle with explicit return? Print text somewhere else?
        println!("Only {} measurements found. Less than requested min_measurements of {}. Skipping test.", tail_summary.len, min_count);
        return Ok(());
    }

    if head_summary.significantly_different_from(&tail_summary, sigma) {
        println!("Measurements differ significantly");
        // TODO(kaihowl) print details
        return Err(AuditError::SignificantDifference);
    }

    Ok(())
}

#[derive(Debug)]
struct Stats {
    mean: f64,
    stddev: f64,
    len: usize,
}

impl Stats {
    fn significantly_different_from(&self, other: &Stats, sigma: f64) -> bool {
        assert!(self.len == 1);
        assert!(other.len >= 1);
        (self.mean - other.mean).abs() / other.stddev > sigma
    }
}

fn aggregate_measurements<'a, F>(
    commits: impl Iterator<Item = &'a Commit>,
    aggregate_by: AggregationFunc,
    filter_by: &F,
) -> Stats
where
    F: Fn(&&MeasurementData) -> bool,
{
    let s: AggStats = commits
        .filter_map(|c| {
            println!("{:?}", c.commit);
            c.measurements
                .iter()
                .filter(filter_by)
                .inspect(|m| println!("{:?}", m))
                .map(|m| m.val)
                .aggregate_by(aggregate_by)
        })
        .inspect(|m| println!("aggregated value ({:?}): {:?}", aggregate_by, m))
        .collect();
    Stats {
        mean: s.mean(),
        stddev: s.sample_variance().sqrt(),
        len: s.mean.len() as usize,
    }

    // measurements
    //     .iter()
    //     .enumerate()
    //     .fold((0, 0), |(old_mean, old_variance), (index, md)| {
    //         let prevq = old
    //         let mean = old_mean + (md.val - old_mean) / (index + 1);
    //         let variance =
    //     });
}

fn retrieve_measurements_by_commit(
    num_commits: usize,
) -> Result<Vec<Commit>, DeserializationError> {
    let repo = Repository::open(".")?;
    walk_commits(&repo, num_commits)
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

// TODO(kaihowl) needs more fine grained output e2e tests
fn report(
    output: PathBuf,
    separate_by: Option<String>,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
) -> Result<(), ReportError> {
    println!("hoewelmk: separate_by: {:?}", separate_by);
    let mut commits = retrieve_measurements_by_commit(num_commits)?;
    commits.reverse();

    let mut plot =
        ReporterFactory::from_file_name(&output).ok_or(ReportError::InvalidOutputFormat)?;

    println!("hoewelmk: measurements.len: {:?}", commits.len());

    plot.add_commits(&commits);

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

    File::create(&output)
        .expect("Cannot open file")
        .write_all(&plot.as_bytes())
        .expect("Could not write file");

    Ok(())
}

#[derive(Debug, PartialEq)]
struct Commit {
    commit: String,
    measurements: Vec<MeasurementData>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct MeasurementData {
    name: String,
    // TODO(kaihowl) change type
    timestamp: f32,
    // TODO(kaihowl) check size of type
    val: f64,
    #[serde(flatten)]
    key_values: HashMap<String, String>,
}

// TODO(kaihowl) serialization with flatten and custom function does not work
#[derive(Debug, PartialEq, Serialize)]
struct SerializeMeasurementData<'a> {
    name: &'a str,
    timestamp: f32,
    val: f64,
    #[serde(serialize_with = "key_value_serialization")]
    key_values: &'a HashMap<String, String>,
}

impl Serialize for MeasurementData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeMeasurementData::from(self).serialize(serializer)
    }
}

impl<'a> From<&'a MeasurementData> for SerializeMeasurementData<'a> {
    fn from(md: &'a MeasurementData) -> Self {
        SerializeMeasurementData {
            name: md.name.as_str(),
            timestamp: md.timestamp,
            val: md.val,
            key_values: &md.key_values,
        }
    }
}

fn key_value_serialization<S>(
    key_values: &HashMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(key_values.len()))?;
    for (k, v) in key_values {
        seq.serialize_element(&format!("{}={}", k, v))?
    }
    seq.end()
}

#[derive(Debug)]
enum DeserializationError {
    CsvError(csv::Error),
    GitError(git2::Error),
}

impl Display for DeserializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserializationError::CsvError(e) => write!(f, "csv error, {e}"),
            DeserializationError::GitError(e) => write!(f, "git error, {e}"),
        }
    }
}

impl From<csv::Error> for DeserializationError {
    fn from(value: csv::Error) -> Self {
        DeserializationError::CsvError(value)
    }
}

impl From<git2::Error> for DeserializationError {
    fn from(value: git2::Error) -> Self {
        DeserializationError::GitError(value)
    }
}

fn serialize_single(measurement_data: &MeasurementData) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b' ')
        .has_headers(false)
        .flexible(true)
        .from_writer(vec![]);

    writer
        .serialize(measurement_data)
        .expect("TODO(kaihowl) fix me");
    String::from_utf8(writer.into_inner().unwrap()).unwrap()
}

fn deserialize(lines: &str) -> Result<Vec<MeasurementData>, DeserializationError> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .has_headers(false)
        .flexible(true)
        .from_reader(lines.as_bytes());

    reader
        .into_records()
        .map(|r| {
            let fixed_headers = vec!["name", "timestamp", "val"];

            let (headers, values): (csv::StringRecord, csv::StringRecord) = r?
                .into_iter()
                .zip_longest(fixed_headers)
                .map(|pair| match pair {
                    EitherOrBoth::Both(val, header) => (header, val),
                    EitherOrBoth::Right(_) => {
                        // TODO(kaihowl) skip the record instead
                        panic!("Too few values");
                    }
                    EitherOrBoth::Left(keyvalue) => match keyvalue.split_once('=') {
                        Some(a) => a,
                        None => {
                            // TODO(kaihowl) skip the record instead
                            panic!("No equals sign in key value pair");
                        }
                    },
                })
                .unzip();

            let md: MeasurementData = values.deserialize(Some(&headers)).unwrap();
            Ok(md)
        })
        .try_collect()
}

fn walk_commits(
    repo: &git2::Repository,
    num_commits: usize,
) -> Result<Vec<Commit>, DeserializationError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.simplify_first_parent()?;
    revwalk
        .take(num_commits)
        .map(|commit_oid| {
            let commit_id = commit_oid?;
            let measurements = match repo.find_note(Some("refs/notes/perf"), commit_id) {
                // TODO(kaihowl) remove unwrap_or
                Ok(note) => deserialize(note.message().unwrap_or("")),
                Err(_) => Ok([].into()),
            };
            Ok(Commit {
                commit: commit_id.to_string(),
                measurements: measurements?,
            })
        })
        .try_collect()
}

fn generate_manpage() -> Result<(), std::io::Error> {
    let man = clap_mangen::Man::new(Cli::command());
    man.render(&mut std::io::stdout())?;

    // TODO(kaihowl) this does not look very nice. Fix it.
    for command in Cli::command()
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
    {
        let man = clap_mangen::Man::new(command.clone());
        man.render(&mut std::io::stdout())?
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn no_floating_error() {
        let commits = (0..100)
            .map(|_| -> Commit {
                Commit {
                    commit: "deadbeef".into(),
                    measurements: [MeasurementData {
                        name: "mymeasurement".into(),
                        timestamp: 120.0,
                        val: 0.1,
                        key_values: [].into(),
                    }]
                    .into(),
                }
            })
            .collect_vec();
        let stats = aggregate_measurements(commits.iter(), AggregationFunc::Min, &|_| true);
        assert_eq!(stats.mean, 0.1);
        assert_eq!(stats.len, 100);
        let naive_mean = (0..100).map(|_| 0.1).sum::<f64>() / 100.0;
        assert_ne!(naive_mean, 0.1);
    }

    #[test]
    fn single_measurement() {
        let commits = vec![Commit {
            commit: "deadbeef".into(),
            measurements: [MeasurementData {
                name: "mymeasurement".into(),
                timestamp: 123.0,
                val: 1.0,
                key_values: [].into(),
            }]
            .into(),
        }];
        let stats = aggregate_measurements(commits.iter(), AggregationFunc::Min, &|_| true);
        assert_eq!(stats.len, 1);
        assert_eq!(stats.mean, 1.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn no_measurement() {
        let commits = vec![Commit {
            commit: "deadbeef".into(),
            measurements: [].into(),
        }];
        let stats = aggregate_measurements(commits.iter(), AggregationFunc::Min, &|_| true);
        assert_eq!(stats.len, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.stddev, 0.0);
    }

    #[test]
    fn z_score_with_zero_stddev() {
        let stddev = 0.0;
        let mean = 30.0;
        let higher_val = 50.0;
        let lower_val = 10.0;
        let z_high = ((higher_val - mean) / stddev as f64).abs();
        let z_low = ((lower_val - mean) / stddev as f64).abs();
        assert_eq!(z_high, f64::INFINITY);
        assert_eq!(z_low, f64::INFINITY);
    }

    #[test]
    fn key_value_deserialization() {
        let lines = "test 1234 123 key1=value1 key2=value2";
        let actual = deserialize(lines);
        let expected = MeasurementData {
            name: "test".to_string(),
            timestamp: 1234.0,
            val: 123.0,
            key_values: [
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ]
            .into(),
        };
        assert_eq!(actual.as_ref().unwrap().len(), 1);
        assert_eq!(actual.unwrap()[0], expected);
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn verify_stats() {
        let empty_vec = [];
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(AggregationFunc::Min)
        );
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(AggregationFunc::Max)
        );
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(AggregationFunc::Median)
        );
        assert_eq!(
            None,
            empty_vec.into_iter().aggregate_by(AggregationFunc::Mean)
        );

        let single_el_vec = [3.0];
        assert_eq!(
            Some(3.0),
            single_el_vec.into_iter().aggregate_by(AggregationFunc::Min)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec.into_iter().aggregate_by(AggregationFunc::Max)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec
                .into_iter()
                .aggregate_by(AggregationFunc::Median)
        );
        assert_eq!(
            Some(3.0),
            single_el_vec
                .into_iter()
                .aggregate_by(AggregationFunc::Mean)
        );

        let two_el_vec = [3.0, 1.0];
        assert_eq!(
            Some(1.0),
            two_el_vec.into_iter().aggregate_by(AggregationFunc::Min)
        );
        assert_eq!(
            Some(3.0),
            two_el_vec.into_iter().aggregate_by(AggregationFunc::Max)
        );
        assert_eq!(
            Some(2.0),
            two_el_vec.into_iter().aggregate_by(AggregationFunc::Median)
        );
        assert_eq!(
            Some(2.0),
            two_el_vec.into_iter().aggregate_by(AggregationFunc::Mean)
        );

        let three_el_vec = [2.0, 6.0, 1.0];
        assert_eq!(
            Some(1.0),
            three_el_vec.into_iter().aggregate_by(AggregationFunc::Min)
        );
        assert_eq!(
            Some(6.0),
            three_el_vec.into_iter().aggregate_by(AggregationFunc::Max)
        );
        assert_eq!(
            Some(2.0),
            three_el_vec
                .into_iter()
                .aggregate_by(AggregationFunc::Median)
        );
        assert_eq!(
            Some(3.0),
            three_el_vec.into_iter().aggregate_by(AggregationFunc::Mean)
        );
    }

    #[test]
    fn test_serialize_single() {
        let md = MeasurementData {
            name: "Mymeasurement".into(),
            timestamp: 1234567.0,
            val: 42.0,
            key_values: [("mykey".to_string(), "myvalue".to_string())].into(),
        };
        let serialized = serialize_single(&md);
        assert_eq!(serialized, "Mymeasurement 1234567.0 42.0 mykey=myvalue\n");
    }

    #[test]
    fn test_resolve_conflicts() {
        let a = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234567.0 42.0\n";
        let b = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234890.0 22.0\n";

        let resolved = resolve_conflicts(a, b);
        assert!(resolved.contains("mymeasurement 1234567.0 23.0 key=value"));
        assert!(resolved.contains("myothermeasurement 1234567.0 42.0"));
        assert!(resolved.contains("myothermeasurement 1234890.0 22.0"));
        assert_eq!(3, resolved.lines().count());
    }

    #[test]
    fn test_resolve_conflicts_no_trailing_newline() {
        let a = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234567.0 42.0";
        let b = "mymeasurement 1234567.0 23.0 key=value\nmyothermeasurement 1234890.0 22.0";

        let resolved = resolve_conflicts(a, b);
        assert!(resolved.contains("mymeasurement 1234567.0 23.0 key=value"));
        assert!(resolved.contains("myothermeasurement 1234567.0 42.0"));
        assert!(resolved.contains("myothermeasurement 1234890.0 22.0"));
        assert_eq!(3, resolved.lines().count());
    }
}
