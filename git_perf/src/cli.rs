use anyhow::Result;
use clap::CommandFactory;
use clap::{error::ErrorKind::ArgumentConflict, Parser};
use env_logger::Env;
use log::Level;

use crate::audit;
use crate::basic_measure::measure;
use crate::config::bump_epoch;
use crate::git::git_interop::check_git_version;
use crate::git::git_interop::{list_commits_with_measurements, prune, pull, push};
use crate::measurement_storage::{add, remove_measurements_from_commits};
use crate::reporting::report;
use crate::stats::ReductionFunc;
use git_perf_cli_types::{Cli, Commands};

pub fn handle_calls() -> Result<()> {
    let cli = Cli::parse();
    let logger_level = match cli.verbose {
        0 => Level::Warn,
        1 => Level::Info,
        2 => Level::Debug,
        _ => Level::Trace,
    };
    env_logger::Builder::from_env(Env::default().default_filter_or(logger_level.as_str())).init();

    check_git_version()?;

    match cli.command {
        Commands::Measure {
            repetitions,
            command,
            measurement,
        } => measure(
            &measurement.name,
            repetitions,
            &command,
            &measurement.key_value,
        ),
        Commands::Add { value, measurement } => {
            add(&measurement.name, value, &measurement.key_value)
        }
        Commands::Push { remote } => push(None, remote.as_deref()),
        Commands::Pull {} => pull(None),
        Commands::Report {
            output,
            separate_by,
            report_history,
            measurement,
            key_value,
            aggregate_by,
        } => report(
            output,
            separate_by,
            report_history.max_count,
            &measurement,
            &key_value,
            aggregate_by.map(ReductionFunc::from),
        ),
        Commands::Audit {
            measurement,
            report_history,
            selectors,
            min_measurements,
            aggregate_by,
            sigma,
            dispersion_method,
        } => {
            if report_history.max_count < min_measurements.into() {
                Cli::command().error(ArgumentConflict, format!("The minimal number of measurements ({}) cannot be more than the maximum number of measurements ({})", min_measurements, report_history.max_count)).exit()
            }

            let final_dispersion_method =
                determine_dispersion_method(dispersion_method, &measurement);

            audit::audit_multiple(
                &measurement,
                report_history.max_count,
                min_measurements,
                &selectors,
                ReductionFunc::from(aggregate_by),
                sigma,
                final_dispersion_method,
            )
        }
        Commands::BumpEpoch { measurement } => bump_epoch(&measurement),
        Commands::Prune {} => prune(),
        Commands::Remove { older_than } => remove_measurements_from_commits(older_than),
        Commands::ListCommits {} => {
            let commits = list_commits_with_measurements()?;
            for commit in commits {
                println!("{}", commit);
            }
            Ok(())
        }
    }
}

/// Determine the final dispersion method with proper precedence:
/// 1. CLI option (if specified)
/// 2. Configuration file (measurement-specific or global)
/// 3. Default (stddev)
fn determine_dispersion_method(
    cli_method: Option<git_perf_cli_types::DispersionMethod>,
    measurement: &[String],
) -> crate::stats::DispersionMethod {
    if let Some(cli_method) = cli_method {
        // User explicitly specified a dispersion method via CLI
        crate::stats::DispersionMethod::from(cli_method)
    } else {
        // User didn't specify --dispersion-method, try to get from configuration
        if measurement.is_empty() {
            crate::stats::DispersionMethod::StandardDeviation
        } else {
            // Use configuration for the first measurement, or fall back to StandardDeviation
            let config_method = crate::config::audit_dispersion_method(&measurement[0]);
            crate::stats::DispersionMethod::from(config_method)
        }
    }
}
