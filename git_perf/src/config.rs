use anyhow::Result;
use config::{Config, ConfigError, File, FileFormat};
use std::{
    env,
    fs::File as StdFile,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toml_edit::{value, Document};

use crate::git::git_interop::{get_head_revision, get_repository_root};

// Import the CLI types for dispersion method
use git_perf_cli_types::DispersionMethod;

/// Extension trait to get values with parent table fallback.
///
/// This provides a consistent way to retrieve a value for a given logical name
/// and fall back to the parent table when the specific name is not present.
pub trait ConfigParentFallbackExt {
    /// Returns a string value for `{parent}.{name}.{key}` if available.
    /// Otherwise falls back to `{parent}.{key}` (parent table defaults).
    ///
    /// The `parent` is the parent table name (e.g., "measurement").
    /// The `name` is the specific identifier within that parent.
    fn get_with_parent_fallback(&self, parent: &str, name: &str, key: &str) -> Option<String>;
}

impl ConfigParentFallbackExt for Config {
    fn get_with_parent_fallback(&self, parent: &str, name: &str, key: &str) -> Option<String> {
        // Try specific measurement first: parent.name.key
        let specific_key = format!("{}.{}.{}", parent, name, key);
        if let Ok(v) = self.get_string(&specific_key) {
            return Some(v);
        }

        // Fallback to parent table: parent.key
        let parent_key = format!("{}.{}", parent, key);
        if let Ok(v) = self.get_string(&parent_key) {
            return Some(v);
        }

        None
    }
}

/// Get the main repository config path (always in repo root)
fn get_main_config_path() -> Result<PathBuf> {
    // Use git to find the repository root
    let repo_root = get_repository_root().map_err(|e| {
        anyhow::anyhow!(
            "Failed to determine repository root - must be run from within a git repository: {}",
            e
        )
    })?;

    if repo_root.is_empty() {
        return Err(anyhow::anyhow!(
            "Repository root is empty - must be run from within a git repository"
        ));
    }

    Ok(PathBuf::from(repo_root).join(".gitperfconfig"))
}

/// Write config to the main repository directory (always in repo root)
pub fn write_config(conf: &str) -> Result<()> {
    let path = get_main_config_path()?;
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
        builder = builder.add_source(
            File::from(system_config_path)
                .format(FileFormat::Toml)
                .required(false),
        );
    } else if let Some(home) = dirs_next::home_dir() {
        let system_config_path = home.join(".config").join("git-perf").join("config.toml");
        builder = builder.add_source(
            File::from(system_config_path)
                .format(FileFormat::Toml)
                .required(false),
        );
    }

    // 2. Local config (repository .gitperfconfig) - this overrides system config
    if let Some(local_path) = find_config_path() {
        builder = builder.add_source(
            File::from(local_path)
                .format(FileFormat::Toml)
                .required(false),
        );
    }

    builder.build()
}

fn find_config_path() -> Option<PathBuf> {
    // Use get_main_config_path but handle errors gracefully
    let path = get_main_config_path().ok()?;
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn read_config_from_file<P: AsRef<Path>>(file: P) -> Result<String> {
    let mut conf_str = String::new();
    StdFile::open(file)?.read_to_string(&mut conf_str)?;
    Ok(conf_str)
}

pub fn determine_epoch_from_config(measurement: &str) -> Option<u32> {
    let config = read_hierarchical_config()
        .map_err(|e| {
            // Log the error but don't fail - this is expected when no config exists
            log::debug!("Could not read hierarchical config: {}", e);
        })
        .ok()?;

    // Use parent fallback for measurement epoch
    config
        .get_with_parent_fallback("measurement", measurement, "epoch")
        .and_then(|s| u32::from_str_radix(&s, 16).ok())
}

pub fn bump_epoch_in_conf(measurement: &str, conf_str: &mut String) -> Result<()> {
    let mut conf = conf_str
        .parse::<Document>()
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

    let head_revision = get_head_revision()?;

    // Ensure non-inline table formatting by explicitly setting up the structure
    if !conf.contains_key("measurement") {
        conf["measurement"] = toml_edit::table();
    }
    if !conf["measurement"].as_table().unwrap().contains_key(measurement) {
        conf["measurement"][measurement] = toml_edit::table();
    }

    // Set the epoch value
    conf["measurement"][measurement]["epoch"] = value(&head_revision[0..8]);
    *conf_str = conf.to_string();

    Ok(())
}

pub fn bump_epoch(measurement: &str) -> Result<()> {
    // Read existing config from the main config path
    let config_path = get_main_config_path()?;
    let mut conf_str = read_config_from_file(&config_path).unwrap_or_default();

    bump_epoch_in_conf(measurement, &mut conf_str)?;
    write_config(&conf_str)?;
    Ok(())
}

/// Returns the backoff max elapsed seconds from config, or 60 if not set.
pub fn backoff_max_elapsed_seconds() -> u64 {
    match read_hierarchical_config() {
        Ok(config) => {
            if let Ok(seconds) = config.get_int("backoff.max_elapsed_seconds") {
                seconds as u64
            } else {
                60 // Default value
            }
        }
        Err(_) => 60, // Default value when no config exists
    }
}

/// Returns the minimum relative deviation threshold from config, or None if not set.
pub fn audit_min_relative_deviation(measurement: &str) -> Option<f64> {
    let config = read_hierarchical_config().ok()?;

    if let Some(s) =
        config.get_with_parent_fallback("measurement", measurement, "min_relative_deviation")
    {
        if let Ok(v) = s.parse::<f64>() {
            return Some(v);
        }
    }

    None
}

/// Returns the dispersion method from config, or StandardDeviation if not set.
pub fn audit_dispersion_method(measurement: &str) -> DispersionMethod {
    let Some(config) = read_hierarchical_config().ok() else {
        return DispersionMethod::StandardDeviation;
    };

    if let Some(s) =
        config.get_with_parent_fallback("measurement", measurement, "dispersion_method")
    {
        if let Ok(method) = s.parse::<DispersionMethod>() {
            return method;
        }
    }

    // Default to StandardDeviation
    DispersionMethod::StandardDeviation
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Test helper to set up an independent HOME directory
    /// This eliminates the need for #[serial] tests by ensuring each test
    /// has its own isolated environment
    fn with_isolated_home<F, R>(f: F) -> R
    where
        F: FnOnce(&Path) -> R,
    {
        let temp_dir = TempDir::new().unwrap();

        // Set up isolated HOME directory
        env::set_var("HOME", temp_dir.path());
        env::remove_var("XDG_CONFIG_HOME");

        f(temp_dir.path())
    }

    /// Initialize a git repository in the given directory
    fn init_git_repo(dir: &Path) {
        std::process::Command::new("git")
            .args(&["init", "--initial-branch=master"])
            .current_dir(dir)
            .output()
            .expect("Failed to initialize git repository");
    }

    /// Initialize a git repository with an initial commit in the given directory
    fn init_git_repo_with_commit(dir: &Path) {
        init_git_repo(dir);

        // Create a test file and commit it
        fs::write(dir.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(&["add", "test.txt"])
            .current_dir(dir)
            .output()
            .expect("Failed to add file");
        std::process::Command::new("git")
            .args(&["commit", "-m", "test commit"])
            .current_dir(dir)
            .output()
            .expect("Failed to commit");
    }

    /// Create a HOME config directory structure and return the config path
    fn create_home_config_dir(home_dir: &Path) -> PathBuf {
        let config_dir = home_dir.join(".config").join("git-perf");
        fs::create_dir_all(&config_dir).unwrap();
        config_dir.join("config.toml")
    }

    #[test]
    fn test_read_epochs() {
        with_isolated_home(|temp_dir| {
            // Create a git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create workspace config with epochs
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            let configfile = r#"[measurement]
# General performance regression
epoch="12344555"

[measurement."something"]
#My comment
epoch="34567898"

[measurement."somethingelse"]
epoch="a3dead"
"#;
            fs::write(&workspace_config_path, configfile).unwrap();

            let epoch = determine_epoch_from_config("something");
            assert_eq!(epoch, Some(0x34567898));

            let epoch = determine_epoch_from_config("somethingelse");
            assert_eq!(epoch, Some(0xa3dead));

            let epoch = determine_epoch_from_config("unspecified");
            assert_eq!(epoch, Some(0x12344555));
        });
    }

    #[test]
    fn test_bump_epochs() {
        with_isolated_home(|temp_dir| {
            // Create a temporary git repository for this test
            env::set_current_dir(temp_dir).unwrap();

            // Set up hermetic git environment
            env::set_var("GIT_CONFIG_NOSYSTEM", "true");
            env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
            env::set_var("GIT_AUTHOR_NAME", "testuser");
            env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
            env::set_var("GIT_COMMITTER_NAME", "testuser");
            env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");

            // Initialize git repository with initial commit
            init_git_repo_with_commit(temp_dir);

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
        });
    }

    #[test]
    fn test_bump_new_epoch_and_read_it() {
        with_isolated_home(|temp_dir| {
            // Create a temporary git repository for this test
            env::set_current_dir(temp_dir).unwrap();

            // Set up hermetic git environment
            env::set_var("GIT_CONFIG_NOSYSTEM", "true");
            env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
            env::set_var("GIT_AUTHOR_NAME", "testuser");
            env::set_var("GIT_AUTHOR_EMAIL", "testuser@example.com");
            env::set_var("GIT_COMMITTER_NAME", "testuser");
            env::set_var("GIT_COMMITTER_EMAIL", "testuser@example.com");

            // Initialize git repository with initial commit
            init_git_repo_with_commit(temp_dir);

            let mut conf = String::new();
            bump_epoch_in_conf("mymeasurement", &mut conf).expect("Failed to bump epoch");

            // Write the config to a file and test reading it
            let config_path = temp_dir.join(".gitperfconfig");
            fs::write(&config_path, &conf).unwrap();

            let epoch = determine_epoch_from_config("mymeasurement");
            assert!(epoch.is_some());
        });
    }

    #[test]
    fn test_backoff_max_elapsed_seconds() {
        with_isolated_home(|temp_dir| {
            // Create git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create workspace config with explicit value
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            let local_config = "[backoff]\nmax_elapsed_seconds = 42\n";
            fs::write(&workspace_config_path, local_config).unwrap();

            // Test with explicit value
            assert_eq!(super::backoff_max_elapsed_seconds(), 42);

            // Remove config file and test default
            fs::remove_file(&workspace_config_path).unwrap();
            assert_eq!(super::backoff_max_elapsed_seconds(), 60);
        });
    }

    #[test]
    fn test_audit_min_relative_deviation() {
        with_isolated_home(|temp_dir| {
            // Create git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create workspace config with measurement-specific settings
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            let local_config = r#"
[measurement]
min_relative_deviation = 5.0

[measurement."build_time"]
min_relative_deviation = 10.0

[measurement."memory_usage"]
min_relative_deviation = 2.5
"#;
            fs::write(&workspace_config_path, local_config).unwrap();

            // Test measurement-specific settings
            assert_eq!(
                super::audit_min_relative_deviation("build_time"),
                Some(10.0)
            );
            assert_eq!(
                super::audit_min_relative_deviation("memory_usage"),
                Some(2.5)
            );
            assert_eq!(
                super::audit_min_relative_deviation("other_measurement"),
                Some(5.0) // Now falls back to parent table
            );

            // Test global (now parent table) setting
            let global_config = r#"
[measurement]
min_relative_deviation = 5.0
"#;
            fs::write(&workspace_config_path, global_config).unwrap();
            assert_eq!(
                super::audit_min_relative_deviation("any_measurement"),
                Some(5.0)
            );

            // Test precedence - measurement-specific overrides global
            let precedence_config = r#"
[measurement]
min_relative_deviation = 5.0

[measurement."build_time"]
min_relative_deviation = 10.0
"#;
            fs::write(&workspace_config_path, precedence_config).unwrap();
            assert_eq!(
                super::audit_min_relative_deviation("build_time"),
                Some(10.0)
            );
            assert_eq!(
                super::audit_min_relative_deviation("other_measurement"),
                Some(5.0)
            );

            // Test no config
            fs::remove_file(&workspace_config_path).unwrap();
            assert_eq!(super::audit_min_relative_deviation("any_measurement"), None);
        });
    }

    #[test]
    fn test_audit_dispersion_method() {
        with_isolated_home(|temp_dir| {
            // Create git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create workspace config with measurement-specific settings
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            let local_config = r#"
[measurement]
dispersion_method = "stddev"

[measurement."build_time"]
dispersion_method = "mad"

[measurement."memory_usage"]
dispersion_method = "stddev"
"#;
            fs::write(&workspace_config_path, local_config).unwrap();

            // Test measurement-specific settings
            assert_eq!(
                super::audit_dispersion_method("build_time"),
                git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
            );
            assert_eq!(
                super::audit_dispersion_method("memory_usage"),
                git_perf_cli_types::DispersionMethod::StandardDeviation
            );
            assert_eq!(
                super::audit_dispersion_method("other_measurement"),
                git_perf_cli_types::DispersionMethod::StandardDeviation
            );

            // Test global (now parent table) setting
            let global_config = r#"
[measurement]
dispersion_method = "mad"
"#;
            fs::write(&workspace_config_path, global_config).unwrap();
            assert_eq!(
                super::audit_dispersion_method("any_measurement"),
                git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
            );

            // Test precedence - measurement-specific overrides global
            let precedence_config = r#"
[measurement]
dispersion_method = "mad"

[measurement."build_time"]
dispersion_method = "stddev"
"#;
            fs::write(&workspace_config_path, precedence_config).unwrap();
            assert_eq!(
                super::audit_dispersion_method("build_time"),
                git_perf_cli_types::DispersionMethod::StandardDeviation
            );
            assert_eq!(
                super::audit_dispersion_method("other_measurement"),
                git_perf_cli_types::DispersionMethod::MedianAbsoluteDeviation
            );

            // Test no config (should return StandardDeviation)
            fs::remove_file(&workspace_config_path).unwrap();
            assert_eq!(
                super::audit_dispersion_method("any_measurement"),
                git_perf_cli_types::DispersionMethod::StandardDeviation
            );
        });
    }

    #[test]
    fn test_find_config_path_in_git_root() {
        with_isolated_home(|temp_dir| {
            // Create a git repository
            env::set_current_dir(temp_dir).unwrap();

            // Initialize git repository
            init_git_repo(temp_dir);

            // Create config in git root
            let config_path = temp_dir.join(".gitperfconfig");
            fs::write(
                &config_path,
                "[measurement.\"test\"]\nepoch = \"12345678\"\n",
            )
            .unwrap();

            // Test that find_config_path finds it
            let found_path = find_config_path();
            assert!(found_path.is_some());
            assert_eq!(found_path.unwrap(), config_path);
        });
    }

    #[test]
    fn test_find_config_path_not_found() {
        with_isolated_home(|temp_dir| {
            // Create a git repository but no .gitperfconfig
            env::set_current_dir(temp_dir).unwrap();

            // Initialize git repository
            init_git_repo(temp_dir);

            // Test that find_config_path returns None when no .gitperfconfig exists
            let found_path = find_config_path();
            assert!(found_path.is_none());
        });
    }

    #[test]
    fn test_hierarchical_config_workspace_overrides_home() {
        with_isolated_home(|temp_dir| {
            // Create a git repository
            env::set_current_dir(temp_dir).unwrap();

            // Initialize git repository
            init_git_repo(temp_dir);

            // Create home config
            let home_config_path = create_home_config_dir(temp_dir);
            fs::write(
                &home_config_path,
                r#"
[measurement."test"]
backoff_max_elapsed_seconds = 30
audit_min_relative_deviation = 1.0
"#,
            )
            .unwrap();

            // Create workspace config that overrides some values
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            fs::write(
                &workspace_config_path,
                r#"
[measurement."test"]
backoff_max_elapsed_seconds = 60
"#,
            )
            .unwrap();

            // Set HOME to our temp directory
            env::set_var("HOME", temp_dir);
            env::remove_var("XDG_CONFIG_HOME");

            // Read hierarchical config and verify workspace overrides home
            let config = read_hierarchical_config().unwrap();

            // backoff_max_elapsed_seconds should be overridden by workspace config
            let backoff: i32 = config
                .get("measurement.test.backoff_max_elapsed_seconds")
                .unwrap();
            assert_eq!(backoff, 60);

            // audit_min_relative_deviation should come from home config
            let deviation: f64 = config
                .get("measurement.test.audit_min_relative_deviation")
                .unwrap();
            assert_eq!(deviation, 1.0);
        });
    }

    #[test]
    fn test_determine_epoch_from_config_with_missing_file() {
        // Test that missing config file doesn't panic and returns None
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let epoch = determine_epoch_from_config("test_measurement");
        assert!(epoch.is_none());
    }

    #[test]
    fn test_determine_epoch_from_config_with_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".gitperfconfig");
        fs::write(&config_path, "invalid toml content").unwrap();

        fs::create_dir_all(temp_dir.path()).unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let epoch = determine_epoch_from_config("test_measurement");
        assert!(epoch.is_none());
    }

    #[test]
    fn test_write_config_creates_file() {
        with_isolated_home(|temp_dir| {
            // Create git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create a subdirectory to test that config is written to repo root
            let subdir = temp_dir.join("a").join("b").join("c");
            fs::create_dir_all(&subdir).unwrap();
            env::set_current_dir(&subdir).unwrap();

            let config_content = "[measurement.\"test\"]\nepoch = \"12345678\"\n";
            write_config(config_content).unwrap();

            // Config should be written to repo root, not subdirectory
            let repo_config_path = temp_dir.join(".gitperfconfig");
            let subdir_config_path = subdir.join(".gitperfconfig");

            assert!(repo_config_path.is_file());
            assert!(!subdir_config_path.is_file());

            let content = fs::read_to_string(&repo_config_path).unwrap();
            assert_eq!(content, config_content);
        });
    }

    #[test]
    fn test_hierarchical_config_system_override() {
        with_isolated_home(|temp_dir| {
            // Create system config (home directory config)
            let system_config_path = create_home_config_dir(temp_dir);
            let system_config = r#"
[measurement]
min_relative_deviation = 5.0
dispersion_method = "mad"

[backoff]
max_elapsed_seconds = 120
"#;
            fs::write(&system_config_path, system_config).unwrap();

            // Create git repository
            env::set_current_dir(temp_dir).unwrap();
            init_git_repo(temp_dir);

            // Create workspace config that overrides system config
            let workspace_config_path = temp_dir.join(".gitperfconfig");
            let local_config = r#"
[measurement]
min_relative_deviation = 10.0

[measurement."build_time"]
min_relative_deviation = 15.0
dispersion_method = "stddev"
"#;
            fs::write(&workspace_config_path, local_config).unwrap();

            // Test hierarchical config reading
            let config = read_hierarchical_config().unwrap();

            // Test that local parent table overrides system config via helper
            use super::ConfigParentFallbackExt;
            assert_eq!(
                config
                    .get_with_parent_fallback(
                        "measurement",
                        "any_measurement",
                        "min_relative_deviation"
                    )
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                10.0
            );
            assert_eq!(
                config
                    .get_with_parent_fallback("measurement", "any_measurement", "dispersion_method")
                    .unwrap(),
                "mad"
            ); // Not overridden in local for parent fallback

            // Test measurement-specific override
            assert_eq!(
                config
                    .get_float("measurement.build_time.min_relative_deviation")
                    .unwrap(),
                15.0
            );
            assert_eq!(
                config
                    .get_string("measurement.build_time.dispersion_method")
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
        });
    }
}
