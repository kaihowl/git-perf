use std::collections::HashSet;
use std::io::Write;
use std::{collections::HashMap, fs::File, path::PathBuf, process::ExitCode, str};

use itertools::{self, EitherOrBoth, Itertools};

use clap::{
    error::ErrorKind::ArgumentConflict, Args, CommandFactory, Parser, Subcommand, ValueEnum,
};

use plotly::layout::Axis;
use plotly::BoxPlot;
use plotly::{common::Title, Histogram, Layout, Plot};
use serde::Deserialize;

use average::{self, concatenate, Estimate, Mean, Variance};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct CliMeasurement {
    /// Name of the measurement
    #[arg(short = 'm', long = "measurement")]
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
        #[arg(short = 'n', long, default_value = "1")]
        repetitions: i32,

        /// Command to measure
        command: Vec<String>,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Add single measurement
    Add {
        // TODO(kaihowl) this is missing float values
        /// Measured value to be added
        value: i32,

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

        // TODO(kaihowl) No check for spaces, etc... Same applies to KV parsing method.
        /// Create individual traces in the graph by grouping with the value of this selector
        #[arg(short, long)]
        separate_by: Option<String>,
    },

    /// For a given measurement, check perfomance deviations of the HEAD commit
    /// against <n> previous commits. Group previous results and aggregate their
    /// results before comparison.
    Audit {
        #[arg(short, long)]
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

        // TODO(hoewelmk) missing short arg
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
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum AggregationFunc {
    Min,
    Max,
    Median,
    Average,
}

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value: no '=' found in '{}'", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() -> ExitCode {
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
            ExitCode::SUCCESS
        }
        Commands::Add { value, measurement } => {
            println!(
                "Measurement: {}, value: {}, key-values: {:?}",
                measurement.name, value, measurement.key_value
            );
            ExitCode::SUCCESS
        }
        Commands::Push {} => todo!(),
        Commands::Pull {} => todo!(),
        Commands::Report {
            output,
            separate_by,
            report_history,
        } => {
            report(output, separate_by, report_history.max_count);
            ExitCode::SUCCESS
        }
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
            audit(
                &measurement,
                report_history.max_count,
                min_measurements,
                &selectors,
                aggregate_by,
                sigma,
            )
        }
        Commands::Good { measurement: _ } => todo!(),
        Commands::Prune {} => todo!(),
    }
}

fn audit(
    measurement: &str,
    max_count: usize,
    min_count: u16,
    selectors: &Vec<(String, String)>,
    aggregate_by: AggregationFunc,
    sigma: f64,
) -> ExitCode {
    let all = retrieve_measurements(max_count + 1); // include HEAD

    let filter_by = |m: &&MeasurementData| {
        m.name == measurement && selectors.iter().all(|s| &m.key_values[&s.0] == &s.1)
    };

    let head_summary = aggregate_measurements(all.iter().take(1), aggregate_by, &filter_by);
    let tail_summary = aggregate_measurements(all.iter().skip(1), aggregate_by, &filter_by);
    println!("head: {:?}, tail: {:?}", head_summary, tail_summary);

    if head_summary.len == 0 {
        println!("No measurement for HEAD");
        return ExitCode::FAILURE;
    }

    if tail_summary.len < min_count.into() {
        println!("Only {} measurements found. Less than requested min_measurements of {}. Skipping test.", tail_summary.len, min_count);
        return ExitCode::SUCCESS;
    }

    if head_summary.significantly_different_from(&tail_summary, sigma) {
        println!("Measurements differ significantly");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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

concatenate!(AggStats, [Mean, mean], [Variance, sample_variance]);

fn aggregate_measurements<'a, F>(
    commits: impl Iterator<Item = &'a Commit>,
    aggregate_by: AggregationFunc,
    filter_by: &F,
) -> Stats
where
    F: Fn(&&MeasurementData) -> bool,
{
    let s: AggStats = commits
        // TODO(kaihowl) configure aggregate_by
        .filter_map(|c| {
            println!("{:?}", c.commit);
            c.measurements
                .iter()
                .take_while(filter_by)
                .inspect(|m| println!("{:?}", m))
                .map(|m| m.val)
                .reduce(f64::min)
        })
        .inspect(|m| println!("min: {:?}", m))
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

fn retrieve_measurements(num_commits: usize) -> Vec<Commit> {
    use git2::Repository;
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let measurements = walk_commits(&repo, num_commits);

    match measurements {
        Err(e) => panic!("Failed to walk tree: {:?}", e),
        Ok(measurements) => measurements,
    }
}

fn report(output: PathBuf, separate_by: Option<String>, num_commits: usize) {
    let measurements = retrieve_measurements(num_commits);

    let (commit_nrs, short_hashes): (Vec<_>, Vec<_>) = measurements
        .iter()
        .rev()
        .enumerate()
        .map(|(n, c)| (n as f64, c.commit[..6].to_owned()))
        .unzip();

    let (measurements_commit_nrs, measurements): (Vec<_>, Vec<_>) = measurements
        .into_iter()
        .rev()
        .enumerate()
        .flat_map(|(n, c)| c.measurements.into_iter().map(move |m| (n, m)))
        .unzip();

    let measurement_groups: HashSet<_> = measurements.iter().map(|m| &m.name).collect();
    println!("{:?}", measurement_groups);

    let x_axis = Axis::new().tick_values(commit_nrs).tick_text(short_hashes);
    let layout = Layout::new()
        .title(Title::new("Something, something"))
        .x_axis(x_axis);
    let mut plot = Plot::new();
    plot.set_layout(layout);

    for group in measurement_groups {
        let (x, y): (Vec<usize>, Vec<f64>) = measurements_commit_nrs
            .iter()
            .zip(measurements.iter())
            .filter(|(_i, m)| m.name == *group)
            .map(|(i, m)| (i, m.val))
            .unzip();
        let trace = BoxPlot::new_xy(x, y).name(group);
        plot.add_trace(trace);
    }
    File::create(output)
        .expect("Cannot open file")
        .write_all(plot.to_html().as_bytes())
        .expect("Could not write file");
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

#[derive(Debug)]
enum DeserializationError {
    CsvError(csv::Error),
    GitError(git2::Error),
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

fn deserialize(lines: &str) -> Result<Vec<MeasurementData>, DeserializationError> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .has_headers(false)
        .flexible(true)
        .from_reader(lines.as_bytes());

    let result = reader
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
        .try_collect();
    result
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
}
