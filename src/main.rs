use std::{collections::HashMap, path::PathBuf, str};

use itertools::{self, EitherOrBoth, Itertools};

use clap::{Args, Parser, Subcommand, ValueEnum};

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

    // TODO(kaihowl) No check for spaces...
    /// What to group the measurements by
    #[arg(short, long, default_value = "commit")]
    group_by: String,
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
        #[arg(long)]
        min_measurements: Option<i32>,

        // TODO(hoewelmk) missing short arg
        /// What to aggregate the measurements in each group with
        #[arg(short, long, default_value = "min")]
        aggregate_by: AggregationFunc,

        /// Multiple of the stddev after which a outlier is detected.
        /// If the HEAD measurement is within [mean-<d>*sigma; mean+<d>*sigma],
        /// it is considered acceptable.
        #[arg(short = 'd', long, default_value = "4")]
        sigma: f32,
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

fn main() {
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
        }
        Commands::Add { value, measurement } => {
            println!(
                "Measurement: {}, value: {}, key-values: {:?}",
                measurement.name, value, measurement.key_value
            );
        }
        Commands::Push {} => todo!(),
        Commands::Pull {} => todo!(),
        Commands::Report {
            output: _,
            separate_by: _,
            report_history,
        } => report(report_history.max_count),
        Commands::Audit {
            measurement,
            report_history,
            selectors,
            // TODO(kaihowl)
            min_measurements: _,
            aggregate_by,
            sigma,
        } => audit(
            &measurement,
            report_history,
            &selectors,
            aggregate_by,
            sigma,
        ),
        Commands::Good { measurement: _ } => todo!(),
        Commands::Prune {} => todo!(),
    }
}

// TODO(kaihowl) do not use cli structure?
fn audit(
    measurement: &str,
    report_history: CliReportHistory,
    selectors: &Vec<(String, String)>,
    aggregate_by: AggregationFunc,
    sigma: f32,
) {
    let all = retrieve_measurements(report_history.max_count + 1); // include HEAD
    let head = match all.first() {
        Some(head) => head,
        None => {
            panic!("No measurement for HEAD")
        }
    };

    let filter_by = |m: &&MeasurementData| {
        m.name == measurement && selectors.iter().all(|s| &m.key_values[&s.0] == &s.1)
    };
    let head_summary = aggregate_measurements(all.iter().take(1), aggregate_by, &filter_by);
    let tail_summary = aggregate_measurements(all.iter().skip(1), aggregate_by, &filter_by);
    println!("head: {:?}, tail: {:?}", head_summary, tail_summary);
}

#[derive(Debug)]
struct Stats {
    mean: f64,
    stddev: f64,
}

concatenate!(MeanVariance, [Mean, mean], [Variance, population_variance]);

fn aggregate_measurements<'a, F>(
    commits: impl Iterator<Item = &'a Commit>,
    aggregate_by: AggregationFunc,
    filter_by: &F,
) -> Stats
where
    F: Fn(&&MeasurementData) -> bool,
{
    let s: MeanVariance = commits
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
        stddev: s.population_variance().sqrt(),
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

fn report(num_commits: usize) {
    retrieve_measurements(num_commits)
        .into_iter()
        .for_each(|m| println!("{:?}", m));
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
        .filter_map(|commit| repo.find_note(Some("refs/notes/perf"), commit.ok()?).ok())
        .map(|note| {
            let lines = note.message().unwrap_or("");
            let commit = note.id().to_string();
            let measurements = deserialize(lines)?;
            Ok(Commit {
                commit,
                measurements,
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
        let naive_mean = (0..100).map(|_| 0.1).sum::<f64>() / 100.0;
        assert_ne!(naive_mean, 0.1);
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
