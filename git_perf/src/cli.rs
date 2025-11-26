use anyhow::Result;
use clap::CommandFactory;
use clap::{error::ErrorKind::ArgumentConflict, Parser};
use env_logger::Env;
use log::Level;

use crate::audit;
use crate::basic_measure::measure;
use crate::config::bump_epoch;
use crate::config_cmd;
use crate::git::git_interop::check_git_version;
use crate::git::git_interop::{list_commits_with_measurements, prune, pull, push};
use crate::import::handle_import;
use crate::measurement_storage::{add, remove_measurements_from_commits};
use crate::reporting::report;
use crate::size;
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
        Commands::Import {
            format,
            file,
            prefix,
            metadata,
            filter,
            dry_run,
            verbose,
        } => handle_import(format, file, prefix, metadata, filter, dry_run, verbose),
        Commands::Push { remote } => push(None, remote.as_deref()),
        Commands::Pull {} => pull(None),
        Commands::Report {
            output,
            separate_by,
            report_history,
            measurement,
            key_value,
            aggregate_by,
            filter,
        } => {
            // Combine measurements (as exact matches) and filter patterns into unified regex patterns
            let combined_patterns =
                crate::filter::combine_measurements_and_filters(&measurement, &filter);

            report(
                output,
                separate_by,
                report_history.max_count,
                &key_value,
                aggregate_by.map(ReductionFunc::from),
                &combined_patterns,
            )
        }
        Commands::Audit {
            measurement,
            report_history,
            selectors,
            min_measurements,
            aggregate_by,
            sigma,
            dispersion_method,
            filter,
        } => {
            // Validate that at least one of measurement or filter is provided
            // (clap's required_unless_present should handle this, but double-check for safety)
            if measurement.is_empty() && filter.is_empty() {
                Cli::command()
                    .error(
                        clap::error::ErrorKind::MissingRequiredArgument,
                        "At least one of --measurement or --filter must be provided",
                    )
                    .exit()
            }

            // Validate max_count vs min_measurements if min_measurements is specified via CLI
            if let Some(min_count) = min_measurements {
                if report_history.max_count < min_count.into() {
                    Cli::command().error(ArgumentConflict, format!("The minimal number of measurements ({}) cannot be more than the maximum number of measurements ({})", min_count, report_history.max_count)).exit()
                }
            }

            // Combine measurements (as exact matches) and filter patterns into unified regex patterns
            let combined_patterns =
                crate::filter::combine_measurements_and_filters(&measurement, &filter);

            audit::audit_multiple(
                report_history.max_count,
                min_measurements,
                &selectors,
                aggregate_by.map(ReductionFunc::from),
                sigma,
                dispersion_method.map(crate::stats::DispersionMethod::from),
                &combined_patterns,
            )
        }
        Commands::BumpEpoch { measurements } => {
            for measurement in measurements {
                bump_epoch(&measurement)?;
            }
            Ok(())
        }
        Commands::Prune {} => prune(),
        Commands::Remove {
            older_than,
            no_prune,
            dry_run,
        } => remove_measurements_from_commits(older_than, !no_prune, dry_run),
        Commands::ListCommits {} => {
            let commits = list_commits_with_measurements()?;
            for commit in commits {
                println!("{}", commit);
            }
            Ok(())
        }
        Commands::Size {
            detailed,
            format,
            disk_size,
            include_objects,
        } => size::calculate_measurement_size(detailed, format, disk_size, include_objects),
        Commands::Config {
            list,
            detailed,
            format,
            validate,
            measurement,
        } => {
            if list {
                config_cmd::list_config(detailed, format, validate, measurement)
            } else {
                // For now, --list is required. In the future, this could support
                // other config operations like --get, --set, etc.
                anyhow::bail!("config command requires --list flag (try: git perf config --list)");
            }
        }
    }
}
