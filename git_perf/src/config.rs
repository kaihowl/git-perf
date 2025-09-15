use anyhow::Result;
use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toml_edit::{value, Document};

use crate::git::git_interop::get_head_revision;

// Import the CLI types for dispersion method
use git_perf_cli_types::DispersionMethod;

pub fn write_config(conf: &str) -> Result<()> {
    let path = find_config_path().unwrap_or_else(|| PathBuf::from(".gitperfconfig"));
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut f = File::create(path)?;
    f.write_all(conf.as_bytes())?;
    Ok(())
}

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
    File::open(file)?.read_to_string(&mut conf_str)?;
    Ok(conf_str)
}

pub fn determine_epoch_from_config(measurement: &str) -> Option<u32> {
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
    audit_dispersion_method_from_str(read_config().unwrap_or_default().as_str(), measurement)
}

#[cfg(test)]
mod test {
    use super::*;
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
    #[serial_test::serial]
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

        // Configure git user
        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .output()
            .expect("Failed to configure git user");
        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to configure git email");

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
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial_test::serial]
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

        // Configure git user
        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .output()
            .expect("Failed to configure git user");
        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .output()
            .expect("Failed to configure git email");

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
        env::set_current_dir(original_dir).unwrap();
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
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
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

        // Set XDG_CONFIG_HOME and test
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        let found_path = find_config_path();
        assert!(found_path.is_some());
        assert_eq!(found_path.unwrap(), config_path);

        // Clean up
        env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
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
    }

    #[test]
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
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_determine_epoch_from_config_with_missing_file() {
        // Test that missing config file doesn't panic and returns None
        let original_dir = env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let epoch = determine_epoch_from_config("test_measurement");
        assert!(epoch.is_none());

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
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
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_write_config_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("a").join("b").join("c");
        let config_path = nested_dir.join(".gitperfconfig");

        // Mock the config path discovery to return our test path
        let original_dir = env::current_dir().unwrap();
        fs::create_dir_all(&nested_dir).unwrap();
        env::set_current_dir(&nested_dir).unwrap();

        let config_content = "[measurement.\"test\"]\nepoch = \"12345678\"\n";
        write_config(config_content).unwrap();

        assert!(config_path.is_file());
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, config_content);

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }
}
