use anyhow::Result;
use clap::CommandFactory;
use clap::{error::ErrorKind::ArgumentConflict, Parser};
use env_logger::Env;
use log::Level;

use crate::audit;
use crate::basic_measure::measure;
use crate::config::bump_epoch;
use crate::git::git_interop::check_git_version;
use crate::git::git_interop::{get_repository_root, prune, pull, push};
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
        Commands::Push {} => push(None),
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
        Commands::Config {} => show_config_info(),
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

/// Show configuration information including branch name and config paths
fn show_config_info() -> Result<()> {
    use std::path::Path;

    println!("Git Performance Configuration Information");
    println!("=======================================");

    // Get current branch
    let current_branch = get_current_branch().unwrap_or_else(|_| "unknown".to_string());
    println!("Current branch: {}", current_branch);

    // Get repository root
    match get_repository_root() {
        Ok(repo_root) => {
            println!("Repository root: {}", repo_root);

            // Check for config file
            let config_path = Path::new(&repo_root).join(".gitperfconfig");
            if config_path.exists() {
                println!("Config file: {} (exists)", config_path.display());
            } else {
                println!("Config file: {} (not found)", config_path.display());
            }
        }
        Err(e) => {
            println!("Repository root: Error - {}", e);
        }
    }

    // Try to load and display config
    match crate::config::read_hierarchical_config() {
        Ok(config) => {
            println!("\nConfiguration loaded successfully");

            // Show some key config values if they exist
            if let Ok(min_rel_dev) = config.get_string("measurement.min_relative_deviation") {
                println!("  Default min_relative_deviation: {}", min_rel_dev);
            }
            if let Ok(dispersion) = config.get_string("measurement.dispersion_method") {
                println!("  Default dispersion_method: {}", dispersion);
            }
        }
        Err(e) => {
            println!("\nConfiguration: Error loading - {}", e);
        }
    }

    Ok(())
}

/// Get the current branch name
fn get_current_branch() -> Result<String> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git command: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
