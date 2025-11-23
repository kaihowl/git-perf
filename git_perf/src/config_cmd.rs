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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        hermetic_git_env, with_isolated_home, with_isolated_test_setup, write_gitperfconfig,
    };
    use std::env;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_gather_git_context() {
        with_isolated_test_setup(|_git_dir, _home_path| {
            let context = gather_git_context().unwrap();
            assert_eq!(context.branch, "master");
            assert!(context.repository_root.exists());
        });
    }

    #[test]
    fn test_find_system_config_xdg() {
        hermetic_git_env();
        with_isolated_home(|home_path| {
            // Set XDG_CONFIG_HOME
            let xdg_config_dir = Path::new(home_path).join("xdg_config");
            env::set_var("XDG_CONFIG_HOME", &xdg_config_dir);

            // Create system config
            let system_config_dir = xdg_config_dir.join("git-perf");
            fs::create_dir_all(&system_config_dir).unwrap();
            let system_config_path = system_config_dir.join("config.toml");
            fs::write(&system_config_path, "# test config\n").unwrap();

            let result = find_system_config();
            assert_eq!(result, Some(system_config_path));
        });
    }

    #[test]
    fn test_find_system_config_home_fallback() {
        hermetic_git_env();
        with_isolated_home(|home_path| {
            // Create config in HOME/.config
            let config_dir = Path::new(home_path).join(".config").join("git-perf");
            fs::create_dir_all(&config_dir).unwrap();
            let config_path = config_dir.join("config.toml");
            fs::write(&config_path, "# test config\n").unwrap();

            let result = find_system_config();
            assert_eq!(result, Some(config_path));
        });
    }

    #[test]
    fn test_find_system_config_none() {
        hermetic_git_env();
        with_isolated_home(|_home_path| {
            let result = find_system_config();
            assert_eq!(result, None);
        });
    }

    #[test]
    fn test_get_local_config_path_exists() {
        with_isolated_test_setup(|git_dir, _home_path| {
            write_gitperfconfig(git_dir, "[measurement]\n");

            let result = get_local_config_path();
            // Canonicalize both paths to handle symlinks (e.g., /var -> /private/var on macOS)
            assert_eq!(
                result.map(|p| p.canonicalize().unwrap()),
                Some(git_dir.join(".gitperfconfig").canonicalize().unwrap())
            );
        });
    }

    #[test]
    fn test_get_local_config_path_none() {
        with_isolated_test_setup(|_git_dir, _home_path| {
            let result = get_local_config_path();
            assert_eq!(result, None);
        });
    }

    #[test]
    fn test_gather_config_sources() {
        with_isolated_test_setup(|git_dir, home_path| {
            // Create system config in HOME/.config
            let system_config_dir = Path::new(home_path).join(".config").join("git-perf");
            fs::create_dir_all(&system_config_dir).unwrap();
            let system_config_path = system_config_dir.join("config.toml");
            fs::write(&system_config_path, "# system config\n").unwrap();

            write_gitperfconfig(git_dir, "[measurement]\n");

            let sources = gather_config_sources().unwrap();
            assert_eq!(sources.system_config, Some(system_config_path));
            // Canonicalize both paths to handle symlinks (e.g., /var -> /private/var on macOS)
            assert_eq!(
                sources.local_config.map(|p| p.canonicalize().unwrap()),
                Some(git_dir.join(".gitperfconfig").canonicalize().unwrap())
            );
        });
    }

    #[test]
    fn test_gather_global_settings() {
        with_isolated_test_setup(|_git_dir, _home_path| {
            let settings = gather_global_settings();
            // Default value is 60 seconds
            assert_eq!(settings.backoff_max_elapsed_seconds, 60);
        });
    }

    #[test]
    fn test_extract_measurement_names_empty() {
        with_isolated_test_setup(|_git_dir, _home_path| {
            let config = Config::builder().build().unwrap();
            let names = extract_measurement_names(&config).unwrap();
            assert!(names.is_empty());
        });
    }

    #[test]
    fn test_extract_measurement_names_with_measurements() {
        with_isolated_test_setup(|git_dir, _home_path| {
            write_gitperfconfig(
                git_dir,
                r#"
[measurement.build_time]
epoch = 0x12345678

[measurement.test_time]
epoch = 0x87654321
"#,
            );

            let config = read_hierarchical_config().unwrap();
            let mut names = extract_measurement_names(&config).unwrap();
            names.sort(); // Sort for consistent comparison

            assert_eq!(names, vec!["build_time", "test_time"]);
        });
    }

    #[test]
    fn test_gather_single_measurement_config() {
        with_isolated_test_setup(|git_dir, _home_path| {
            write_gitperfconfig(
                git_dir,
                r#"
[measurement.build_time]
epoch = "12345678"
min_relative_deviation = 5.0
dispersion_method = "mad"
min_measurements = 10
aggregate_by = "median"
sigma = 2.0
unit = "ms"
"#,
            );

            let config = read_hierarchical_config().unwrap();
            let meas_config = gather_single_measurement_config("build_time", &config);

            assert_eq!(meas_config.name, "build_time");
            assert_eq!(meas_config.epoch, Some("12345678".to_string()));
            assert_eq!(meas_config.min_relative_deviation, Some(5.0));
            assert_eq!(meas_config.dispersion_method, "medianabsolutedeviation");
            assert_eq!(meas_config.min_measurements, Some(10));
            assert_eq!(meas_config.aggregate_by, Some("median".to_string()));
            assert_eq!(meas_config.sigma, Some(2.0));
            assert_eq!(meas_config.unit, Some("ms".to_string()));
            assert!(!meas_config.from_parent_fallback);
        });
    }

    #[test]
    fn test_gather_single_measurement_config_parent_fallback() {
        with_isolated_test_setup(|git_dir, _home_path| {
            write_gitperfconfig(
                git_dir,
                r#"
[measurement]
dispersion_method = "stddev"
"#,
            );

            let config = read_hierarchical_config().unwrap();
            let meas_config = gather_single_measurement_config("build_time", &config);

            assert_eq!(meas_config.name, "build_time");
            assert_eq!(meas_config.dispersion_method, "standarddeviation");
            assert!(meas_config.from_parent_fallback);
        });
    }

    #[test]
    fn test_validate_config_valid() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: Some("12345678".to_string()),
                min_relative_deviation: Some(5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(10),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(3.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_config_missing_epoch() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: None,
                min_relative_deviation: Some(5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(10),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(3.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("No epoch configured"));
    }

    #[test]
    fn test_validate_config_invalid_sigma() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: Some("12345678".to_string()),
                min_relative_deviation: Some(5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(10),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(-1.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Invalid sigma value"));
    }

    #[test]
    fn test_validate_config_invalid_min_relative_deviation() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: Some("12345678".to_string()),
                min_relative_deviation: Some(-5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(10),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(3.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Invalid min_relative_deviation"));
    }

    #[test]
    fn test_validate_config_invalid_min_measurements() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: Some("12345678".to_string()),
                min_relative_deviation: Some(5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(1),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(3.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Invalid min_measurements"));
    }

    #[test]
    fn test_validate_config_multiple_issues() {
        let mut measurements = HashMap::new();
        measurements.insert(
            "build_time".to_string(),
            MeasurementConfig {
                name: "build_time".to_string(),
                epoch: None,
                min_relative_deviation: Some(-5.0),
                dispersion_method: "stddev".to_string(),
                min_measurements: Some(1),
                aggregate_by: Some("mean".to_string()),
                sigma: Some(-3.0),
                unit: Some("ms".to_string()),
                from_parent_fallback: false,
            },
        );

        let issues = validate_config(&measurements).unwrap();
        assert_eq!(issues.len(), 4); // epoch, sigma, min_relative_deviation, min_measurements
    }

    #[test]
    fn test_gather_measurement_configs_empty() {
        with_isolated_test_setup(|_git_dir, _home_path| {
            // No config file
            let measurements = gather_measurement_configs(None).unwrap();
            assert!(measurements.is_empty());
        });
    }

    #[test]
    fn test_gather_measurement_configs_with_filter() {
        with_isolated_test_setup(|git_dir, _home_path| {
            write_gitperfconfig(
                git_dir,
                r#"
[measurement.build_time]
epoch = 0x12345678

[measurement.test_time]
epoch = 0x87654321
"#,
            );

            let measurements = gather_measurement_configs(Some("build_time")).unwrap();
            assert_eq!(measurements.len(), 1);
            assert!(measurements.contains_key("build_time"));
            assert!(!measurements.contains_key("test_time"));
        });
    }

    #[test]
    fn test_config_info_serialization() {
        hermetic_git_env();
        with_isolated_home(|home_path| {
            let config_info = ConfigInfo {
                git_context: GitContext {
                    branch: "master".to_string(),
                    repository_root: PathBuf::from(home_path),
                },
                config_sources: ConfigSources {
                    system_config: None,
                    local_config: Some(PathBuf::from(home_path).join(".gitperfconfig")),
                },
                global_settings: GlobalSettings {
                    backoff_max_elapsed_seconds: 60,
                },
                measurements: HashMap::new(),
                validation_issues: None,
            };

            // Test that it serializes to JSON without errors
            let json = serde_json::to_string_pretty(&config_info).unwrap();
            assert!(json.contains("master"));
            assert!(json.contains("backoff_max_elapsed_seconds"));

            // Test that it deserializes back
            let deserialized: ConfigInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized.git_context.branch, "master");
        });
    }

    #[test]
    fn test_display_measurement_human_detailed() {
        let measurement = MeasurementConfig {
            name: "build_time".to_string(),
            epoch: Some("12345678".to_string()),
            min_relative_deviation: Some(5.0),
            dispersion_method: "stddev".to_string(),
            min_measurements: Some(10),
            aggregate_by: Some("mean".to_string()),
            sigma: Some(3.0),
            unit: Some("ms".to_string()),
            from_parent_fallback: false,
        };

        // This test just ensures the function doesn't panic
        display_measurement_human(&measurement, true);
    }

    #[test]
    fn test_display_measurement_human_summary() {
        let measurement = MeasurementConfig {
            name: "build_time".to_string(),
            epoch: Some("12345678".to_string()),
            min_relative_deviation: Some(5.0),
            dispersion_method: "stddev".to_string(),
            min_measurements: Some(10),
            aggregate_by: Some("mean".to_string()),
            sigma: Some(3.0),
            unit: Some("ms".to_string()),
            from_parent_fallback: false,
        };

        // This test just ensures the function doesn't panic
        display_measurement_human(&measurement, false);
    }
}
