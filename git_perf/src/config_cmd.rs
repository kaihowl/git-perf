use crate::config::{
    audit_aggregate_by, audit_dispersion_method, audit_min_measurements,
    audit_min_relative_deviation, audit_sigma, backoff_max_elapsed_seconds,
    determine_epoch_from_config, measurement_unit, read_hierarchical_config,
};
use crate::git::git_interop::get_repository_root;
use anyhow::{Context, Result};
use config::Config;
use git_perf_cli_types::ConfigFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Complete configuration information
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigInfo {
    /// Git context information
    pub git_context: GitContext,

    /// Configuration sources being used
    pub config_sources: ConfigSources,

    /// Global settings (not measurement-specific)
    pub global_settings: GlobalSettings,

    /// Measurement-specific configurations
    pub measurements: HashMap<String, MeasurementConfig>,

    /// Validation issues (if validation was requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_issues: Option<Vec<String>>,
}

/// Git repository context
#[derive(Debug, Serialize, Deserialize)]
pub struct GitContext {
    /// Current branch name
    pub branch: String,

    /// Repository root path
    pub repository_root: PathBuf,
}

/// Configuration file sources
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSources {
    /// System-wide config path (if exists)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_config: Option<PathBuf>,

    /// Local repository config path (if exists)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_config: Option<PathBuf>,
}

/// Global configuration settings
#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Backoff max elapsed seconds
    pub backoff_max_elapsed_seconds: u64,
}

/// Configuration for a specific measurement
#[derive(Debug, Serialize, Deserialize)]
pub struct MeasurementConfig {
    /// Measurement name
    pub name: String,

    /// Epoch (8-char hex string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<String>,

    /// Minimum relative deviation threshold (%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_relative_deviation: Option<f64>,

    /// Dispersion method (stddev or mad)
    pub dispersion_method: String,

    /// Minimum measurements required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_measurements: Option<u16>,

    /// Aggregation function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_by: Option<String>,

    /// Sigma threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sigma: Option<f64>,

    /// Measurement unit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Whether this is from parent table fallback (vs measurement-specific)
    pub from_parent_fallback: bool,
}

/// Display configuration information (implements config --list)
pub fn list_config(
    detailed: bool,
    format: ConfigFormat,
    validate: bool,
    measurement_filter: Option<String>,
) -> Result<()> {
    // 1. Gather configuration information
    let config_info = gather_config_info(validate, measurement_filter.as_deref())?;

    // 2. Display based on format
    match format {
        ConfigFormat::Human => display_human_readable(&config_info, detailed)?,
        ConfigFormat::Json => display_json(&config_info)?,
    }

    // 3. Exit with error if validation found issues
    if validate {
        if let Some(ref issues) = config_info.validation_issues {
            if !issues.is_empty() {
                return Err(anyhow::anyhow!(
                    "Configuration validation found {} issue(s)",
                    issues.len()
                ));
            }
        }
    }

    Ok(())
}

/// Gather all configuration information
fn gather_config_info(validate: bool, measurement_filter: Option<&str>) -> Result<ConfigInfo> {
    let git_context = gather_git_context()?;
    let config_sources = gather_config_sources()?;
    let global_settings = gather_global_settings();
    let measurements = gather_measurement_configs(measurement_filter)?;

    let validation_issues = if validate {
        Some(validate_config(&measurements)?)
    } else {
        None
    };

    Ok(ConfigInfo {
        git_context,
        config_sources,
        global_settings,
        measurements,
        validation_issues,
    })
}

/// Get git context information
fn gather_git_context() -> Result<GitContext> {
    // Get current branch name
    let branch_output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("Failed to get current branch")?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get repository root
    let repo_root = get_repository_root()
        .map_err(|e| anyhow::anyhow!("Failed to get repository root: {}", e))?;
    let repository_root = PathBuf::from(repo_root);

    Ok(GitContext {
        branch,
        repository_root,
    })
}

/// Determine which config files are being used
fn gather_config_sources() -> Result<ConfigSources> {
    // System config
    let system_config = find_system_config();

    // Local config - get repository config path
    let local_config = get_local_config_path();

    Ok(ConfigSources {
        system_config,
        local_config,
    })
}

/// Find system config path if it exists
fn find_system_config() -> Option<PathBuf> {
    use std::env;

    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_config_home)
            .join("git-perf")
            .join("config.toml");
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(home) = dirs_next::home_dir() {
        let path = home.join(".config").join("git-perf").join("config.toml");
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Get the local repository config path (if it exists)
fn get_local_config_path() -> Option<PathBuf> {
    let repo_root = get_repository_root().ok()?;
    let path = PathBuf::from(repo_root).join(".gitperfconfig");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Gather global (non-measurement) settings
fn gather_global_settings() -> GlobalSettings {
    GlobalSettings {
        backoff_max_elapsed_seconds: backoff_max_elapsed_seconds(),
    }
}

/// Gather measurement configurations
fn gather_measurement_configs(
    measurement_filter: Option<&str>,
) -> Result<HashMap<String, MeasurementConfig>> {
    let mut measurements = HashMap::new();

    // Get hierarchical config
    let config = match read_hierarchical_config() {
        Ok(c) => c,
        Err(_) => {
            // No config found, return empty map
            return Ok(measurements);
        }
    };

    // Extract all measurement names from config
    let measurement_names = extract_measurement_names(&config)?;

    // Filter if requested
    let filtered_names: Vec<String> = if let Some(filter) = measurement_filter {
        measurement_names
            .into_iter()
            .filter(|name| name == filter)
            .collect()
    } else {
        measurement_names
    };

    // Gather config for each measurement
    for name in filtered_names {
        let measurement_config = gather_single_measurement_config(&name, &config);
        measurements.insert(name.clone(), measurement_config);
    }

    Ok(measurements)
}

/// Extract measurement names from config
fn extract_measurement_names(config: &Config) -> Result<Vec<String>> {
    let mut names = Vec::new();

    // Try to get the measurement table
    if let Ok(table) = config.get_table("measurement") {
        for (key, value) in table {
            // Skip non-table values (these are parent defaults)
            if matches!(value.kind, config::ValueKind::Table(_)) {
                names.push(key);
            }
        }
    }

    Ok(names)
}

/// Gather configuration for a single measurement
fn gather_single_measurement_config(name: &str, config: &Config) -> MeasurementConfig {
    // Check if settings are measurement-specific or from parent fallback
    let has_specific_config = config.get_table(&format!("measurement.{}", name)).is_ok();

    MeasurementConfig {
        name: name.to_string(),
        epoch: determine_epoch_from_config(name).map(|e| format!("{:08x}", e)),
        min_relative_deviation: audit_min_relative_deviation(name),
        dispersion_method: format!("{:?}", audit_dispersion_method(name)).to_lowercase(),
        min_measurements: audit_min_measurements(name),
        aggregate_by: audit_aggregate_by(name).map(|f| format!("{:?}", f).to_lowercase()),
        sigma: audit_sigma(name),
        unit: measurement_unit(name),
        from_parent_fallback: !has_specific_config,
    }
}

/// Validate configuration
fn validate_config(measurements: &HashMap<String, MeasurementConfig>) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    for (name, config) in measurements {
        // Check for missing epoch
        if config.epoch.is_none() {
            issues.push(format!(
                "Measurement '{}': No epoch configured (run 'git perf bump-epoch -m {}')",
                name, name
            ));
        }

        // Check for invalid sigma values
        if let Some(sigma) = config.sigma {
            if sigma <= 0.0 {
                issues.push(format!(
                    "Measurement '{}': Invalid sigma value {} (must be positive)",
                    name, sigma
                ));
            }
        }

        // Check for invalid min_relative_deviation
        if let Some(deviation) = config.min_relative_deviation {
            if deviation < 0.0 {
                issues.push(format!(
                    "Measurement '{}': Invalid min_relative_deviation {} (must be non-negative)",
                    name, deviation
                ));
            }
        }

        // Check for invalid min_measurements
        if let Some(min_meas) = config.min_measurements {
            if min_meas < 2 {
                issues.push(format!(
                    "Measurement '{}': Invalid min_measurements {} (must be at least 2)",
                    name, min_meas
                ));
            }
        }
    }

    Ok(issues)
}

/// Display configuration in human-readable format
fn display_human_readable(info: &ConfigInfo, detailed: bool) -> Result<()> {
    println!("Git-Perf Configuration");
    println!("======================");
    println!();

    // Git Context
    println!("Git Context:");
    println!("  Branch: {}", info.git_context.branch);
    println!(
        "  Repository: {}",
        info.git_context.repository_root.display()
    );
    println!();

    // Configuration Sources
    println!("Configuration Sources:");
    if let Some(ref system_path) = info.config_sources.system_config {
        println!("  System config: {}", system_path.display());
    } else {
        println!("  System config: (none)");
    }
    if let Some(ref local_path) = info.config_sources.local_config {
        println!("  Local config:  {}", local_path.display());
    } else {
        println!("  Local config:  (none)");
    }
    println!();

    // Global Settings
    println!("Global Settings:");
    println!(
        "  backoff.max_elapsed_seconds: {}",
        info.global_settings.backoff_max_elapsed_seconds
    );
    println!();

    // Measurements
    if info.measurements.is_empty() {
        println!("Measurements: (none configured)");
    } else {
        println!("Measurements: ({} configured)", info.measurements.len());
        println!();

        let mut sorted_measurements: Vec<_> = info.measurements.values().collect();
        sorted_measurements.sort_by_key(|m| &m.name);

        for measurement in sorted_measurements {
            display_measurement_human(measurement, detailed);
        }
    }

    // Validation Issues
    if let Some(ref issues) = info.validation_issues {
        if !issues.is_empty() {
            println!();
            println!("Validation Issues:");
            for issue in issues {
                println!("  \u{26A0} {}", issue);
            }
        } else {
            println!();
            println!("\u{2713} Configuration is valid");
        }
    }

    Ok(())
}

/// Display a single measurement configuration
fn display_measurement_human(measurement: &MeasurementConfig, detailed: bool) {
    if detailed {
        println!("  [{}]", measurement.name);
        if measurement.from_parent_fallback {
            println!("    (using parent table defaults)");
        }
        println!("    epoch:                  {:?}", measurement.epoch);
        println!(
            "    min_relative_deviation: {:?}",
            measurement.min_relative_deviation
        );
        println!(
            "    dispersion_method:      {}",
            measurement.dispersion_method
        );
        println!(
            "    min_measurements:       {:?}",
            measurement.min_measurements
        );
        println!("    aggregate_by:           {:?}", measurement.aggregate_by);
        println!("    sigma:                  {:?}", measurement.sigma);
        println!("    unit:                   {:?}", measurement.unit);
        println!();
    } else {
        // Summary view - just name and epoch
        let epoch_display = measurement.epoch.as_deref().unwrap_or("(not set)");
        let unit_display = measurement.unit.as_deref().unwrap_or("(not set)");
        println!(
            "  {} - epoch: {}, unit: {}",
            measurement.name, epoch_display, unit_display
        );
    }
}

/// Display configuration as JSON
fn display_json(info: &ConfigInfo) -> Result<()> {
    let json =
        serde_json::to_string_pretty(info).context("Failed to serialize configuration to JSON")?;
    println!("{}", json);
    Ok(())
}
