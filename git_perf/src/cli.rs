use anyhow::Result;
use clap::CommandFactory;
use clap::{error::ErrorKind::ArgumentConflict, Parser};
use env_logger::Env;
use log::Level;

use crate::audit;
use crate::basic_measure::measure;
use crate::config::bump_epoch;
use crate::git_interop;
use crate::git_interop::{prune, pull, push};
use crate::measurement_storage::{add, remove_measurements_from_commits};
use crate::reporting::report;
use cli_types::{Cli, Commands};

pub fn handle_calls() -> Result<()> {
    let cli = Cli::parse();
    let logger_level = match cli.verbose {
        0 => Level::Warn,
        1 => Level::Info,
        2 => Level::Debug,
        3 | _ => Level::Trace,
    };
    env_logger::Builder::from_env(Env::default().default_filter_or(logger_level.as_str())).init();

    git_interop::check_git_version()?;

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
            aggregate_by,
        } => Ok(report(
            output,
            separate_by,
            report_history.max_count,
            &measurement,
            &key_value,
            aggregate_by,
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
        Commands::Remove { older_than } => remove_measurements_from_commits(older_than),
    }
}
