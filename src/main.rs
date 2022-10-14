use std::{collections::HashMap, path::PathBuf, str};

use clap::{Args, Parser, Subcommand, ValueEnum};

use serde::Deserialize;

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
        #[command(flatten)]
        report_history: CliReportHistory,

        /// Key-value pair separated by "=" with no whitespaces to subselect measurements
        #[arg(short, long, value_parser=parse_key_value)]
        selector: Vec<(String, String)>,

        /// Minimum number of measurements needed. If less, pass test and assume
        /// more measurements are needed.
        #[arg(short, long)]
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
            report_history: _,
            selector: _,
            min_measurements: _,
            aggregate_by: _,
            sigma: _,
        } => todo!(),
        Commands::Good { measurement: _ } => todo!(),
        Commands::Prune {} => todo!(),
    }
}

fn report(num_commits: usize) {
    use git2::Repository;
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    if let Err(e) = walk_commits(&repo, num_commits) {
        panic!("Failed to walk tree: {}", e);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Measurement {
    commit: String,
    measurement: MeasurementData,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct MeasurementData {
    name: String,
    // TODO(kaihowl) change type
    timestamp: i32,
    // TODO(kaihowl) check size of type
    val: i32,
    #[serde(flatten)]
    key_values: HashMap<String, String>,
}

fn deserialize(lines: &str, commit_id: &str) -> Vec<Measurement> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .flexible(true)
        .from_reader(lines.as_bytes());
    reader.set_headers(csv::StringRecord::from(vec![
        "name",
        "timestamp",
        "val",
        "key_values",
    ]));
    reader
        .deserialize()
        .map(|r| {
            // TODO(kaihowl) no unwrap
            let md: MeasurementData = r.unwrap();
            Measurement {
                // TODO(kaihowl) oh man
                commit: commit_id.to_string(),
                measurement: md,
            }
        })
        .collect()
}

fn walk_commits(repo: &git2::Repository, num_commits: usize) -> Result<(), git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.simplify_first_parent()?;
    let notes = revwalk
        .take(num_commits)
        .filter_map(|commit| repo.find_note(Some("refs/notes/perf"), commit.ok()?).ok());

    for note in notes {
        let lines = note.message().unwrap_or("");
        let commit_id = note.id().to_string();
        // TODO(kaihowl) maybe split up serialization of MeasurementData
        deserialize(lines, &commit_id)
            .into_iter()
            .for_each(|m| println!("{:?}", m));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn key_value_deserialization() {
        let lines = "test 1234 123 key1=value1 key2=value2";
        let commit_id = "deadbeef";
        let actual = deserialize(lines, commit_id);
        let expected = Measurement {
            commit: commit_id.to_string(),
            measurement: MeasurementData {
                name: "test".to_string(),
                timestamp: 1234,
                val: 123,
                key_values: [
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                ]
                .into(),
            },
        };
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0], expected);
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
