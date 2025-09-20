use anyhow::Result;
use config::{Config, ConfigError, File, FileFormat};
use std::{
    env,
    fs::{self, File as StdFile},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toml_edit::{value, Document};

use crate::git::git_interop::get_head_revision;

// Import the CLI types for dispersion method
use git_perf_cli_types::DispersionMethod;

/// Get the main repository config path (always in repo root)
fn get_main_config_path() -> PathBuf {
    if let Ok(mut current_dir) = env::current_dir() {
        loop {
            let candidate = current_dir.join(".gitperfconfig");
            if candidate.is_file() {
                return candidate;
            }
            // Check if we're in a git repository
            if current_dir.join(".git").is_dir() {
                return candidate;
            }
            if !current_dir.pop() {
                break;
            }
        }
    }
    // Fallback to current directory
    PathBuf::from(".gitperfconfig")
}

/// Write config to the main repository directory (always in repo root)
pub fn write_config(conf: &str) -> Result<()> {
    let path = get_main_config_path();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut f = StdFile::create(path)?;
    f.write_all(conf.as_bytes())?;
    Ok(())
}

/// Read hierarchical configuration (system -> local override)
pub fn read_hierarchical_config() -> Result<Config, ConfigError> {
    let mut builder = Config::builder();

    // 1. System-wide config (XDG_CONFIG_HOME or ~/.config/git-perf/config.toml)
    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        let system_config_path = Path::new(&xdg_config_home)
            .join("git-perf")
            .join("config.toml");
        if system_config_path.is_file() {
            builder = builder.add_source(File::from(system_config_path).format(FileFormat::Toml));
        }
    } else if let Some(home) = dirs_next::home_dir() {
        let system_config_path = home.join(".config").join("git-perf").join("config.toml");
        if system_config_path.is_file() {
            builder = builder.add_source(File::from(system_config_path).format(FileFormat::Toml));
        }
    }

    // 2. Local config (repository .gitperfconfig) - this overrides system config
    if let Some(local_path) = find_config_path() {
        if local_path.is_file() {
            builder = builder.add_source(File::from(local_path).format(FileFormat::Toml));
        }
    }

    builder.build()
}

/// Read config as string (for backward compatibility)
pub fn read_config() -> Result<String> {
    if let Some(path) = find_config_path() {
        return read_config_from_file(path);
    }
    read_config_from_file(".gitperfconfig")
}

fn find_config_path() -> Option<PathBuf> {
    if let Ok(mut current_dir) = env::current_dir() {
        loop {
            let candidate = current_dir.join(".gitperfconfig");
            if candidate.is_file() {
                return Some(candidate);
            }
            if !current_dir.pop() {
                break;
            }
        }
    }

    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        let candidate = Path::new(&xdg_config_home)
            .join("git-perf")
            .join("config.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(home) = dirs_next::home_dir() {
        let candidate = home.join(".config").join("git-perf").join("config.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn read_config_from_file<P: AsRef<Path>>(file: P) -> Result<String> {
    let mut conf_str = String::new();
    StdFile::open(file)?.read_to_string(&mut conf_str)?;
    Ok(conf_str)
}

pub fn determine_epoch_from_config(measurement: &str) -> Option<u32> {
    // Try hierarchical config first
    if let Ok(config) = read_hierarchical_config() {
        if let Ok(epoch_str) = config.get_string(&format!("measurement.{}.epoch", measurement)) {
            if let Ok(epoch) = u32::from_str_radix(&epoch_str, 16) {
                return Some(epoch);
            }
        }
        // Try wildcard fallback
        if let Ok(epoch_str) = config.get_string("measurement.*.epoch") {
            if let Ok(epoch) = u32::from_str_radix(&epoch_str, 16) {
                return Some(epoch);
            }
        }
    }

    // Fallback to old method for backward compatibility
    let conf = match read_config() {
        Ok(conf) => conf,
        Err(e) => {
            // Log the error but don't fail - this is expected when no config exists
            log::debug!("Could not read config file: {}", e);
            return None;
        }
    };
    determine_epoch(measurement, &conf)
}

fn determine_epoch(measurement: &str, conf_str: &str) -> Option<u32> {
    let config = match conf_str.parse::<Document>() {
        Ok(config) => config,
        Err(e) => {
            log::debug!("Failed to parse config: {}", e);
            return None;
        }
    };

    let get_epoch = |section: &str| {
        let s = config
            .get("measurement")?
            .get(section)?
            .get("epoch")?
            .as_str()?;
        u32::from_str_radix(s, 16).ok()
    };

    get_epoch(measurement).or_else(|| get_epoch("*"))
}

pub fn bump_epoch_in_conf(measurement: &str, conf_str: &mut String) -> Result<()> {
    let mut conf = conf_str
        .parse::<Document>()
        .expect("failed to parse config");

    let head_revision = get_head_revision()?;
    // TODO(kaihowl) ensure that always non-inline tables are written in an empty config file
    conf["measurement"][measurement]["epoch"] = value(&head_revision[0..8]);
    *conf_str = conf.to_string();

    Ok(())
}

pub fn bump_epoch(measurement: &str) -> Result<()> {
    let mut conf_str = read_config().unwrap_or_default();
    bump_epoch_in_conf(measurement, &mut conf_str)?;
    write_config(&conf_str)?;
    Ok(())
}

/// Returns the backoff max elapsed seconds from a config string, or 60 if not set.
pub fn backoff_max_elapsed_seconds_from_str(conf: &str) -> u64 {
    let doc = conf.parse::<Document>().ok();
    doc.and_then(|doc| {
        doc.get("backoff")
            .and_then(|b| b.get("max_elapsed_seconds"))
            .and_then(|v| v.as_integer())
            .map(|v| v as u64)
    })
    .unwrap_or(60)
}

/// Returns the backoff max elapsed seconds from config, or 60 if not set.
pub fn backoff_max_elapsed_seconds() -> u64 {
    // Try hierarchical config first
    if let Ok(config) = read_hierarchical_config() {
        if let Ok(seconds) = config.get_int("backoff.max_elapsed_seconds") {
            return seconds as u64;
        }
    }

    // Fallback to old method
    backoff_max_elapsed_seconds_from_str(read_config().unwrap_or_default().as_str())
}

/// Returns the minimum relative deviation threshold from a config string, or None if not set.
/// Follows precedence: measurement-specific > global > None
pub fn audit_min_relative_deviation_from_str(conf: &str, measurement: &str) -> Option<f64> {
    let doc = conf.parse::<Document>().ok()?;

    // Check for measurement-specific setting first
    if let Some(threshold) = doc
        .get("audit")
        .and_then(|audit| audit.get("measurement"))
        .and_then(|measurement_section| measurement_section.get(measurement))
        .and_then(|config| config.get("min_relative_deviation"))
        .and_then(|threshold| threshold.as_float())
    {
        return Some(threshold);
    }

    // Check for global setting
    if let Some(threshold) = doc
        .get("audit")
        .and_then(|audit| audit.get("global"))
        .and_then(|global| global.get("min_relative_deviation"))
        .and_then(|threshold| threshold.as_float())
    {
        return Some(threshold);
    }

    None
}

/// Returns the minimum relative deviation threshold from config, or None if not set.
pub fn audit_min_relative_deviation(measurement: &str) -> Option<f64> {
    // Try hierarchical config first
    if let Ok(config) = read_hierarchical_config() {
        // Check measurement-specific setting first
        if let Ok(threshold) = config.get_float(&format!(
            "audit.measurement.{}.min_relative_deviation",
            measurement
        )) {
            return Some(threshold);
        }
        // Check global setting
        if let Ok(threshold) = config.get_float("audit.global.min_relative_deviation") {
            return Some(threshold);
        }
    }

    // Fallback to old method
    audit_min_relative_deviation_from_str(read_config().unwrap_or_default().as_str(), measurement)
}

/// Returns the dispersion method from a config string, or StandardDeviation if not set.
/// Follows precedence: measurement-specific > global > StandardDeviation
pub fn audit_dispersion_method_from_str(conf: &str, measurement: &str) -> DispersionMethod {
    let doc = match conf.parse::<Document>() {
        Ok(doc) => doc,
        Err(_) => return DispersionMethod::StandardDeviation,
    };

    // Check for measurement-specific setting first
    if let Some(method_str) = doc
        .get("audit")
        .and_then(|audit| audit.get("measurement"))
        .and_then(|measurement_section| measurement_section.get(measurement))
        .and_then(|config| config.get("dispersion_method"))
        .and_then(|method| method.as_str())
    {
        if let Ok(method) = method_str.parse::<DispersionMethod>() {
            return method;
        }
    }

    // Check for global setting
    if let Some(method_str) = doc
        .get("audit")
        .and_then(|audit| audit.get("global"))
        .and_then(|global| global.get("dispersion_method"))
        .and_then(|method| method.as_str())
    {
        if let Ok(method) = method_str.parse::<DispersionMethod>() {
            return method;
        }
    }

    // Default to StandardDeviation
    DispersionMethod::StandardDeviation
}

/// Returns the dispersion method from config, or StandardDeviation if not set.
pub fn audit_dispersion_method(measurement: &str) -> DispersionMethod {
    // Try hierarchical config first
    if let Ok(config) = read_hierarchical_config() {
        // Check measurement-specific setting first
        if let Ok(method_str) = config.get_string(&format!(
            "audit.measurement.{}.dispersion_method",
            measurement
        )) {
            if let Ok(method) = method_str.parse::<DispersionMethod>() {
                return method;
            }
        }
        // Check global setting
        if let Ok(method_str) = config.get_string("audit.global.dispersion_method") {
            if let Ok(method) = method_str.parse::<DispersionMethod>() {
                return method;
            }
        }
    }

    // Fallback to old method
    audit_dispersion_method_from_str(read_config().unwrap_or_default().as_str(), measurement)
}

#[cfg(test)]
mod test {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_epochs() {
        // TODO(hoewelmk) order unspecified in serialization...
        let configfile = r#"[measurement."something"]
#My comment
epoch="34567898"

[measurement."somethingelse"]
epoch="a3dead"

[measurement."*"]
# General performance regression
epoch="12344555"
"#;

        let epoch = determine_epoch("something", configfile);
        assert_eq!(epoch, Some(0x34567898));

        let epoch = determine_epoch("somethingelse", configfile);
        assert_eq!(epoch, Some(0xa3dead));

        let epoch = determine_epoch("unspecified", configfile);
        assert_eq!(epoch, Some(0x12344555));
    }

    #[test]
    #[serial]
    fn test_bump_epochs() {
        // Create a temporary git repository for this test
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Initialize git repository
        std::process::Command::new("git")
            .args(&["init", "--initial-branch=master"])
            .output()
            .expect("Failed to initialize git repository");

        // Set up hermetic git environment
        env::set_var("GIT_CONFIG_NOSYSTEM", "true");
        env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        env::set_var("GIT_AUTHOR_NAME", "testuser");
        env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
        env::set_var("GIT_COMMITTER_NAME", "testuser");
        env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");

        // Create a commit to have a HEAD
        fs::write("test.txt", "test content").unwrap();
        std::process::Command::new("git")
            .args(&["add", "test.txt"])
            .output()
            .expect("Failed to add file");
        let commit_output = std::process::Command::new("git")
            .args(&["commit", "-m", "test commit"])
            .output()
            .expect("Failed to commit");

        // Verify commit was successful
        if !commit_output.status.success() {
            panic!(
                "Git commit failed: {}",
                String::from_utf8_lossy(&commit_output.stderr)
            );
        }

        let configfile = r#"[measurement."something"]
#My comment
epoch = "34567898"
"#;

        let mut actual = String::from(configfile);
        bump_epoch_in_conf("something", &mut actual).expect("Failed to bump epoch");

        let expected = format!(
            r#"[measurement."something"]
#My comment
epoch = "{}"
"#,
            &get_head_revision().expect("get_head_revision failed")[0..8],
        );

        assert_eq!(actual, expected);

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_bump_new_epoch_and_read_it() {
        // Create a temporary git repository for this test
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Initialize git repository
        std::process::Command::new("git")
            .args(&["init", "--initial-branch=master"])
            .output()
            .expect("Failed to initialize git repository");

        // Set up hermetic git environment
        env::set_var("GIT_CONFIG_NOSYSTEM", "true");
        env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        env::set_var("GIT_AUTHOR_NAME", "testuser");
        env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
        env::set_var("GIT_COMMITTER_NAME", "testuser");
        env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");

        // Create a commit to have a HEAD
        fs::write("test.txt", "test content").unwrap();
        std::process::Command::new("git")
            .args(&["add", "test.txt"])
            .output()
            .expect("Failed to add file");
        let commit_output = std::process::Command::new("git")
            .args(&["commit", "-m", "test commit"])
            .output()
            .expect("Failed to commit");

        // Verify commit was successful
        if !commit_output.status.success() {
            panic!(
                "Git commit failed: {}",
                String::from_utf8_lossy(&commit_output.stderr)
            );
        }

        let mut conf = String::new();
        bump_epoch_in_conf("mymeasurement", &mut conf).expect("Failed to bump epoch");
        let epoch = determine_epoch("mymeasurement", &conf);
        assert!(epoch.is_some());

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    fn test_parsing() {
        let toml_str = r#"
        measurement = { test2 = { epoch = "834ae670e2ecd5c87020fde23378b890832d6076" } }
    "#;

        let doc = toml_str.parse::<Document>().expect("sfdfdf");

        let measurement = "test";

        if let Some(e) = doc
            .get("measurement")
            .and_then(|m| m.get(measurement))
            .and_then(|m| m.get("epoch"))
        {
            println!("YAY: {}", e);
            panic!("stuff");
        }
    }

    #[test]
    fn test_backoff_max_elapsed_seconds() {
        // Case 1: config string with explicit value
        let configfile = "[backoff]\nmax_elapsed_seconds = 42\n";
        assert_eq!(super::backoff_max_elapsed_seconds_from_str(configfile), 42);

        // Case 2: config string missing value
        let configfile = "";
        assert_eq!(super::backoff_max_elapsed_seconds_from_str(configfile), 60);
    }

    #[test]
    fn test_audit_min_relative_deviation() {
        // Case 1: measurement-specific setting
        let configfile = r#"
[audit.measurement."build_time"]
min_relative_deviation = 10.0

[audit.measurement."memory_usage"]
min_relative_deviation = 2.5
"#;
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "build_time"),
            Some(10.0)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "memory_usage"),
            Some(2.5)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "other_measurement"),
            None
        );

        // Case 2: global setting
        let configfile = r#"
[audit.global]
min_relative_deviation = 5.0
"#;
        println!("Testing Case 2: global setting");
        let result = super::audit_min_relative_deviation_from_str(configfile, "any_measurement");
        println!("Case 2 result: {:?}", result);
        assert_eq!(result, Some(5.0));

        // Case 3: precedence - measurement-specific overrides global
        let configfile = r#"
[audit.global]
min_relative_deviation = 5.0

[audit.measurement."build_time"]
min_relative_deviation = 10.0
"#;
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "build_time"),
            Some(10.0)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "other_measurement"),
            Some(5.0)
        );

        // Case 5: no audit configuration
        let configfile = "";
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "any_measurement"),
            None
        );

        // Case 6: invalid config (should return None)
        let configfile = "[audit]\nmin_relative_deviation = invalid\n";
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "any_measurement"),
            None
        );
    }

    #[test]
    fn test_audit_dispersion_method() {
        // Case 1: measurement-specific setting
        let configfile = r#"
[audit.measurement."build_time"]
dispersion_method = "mad"

[audit.measurement."memory_usage"]
dispersion_method = "stddev"
"#;
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "build_time"),
            git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
        );
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "memory_usage"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "other_measurement"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );

        // Case 2: global setting
        let configfile = r#"
[audit.global]
dispersion_method = "mad"
"#;
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "any_measurement"),
            git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
        );

        // Case 3: precedence - measurement-specific overrides global
        let configfile = r#"
[audit.global]
dispersion_method = "mad"

[audit.measurement."build_time"]
dispersion_method = "stddev"
"#;
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "build_time"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "other_measurement"),
            git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
        );

        // Case 4: no audit configuration (should return StandardDeviation)
        let configfile = "";
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "any_measurement"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );

        // Case 5: invalid config (should return StandardDeviation)
        let configfile = "[audit]\ndispersion_method = invalid\n";
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "any_measurement"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );

        // Case 6: malformed TOML (should return StandardDeviation)
        let configfile = "[audit\n";
        assert_eq!(
            super::audit_dispersion_method_from_str(configfile, "any_measurement"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );
    }

    #[test]
    fn test_find_config_path_upward_search() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        // Create config in parent directory
        let config_path = temp_dir.path().join(".gitperfconfig");
        fs::write(
            &config_path,
            "[measurement.\"test\"]\nepoch = \"12345678\"\n",
        )
        .unwrap();

        // Change to subdirectory and test upward search
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&subdir).unwrap();

        let found_path = find_config_path();
        assert!(found_path.is_some());
        assert_eq!(found_path.unwrap(), config_path);

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_find_config_path_xdg_config_home() {
        let temp_dir = TempDir::new().unwrap();
        let xdg_config_dir = temp_dir.path().join("git-perf");
        fs::create_dir_all(&xdg_config_dir).unwrap();

        let config_path = xdg_config_dir.join("config.toml");
        fs::write(
            &config_path,
            "[measurement.\"test\"]\nepoch = \"12345678\"\n",
        )
        .unwrap();

        // Change to a clean directory to avoid finding workspace .gitperfconfig
        let original_dir = env::current_dir().unwrap();
        let clean_dir = temp_dir.path().join("clean");
        fs::create_dir_all(&clean_dir).unwrap();
        env::set_current_dir(&clean_dir).unwrap();

        // Set XDG_CONFIG_HOME and test
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        let found_path = find_config_path();
        assert!(found_path.is_some());
        assert_eq!(found_path.unwrap(), config_path);

        // Clean up
        env::remove_var("XDG_CONFIG_HOME");
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            // Ensure original directory still exists before changing back
            if original_dir.exists() {
                env::set_current_dir(original_dir).unwrap();
            }
        }
    }

    #[test]
    #[serial]
    fn test_find_config_path_home_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let home_config_dir = temp_dir.path().join(".config").join("git-perf");
        fs::create_dir_all(&home_config_dir).unwrap();

        let config_path = home_config_dir.join("config.toml");
        fs::write(
            &config_path,
            "[measurement.\"test\"]\nepoch = \"12345678\"\n",
        )
        .unwrap();

        // Change to a clean directory to avoid finding workspace .gitperfconfig
        let original_dir = env::current_dir().unwrap();
        let clean_dir = temp_dir.path().join("clean");
        fs::create_dir_all(&clean_dir).unwrap();
        env::set_current_dir(&clean_dir).unwrap();

        // Mock home directory by setting both HOME and XDG_CONFIG_HOME to empty
        let original_home = env::var("HOME").ok();
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        env::set_var("HOME", temp_dir.path());
        env::remove_var("XDG_CONFIG_HOME");

        let found_path = find_config_path();
        assert!(found_path.is_some());
        assert_eq!(found_path.unwrap(), config_path);

        // Restore original environment
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
        if let Some(xdg) = original_xdg {
            env::set_var("XDG_CONFIG_HOME", xdg);
        }
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_find_config_path_not_found() {
        // Ensure no config exists in current directory or XDG locations
        let original_dir = env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("HOME");

        let found_path = find_config_path();
        assert!(found_path.is_none());

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_determine_epoch_from_config_with_missing_file() {
        // Test that missing config file doesn't panic and returns None
        let original_dir = env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let epoch = determine_epoch_from_config("test_measurement");
        assert!(epoch.is_none());

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_determine_epoch_from_config_with_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".gitperfconfig");
        fs::write(&config_path, "invalid toml content").unwrap();

        let original_dir = env::current_dir().unwrap();
        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let epoch = determine_epoch_from_config("test_measurement");
        assert!(epoch.is_none());

        // Restore original directory
        // Ensure original directory still exists before changing back
        if original_dir.exists() {
            env::set_current_dir(original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_write_config_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("a").join("b").join("c");
        let config_path = nested_dir.join(".gitperfconfig");

        // Store original directory before any changes
        let original_dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(_) => {
                // If current directory is invalid, use a safe fallback
                std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| "/tmp".into())
            }
        };

        fs::create_dir_all(&nested_dir).unwrap();
        env::set_current_dir(&nested_dir).unwrap();

        let config_content = "[measurement.\"test\"]\nepoch = \"12345678\"\n";
        write_config(config_content).unwrap();

        assert!(config_path.is_file());
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, config_content);

        // Restore original directory - keep temp_dir alive until after we change back
        let _keep_temp_dir = temp_dir;
        if original_dir.exists() {
            let _ = env::set_current_dir(&original_dir);
        }
    }

    #[test]
    #[serial]
    fn test_hierarchical_config_system_override() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Create system config directory
        let system_config_dir = temp_dir.path().join(".config").join("git-perf");
        fs::create_dir_all(&system_config_dir).unwrap();
        let system_config_path = system_config_dir.join("config.toml");

        // Create system config
        let system_config = r#"
[audit.global]
min_relative_deviation = 5.0
dispersion_method = "mad"

[backoff]
max_elapsed_seconds = 120
"#;
        fs::write(&system_config_path, system_config).unwrap();

        // Create local config that overrides system config
        let local_config_dir = temp_dir.path().join("repo");
        fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_path = local_config_dir.join(".gitperfconfig");

        let local_config = r#"
[audit.global]
min_relative_deviation = 10.0

[audit.measurement."build_time"]
min_relative_deviation = 15.0
dispersion_method = "stddev"
"#;
        fs::write(&local_config_path, local_config).unwrap();

        // Set up environment
        env::set_var("HOME", temp_dir.path());
        env::remove_var("XDG_CONFIG_HOME");
        env::set_current_dir(&local_config_dir).unwrap();

        // Test hierarchical config reading
        let config = read_hierarchical_config().unwrap();

        // Test that local config overrides system config
        assert_eq!(
            config
                .get_float("audit.global.min_relative_deviation")
                .unwrap(),
            10.0
        );
        assert_eq!(
            config.get_string("audit.global.dispersion_method").unwrap(),
            "mad"
        ); // Not overridden in local

        // Test measurement-specific override
        assert_eq!(
            config
                .get_float("audit.measurement.build_time.min_relative_deviation")
                .unwrap(),
            15.0
        );
        assert_eq!(
            config
                .get_string("audit.measurement.build_time.dispersion_method")
                .unwrap(),
            "stddev"
        );

        // Test that system config is still available for non-overridden values
        assert_eq!(config.get_int("backoff.max_elapsed_seconds").unwrap(), 120);

        // Test the convenience functions
        assert_eq!(audit_min_relative_deviation("build_time"), Some(15.0));
        assert_eq!(
            audit_min_relative_deviation("other_measurement"),
            Some(10.0)
        );
        assert_eq!(
            audit_dispersion_method("build_time"),
            git_perf_cli_types::DispersionMethod::StandardDeviation
        );
        assert_eq!(
            audit_dispersion_method("other_measurement"),
            git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
        );
        assert_eq!(backoff_max_elapsed_seconds(), 120);

        // Clean up
        env::remove_var("HOME");
        if original_dir.exists() {
            env::set_current_dir(&original_dir).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_write_config_always_goes_to_repo_root() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        // Create a git repository structure
        let repo_root = temp_dir.path().join("repo");
        let subdir = repo_root.join("subdir").join("deep");
        fs::create_dir_all(&subdir).unwrap();

        // Initialize git repository
        env::set_current_dir(&repo_root).unwrap();
        std::process::Command::new("git")
            .args(&["init", "--initial-branch=master"])
            .output()
            .expect("Failed to initialize git repository");

        // Set up hermetic git environment
        env::set_var("GIT_CONFIG_NOSYSTEM", "true");
        env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        env::set_var("GIT_AUTHOR_NAME", "testuser");
        env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
        env::set_var("GIT_COMMITTER_NAME", "testuser");
        env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");

        // Create a commit to have a HEAD
        fs::write("test.txt", "test content").unwrap();
        std::process::Command::new("git")
            .args(&["add", "test.txt"])
            .output()
            .expect("Failed to add file");
        std::process::Command::new("git")
            .args(&["commit", "-m", "test commit"])
            .output()
            .expect("Failed to commit");

        // Change to subdirectory
        env::set_current_dir(&subdir).unwrap();

        // Write config from subdirectory
        let config_content = "[measurement.\"test\"]\nepoch = \"12345678\"\n";
        write_config(config_content).unwrap();

        // Verify config was written to repo root, not subdirectory
        let repo_config_path = repo_root.join(".gitperfconfig");
        let subdir_config_path = subdir.join(".gitperfconfig");

        assert!(repo_config_path.is_file());
        assert!(!subdir_config_path.is_file());

        let content = fs::read_to_string(&repo_config_path).unwrap();
        assert_eq!(content, config_content);

        // Clean up
        env::remove_var("GIT_CONFIG_NOSYSTEM");
        env::remove_var("GIT_CONFIG_GLOBAL");
        env::remove_var("GIT_AUTHOR_NAME");
        env::remove_var("GIT_AUTHOR_EMAIL");
        env::remove_var("GIT_COMMITTER_NAME");
        env::remove_var("GIT_COMMITTER_EMAIL");
        if original_dir.exists() {
            env::set_current_dir(&original_dir).unwrap();
        }
    }
}
