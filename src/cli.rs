use clap::{error::ErrorKind::ArgumentConflict, Args, Parser};
use clap::{CommandFactory, Subcommand};
use std::path::PathBuf;

use crate::config::bump_epoch;
use crate::measurement_retrieval::ReductionFunc;
use crate::pull;
use crate::push;
use crate::reporting::report;
use crate::CliError;
use crate::{add, audit};
use crate::{measure, prune};

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
    /// Limit the number of previous commits considered.
    /// HEAD is included in this count.
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

    /// Remove all performance measurements for non-existent/unreachable objects.
    /// Will refuse to work if run on a shallow clone.
    Prune {},

    /// Generate the manpage content
    #[command(hide = true)]
    Manpage {},
}

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

pub fn handle_calls() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Measure {
            repetitions,
            command,
            measurement,
        } => Ok(measure(
            &measurement.name,
            repetitions,
            &command,
            &measurement.key_value,
        )?),
        Commands::Add { value, measurement } => {
            Ok(add(&measurement.name, value, &measurement.key_value)?)
        }
        Commands::Push {} => Ok(push(None)?),
        Commands::Pull {} => Ok(pull(None)?),
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
            Ok(audit::audit(
                &measurement,
                report_history.max_count,
                min_measurements,
                &selectors,
                aggregate_by,
                sigma,
            )?)
        }
        Commands::BumpEpoch { measurement } => Ok(bump_epoch(&measurement)?),
        Commands::Prune {} => Ok(prune()?),
        Commands::Manpage {} => {
            generate_manpage().expect("Man page generation failed");
            Ok(())
        }
    }
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
    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }
}
