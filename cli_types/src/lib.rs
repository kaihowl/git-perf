use anyhow::{anyhow, bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use chrono::prelude::*;
use chrono::Duration;

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReductionFunc {
    Min,
    Max,
    Median,
    Mean,
}

#[derive(Parser)]
#[command(version, name = "git-perf")]
pub struct Cli {
    /// Increase verbosity level (can be specified multiple times.) The first level sets level
    /// "info", second sets level "debug", and third sets level "trace" for the logger.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
pub struct CliMeasurement {
    /// Name of the measurement
    #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string)]
    pub name: String,

    /// Key-value pairs separated by '='
    #[arg(short, long, value_parser=parse_key_value)]
    pub key_value: Vec<(String, String)>,
}

#[derive(Args)]
pub struct CliReportHistory {
    /// Limit the number of previous commits considered.
    /// HEAD is included in this count.
    #[arg(short = 'n', long, default_value = "40")]
    pub max_count: usize,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Measure the runtime of the supplied command (in nanoseconds)
    Measure {
        /// Repetitions
        #[arg(short = 'n', long, value_parser=clap::value_parser!(u16).range(1..), default_value = "1")]
        repetitions: u16,

        #[command(flatten)]
        measurement: CliMeasurement,

        /// Command to measure
        #[arg(required(true), last(true))]
        command: Vec<String>,
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

        /// What to aggregate the measurements in each group with
        #[arg(short, long)]
        aggregate_by: Option<ReductionFunc>,
    },

    /// For a given measurement, check perfomance deviations of the HEAD commit
    /// against `<n>` previous commits. Group previous results and aggregate their
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
        /// A minimum of two historic measurements are needed for proper evaluation of standard
        /// deviation.
        // TODO(kaihowl) fix up min value and default_value
        #[arg(long, value_parser=clap::value_parser!(u16).range(1..), default_value="2")]
        min_measurements: u16,

        /// What to aggregate the measurements in each group with
        #[arg(short, long, default_value = "min")]
        aggregate_by: ReductionFunc,

        /// Multiple of the stddev after which a outlier is detected.
        /// If the HEAD measurement is within `[mean-<d>*sigma; mean+<d>*sigma]`,
        /// it is considered acceptable.
        #[arg(short = 'd', long, default_value = "4.0")]
        sigma: f64,
    },

    /// Accept HEAD commit's measurement for audit, even if outside of range.
    /// This is allows to accept expected performance changes.
    /// This is accomplished by starting a new epoch for the given measurement.
    /// The epoch is configured in the git perf config file.
    /// A change to the epoch therefore has to be committed and will result in a new HEAD for which
    /// new measurements have to be taken.
    BumpEpoch {
        #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string)]
        measurement: String,
    },

    /// Remove all performance measurements for commits that have been committed
    /// before the specified time period.
    Remove {
        #[arg(long = "older-than", value_parser = parse_datetime_value_now)]
        older_than: DateTime<Utc>,
    },

    /// Remove all performance measurements for non-existent/unreachable objects.
    /// Will refuse to work if run on a shallow clone.
    Prune {},
}

fn parse_key_value(s: &str) -> Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| anyhow!("invalid key=value: no '=' found in '{}'", s))?;
    let key = parse_spaceless_string(&s[..pos])?;
    let value = parse_spaceless_string(&s[pos + 1..])?;
    Ok((key, value))
}

fn parse_spaceless_string(s: &str) -> Result<String> {
    if s.split_whitespace().count() > 1 {
        Err(anyhow!("invalid string/key/value: found space in '{}'", s))
    } else {
        Ok(String::from(s))
    }
}

fn parse_datetime_value_now(input: &str) -> Result<DateTime<Utc>> {
    parse_datetime_value(&Utc::now(), input)
}

fn parse_datetime_value(now: &DateTime<Utc>, input: &str) -> Result<DateTime<Utc>> {
    if input.len() < 2 {
        bail!("Invalid datetime format");
    }

    let (num, unit) = input.split_at(input.len() - 1);
    let num: i64 = num.parse()?;
    let subtractor = match unit {
        "w" => Duration::weeks(num),
        "d" => Duration::days(num),
        "h" => Duration::hours(num),
        _ => bail!("Unsupported datetime format"),
    };
    Ok(*now - subtractor)
}

#[cfg(test)]
mod test {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }

    #[test]
    fn verify_date_parsing() {
        let now = Utc::now();

        assert_eq!(
            now - Duration::weeks(2),
            parse_datetime_value(&now, "2w").unwrap()
        );

        assert_eq!(
            now - Duration::days(30),
            parse_datetime_value(&now, "30d").unwrap()
        );

        assert_eq!(
            now - Duration::hours(72),
            parse_datetime_value(&now, "72h").unwrap()
        );

        assert!(parse_datetime_value(&now, " 2w ").is_err());

        assert!(parse_datetime_value(&now, "").is_err());

        assert!(parse_datetime_value(&now, "945kjfg").is_err());
    }
}
