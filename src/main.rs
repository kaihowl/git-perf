use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

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
    max_count: i32,

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

fn report(_num_commits: i32) {
    use git2::Repository;
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let head = find_head_commit(&repo);

    println!("name of HEAD: {:?}", head);
}

fn find_head_commit(repo: &git2::Repository) -> Result<git2::Commit, git2::Error> {
    let head = repo.head();
    let head = head?.resolve()?.peel_to_commit()?;
    println!("Result of head: {:?}", head.author().name());
    Ok(head)
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
