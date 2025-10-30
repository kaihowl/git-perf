# Plan: Add `config list` Subcommand

**Status:** Planning
**Created:** 2025-10-30
**Issue:** #327

## Overview

Add a new `config list` subcommand to git-perf that displays current configuration information and git context. This will help users understand their active git-perf configuration, including branch context, config sources, and measurement settings.

## Motivation

Currently, git-perf users have limited visibility into their active configuration:
- No easy way to see which config settings are active
- Unclear which configuration source is being used (system vs local)
- No quick way to check current branch context
- Difficult to debug configuration issues
- Users must manually inspect `.gitperfconfig` files

A dedicated `config list` command would provide:
- Quick overview of current git-perf environment
- Configuration debugging capabilities
- Better understanding of hierarchical config resolution
- Context awareness (branch name, repository location)
- Documentation of active measurement settings

## Goals

1. **Display git context** - Show current branch name and repository root
2. **Show configuration sources** - Indicate which config files are being used
3. **List measurement configurations** - Display all configured measurements and their settings
4. **Global settings visibility** - Show global configuration values (backoff, etc.)
5. **Multiple output formats** - Support human-readable and machine-readable (JSON) formats
6. **Config validation** - Optionally validate configuration and report issues

## Non-Goals (Future Work)

- Editing configuration via CLI (users should edit `.gitperfconfig` directly)
- Interactive configuration wizard
- Comparing configurations across branches
- Configuration migration/upgrade tools
- Performance analysis of configuration settings

## Background: Configuration System

Based on analysis of `git_perf/src/config.rs`:

### Configuration Hierarchy

git-perf uses a two-level hierarchical configuration system:

1. **System-wide config** (optional):
   - Location: `$XDG_CONFIG_HOME/git-perf/config.toml` or `~/.config/git-perf/config.toml`
   - Purpose: Default settings for all repositories
   - Priority: Lowest

2. **Local config** (optional):
   - Location: `.gitperfconfig` in repository root
   - Purpose: Repository-specific settings that override system defaults
   - Priority: Highest (overrides system config)

### Configuration Structure

TOML format with the following sections:

```toml
# Global settings
[backoff]
max_elapsed_seconds = 60

# Parent table defaults (apply to all measurements)
[measurement]
epoch = "12345678"
min_relative_deviation = 5.0
dispersion_method = "stddev"
min_measurements = 5
aggregate_by = "min"
sigma = 4.0
unit = "ms"

# Measurement-specific settings (override parent defaults)
[measurement."build_time"]
epoch = "abcdef01"
min_relative_deviation = 10.0
dispersion_method = "mad"
min_measurements = 10
aggregate_by = "median"
sigma = 3.0
unit = "seconds"

[measurement."memory_usage"]
unit = "bytes"
# Other settings inherited from parent table
```

### Configuration Resolution

The `ConfigParentFallbackExt` trait (lines 16-45) provides hierarchical resolution:
1. Try measurement-specific setting: `measurement.{name}.{key}`
2. Fall back to parent table default: `measurement.{key}`
3. Return None if neither exists

### Existing Configuration Functions

From `config.rs`:
- `get_main_config_path()` - Get repository config path
- `read_hierarchical_config()` - Read merged system + local config
- `determine_epoch_from_config(measurement)` - Get epoch for measurement
- `audit_min_relative_deviation(measurement)` - Get min relative deviation
- `audit_dispersion_method(measurement)` - Get dispersion method
- `audit_min_measurements(measurement)` - Get min measurements
- `audit_aggregate_by(measurement)` - Get aggregation function
- `audit_sigma(measurement)` - Get sigma value
- `measurement_unit(measurement)` - Get measurement unit
- `backoff_max_elapsed_seconds()` - Get backoff setting

## Design

### 1. CLI Definition

**File**: `cli_types/src/lib.rs`

Add to `Commands` enum (after `Size`, around line 431):

```rust
/// Display current git-perf configuration and context
///
/// Shows active configuration settings, including git context (branch name,
/// repository location), configuration sources, and measurement-specific
/// settings. This helps users understand their git-perf environment and
/// debug configuration issues.
///
/// By default, shows a summary of configuration. Use --detailed to see
/// all measurement-specific settings, or --json for machine-readable output.
///
/// Examples:
///   git perf config list                    # Show configuration summary
///   git perf config list --detailed         # Show all measurement settings
///   git perf config list --json             # Output as JSON
///   git perf config list --validate         # Check for config issues
ConfigList {
    /// Show detailed configuration including all measurements
    #[arg(short, long)]
    detailed: bool,

    /// Output format (human-readable or JSON)
    #[arg(short, long, value_enum, default_value = "human")]
    format: ConfigInfoFormat,

    /// Validate configuration and report issues
    #[arg(short, long)]
    validate: bool,

    /// Show specific measurement configuration only
    #[arg(short, long)]
    measurement: Option<String>,
},
```

Add enum for format (after `SizeFormat`, around line 106):

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigListFormat {
    /// Human-readable format
    Human,
    /// JSON format for machine parsing
    Json,
}
```

### 2. Config List Module Implementation

**File**: `git_perf/src/config_list.rs` (NEW FILE)

#### Core Data Structures

```rust
use crate::config::{
    audit_aggregate_by, audit_dispersion_method, audit_min_measurements,
    audit_min_relative_deviation, audit_sigma, backoff_max_elapsed_seconds,
    determine_epoch_from_config, get_main_config_path, measurement_unit,
    read_hierarchical_config,
};
use crate::git::git_interop::{get_repository_root};
use git_perf_cli_types::ConfigListFormat;
use anyhow::{Context, Result};
use config::Config;
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
    pub system_config: Option<PathBuf>,

    /// Local repository config path (if exists)
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
    pub epoch: Option<String>,

    /// Minimum relative deviation threshold (%)
    pub min_relative_deviation: Option<f64>,

    /// Dispersion method (stddev or mad)
    pub dispersion_method: String,

    /// Minimum measurements required
    pub min_measurements: Option<u16>,

    /// Aggregation function
    pub aggregate_by: Option<String>,

    /// Sigma threshold
    pub sigma: Option<f64>,

    /// Measurement unit
    pub unit: Option<String>,

    /// Whether this is from parent table fallback (vs measurement-specific)
    pub from_parent_fallback: bool,
}
```

#### Main Entry Point

```rust
/// Display configuration information
pub fn show_config_list(
    detailed: bool,
    format: ConfigListFormat,
    validate: bool,
    measurement_filter: Option<String>,
) -> Result<()> {
    // 1. Gather configuration information
    let config_info = gather_config_info(validate, measurement_filter.as_deref())?;

    // 2. Display based on format
    match format {
        ConfigListFormat::Human => display_human_readable(&config_info, detailed)?,
        ConfigListFormat::Json => display_json(&config_info)?,
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
```

#### Information Gathering Functions

```rust
/// Gather all configuration information
fn gather_config_info(
    validate: bool,
    measurement_filter: Option<&str>,
) -> Result<ConfigInfo> {
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
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("Failed to get current branch")?;

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get repository root
    let repository_root = PathBuf::from(get_repository_root()?);

    Ok(GitContext {
        branch,
        repository_root,
    })
}

/// Determine which config files are being used
fn gather_config_sources() -> Result<ConfigSources> {
    // System config
    let system_config = find_system_config();

    // Local config
    let local_config = get_main_config_path().ok()
        .filter(|p| p.exists());

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
            if value.kind() == config::ValueKind::Table {
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
```

#### Display Functions

```rust
/// Display configuration in human-readable format
fn display_human_readable(info: &ConfigInfo, detailed: bool) -> Result<()> {
    println!("Git-Perf Configuration");
    println!("======================");
    println!();

    // Git Context
    println!("Git Context:");
    println!("  Branch: {}", info.git_context.branch);
    println!("  Repository: {}", info.git_context.repository_root.display());
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
                println!("  ⚠ {}", issue);
            }
        } else {
            println!();
            println!("✓ Configuration is valid");
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
        println!("    min_relative_deviation: {:?}", measurement.min_relative_deviation);
        println!("    dispersion_method:      {}", measurement.dispersion_method);
        println!("    min_measurements:       {:?}", measurement.min_measurements);
        println!("    aggregate_by:           {:?}", measurement.aggregate_by);
        println!("    sigma:                  {:?}", measurement.sigma);
        println!("    unit:                   {:?}", measurement.unit);
        println!();
    } else {
        // Summary view - just name and epoch
        let epoch_display = measurement.epoch.as_deref().unwrap_or("(not set)");
        let unit_display = measurement.unit.as_deref().unwrap_or("(not set)");
        println!("  {} - epoch: {}, unit: {}", measurement.name, epoch_display, unit_display);
    }
}

/// Display configuration as JSON
fn display_json(info: &ConfigInfo) -> Result<()> {
    let json = serde_json::to_string_pretty(info)
        .context("Failed to serialize configuration to JSON")?;
    println!("{}", json);
    Ok(())
}
```

### 3. Command Handler

**File**: `git_perf/src/cli.rs`

Add to match statement (after `Size`, around line 130):

```rust
Commands::ConfigList {
    detailed,
    format,
    validate,
    measurement,
} => {
    config_list::show_config_list(
        *detailed,
        *format,
        *validate,
        measurement.clone(),
    )?;
}
```

Add module declaration at top of file:

```rust
mod config_list;
```

### 4. Module Registration

**File**: `git_perf/src/lib.rs`

Ensure the `config_list` module is declared (if not already included via other module declarations).

## Implementation Phases

### Phase 1: Core Implementation

1. Add CLI definition to `cli_types/src/lib.rs`
   - `ConfigList` command variant with flags
   - `ConfigListFormat` enum
2. Create `git_perf/src/config_list.rs` module
   - Core data structures
   - Main entry point
   - Information gathering functions
   - Basic human-readable display
3. Wire up command handler in `git_perf/src/cli.rs`
4. Run `cargo fmt` and `cargo clippy`
5. Run `./scripts/generate-manpages.sh` to regenerate documentation

### Phase 2: Enhanced Features

1. Implement JSON output format
2. Add configuration validation
3. Add measurement-specific filtering
4. Improve error messages and context

### Phase 3: Documentation

1. Add comprehensive doc comments to CLI definition
2. Update main README if needed
3. Add examples to manpage
4. Update `docs/INTEGRATION_TUTORIAL.md` to reference `config list`

### Phase 4: Testing

1. Create `test/test_config_list.sh` integration test
2. Add test to `test/run_tests.sh`
3. Run full test suite: `cargo nextest run -- --skip slow`
4. Manual testing with various configurations

### Phase 5: Code Quality

1. Run `cargo fmt` to format code
2. Run `cargo clippy` and address warnings
3. Manual testing with edge cases
4. Performance testing with large configurations

## Example Usage

### Basic Summary

```bash
$ git perf config list
Git-Perf Configuration
======================

Git Context:
  Branch: terragon/feature-next-item-selection-tqib1h
  Repository: /root/repo

Configuration Sources:
  System config: (none)
  Local config:  /root/repo/.gitperfconfig

Global Settings:
  backoff.max_elapsed_seconds: 60

Measurements: (3 configured)

  benchmark_render - epoch: abcdef01, unit: ms
  benchmark_parse - epoch: 12345678, unit: ns
  memory_usage - epoch: fedcba98, unit: bytes
```

### Detailed View

```bash
$ git perf config list --detailed
Git-Perf Configuration
======================

Git Context:
  Branch: main
  Repository: /home/user/myproject

Configuration Sources:
  System config: /home/user/.config/git-perf/config.toml
  Local config:  /home/user/myproject/.gitperfconfig

Global Settings:
  backoff.max_elapsed_seconds: 120

Measurements: (2 configured)

  [benchmark_render]
    epoch:                  Some("abcdef01")
    min_relative_deviation: Some(10.0)
    dispersion_method:      mad
    min_measurements:       Some(5)
    aggregate_by:           Some("median")
    sigma:                  Some(3.0)
    unit:                   Some("ms")

  [memory_usage]
    (using parent table defaults)
    epoch:                  Some("12345678")
    min_relative_deviation: Some(5.0)
    dispersion_method:      stddev
    min_measurements:       Some(2)
    aggregate_by:           Some("min")
    sigma:                  Some(4.0)
    unit:                   Some("bytes")
```

### JSON Output

```bash
$ git perf config list --json
{
  "git_context": {
    "branch": "main",
    "repository_root": "/home/user/myproject"
  },
  "config_sources": {
    "system_config": "/home/user/.config/git-perf/config.toml",
    "local_config": "/home/user/myproject/.gitperfconfig"
  },
  "global_settings": {
    "backoff_max_elapsed_seconds": 60
  },
  "measurements": {
    "benchmark_render": {
      "name": "benchmark_render",
      "epoch": "abcdef01",
      "min_relative_deviation": 10.0,
      "dispersion_method": "mad",
      "min_measurements": 5,
      "aggregate_by": "median",
      "sigma": 3.0,
      "unit": "ms",
      "from_parent_fallback": false
    }
  },
  "validation_issues": null
}
```

### Validation

```bash
$ git perf config list --validate
Git-Perf Configuration
======================

Git Context:
  Branch: main
  Repository: /home/user/myproject

Configuration Sources:
  System config: (none)
  Local config:  /home/user/myproject/.gitperfconfig

Global Settings:
  backoff.max_elapsed_seconds: 60

Measurements: (1 configured)

  new_measurement - epoch: (not set), unit: ms

Validation Issues:
  ⚠ Measurement 'new_measurement': No epoch configured (run 'git perf bump-epoch -m new_measurement')
```

### Specific Measurement

```bash
$ git perf config list --measurement benchmark_render --detailed
Git-Perf Configuration
======================

Git Context:
  Branch: main
  Repository: /home/user/myproject

Configuration Sources:
  System config: (none)
  Local config:  /home/user/myproject/.gitperfconfig

Global Settings:
  backoff.max_elapsed_seconds: 60

Measurements: (1 configured)

  [benchmark_render]
    epoch:                  Some("abcdef01")
    min_relative_deviation: Some(10.0)
    dispersion_method:      mad
    min_measurements:       Some(5)
    aggregate_by:           Some("median")
    sigma:                  Some(3.0)
    unit:                   Some("ms")
```

## Technical Considerations

### Configuration Resolution

1. **Hierarchical Config**:
   - System config provides base defaults
   - Local config overrides system settings
   - Use existing `read_hierarchical_config()` function

2. **Parent Table Fallback**:
   - Measurement-specific settings override parent defaults
   - Use existing `ConfigParentFallbackExt` trait
   - Indicate in output whether setting is specific or inherited

3. **Missing Config**:
   - Handle gracefully when no config files exist
   - Show defaults being used
   - Don't error on missing configuration

### Git Context

1. **Branch Name**:
   - Use `git rev-parse --abbrev-ref HEAD`
   - Handle detached HEAD state (shows commit hash)
   - Handle errors if not in a git repository

2. **Repository Root**:
   - Use existing `get_repository_root()` function
   - Display absolute path for clarity

### Output Formats

1. **Human-Readable**:
   - Clear section headers
   - Indented hierarchy
   - Optional detailed/summary views
   - Unicode symbols for validation (✓, ⚠)

2. **JSON**:
   - Machine-parseable
   - Complete information
   - Consistent schema
   - Use serde for serialization

### Validation

1. **Config Issues to Detect**:
   - Missing epochs (warn user to run bump-epoch)
   - Invalid numeric values (negative sigma, etc.)
   - Invalid min_measurements (< 2)
   - Malformed TOML (already handled by config parser)

2. **Validation Output**:
   - Clear descriptions of issues
   - Actionable recommendations
   - Exit with error code if issues found

### Error Handling

1. **Not in Git Repository**:
   - Clear error message
   - Suggest running from within repository

2. **Config Parse Errors**:
   - Show file path and error
   - Suggest checking TOML syntax

3. **Missing Files**:
   - Indicate which files are optional
   - Don't error on missing system config

## Dependencies

### Existing Crates

- `config` - Already used for config parsing
- `serde` and `serde_json` - For JSON output (may need to add)
- `anyhow` - Error handling (already used)

### New Dependencies (if needed)

Add to `git_perf/Cargo.toml`:

```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dirs-next = "2.0"  # Already used in config.rs
```

## Compatibility

### Git Version Requirements

- No special Git version requirements
- Uses basic `git rev-parse` command (available in all Git versions)

### Backward Compatibility

- No changes to configuration file format
- No changes to existing commands
- New command, no risk of breaking existing workflows
- Safe to run at any time (read-only operation)

## Future Enhancements

### Near-term

1. Config diff between system and local
2. Show effective config (merged view)
3. List all possible configuration keys with descriptions
4. Export config as template

### Long-term

1. Interactive config editor
2. Config validation in CI
3. Config migration tools
4. Compare config across branches
5. Config profiles (dev, ci, production)

## Success Criteria

- [ ] Command successfully displays git context
- [ ] Command shows configuration sources
- [ ] Command lists all measurement configurations
- [ ] Detailed view shows all settings for each measurement
- [ ] JSON output is valid and complete
- [ ] Validation detects common config issues
- [ ] Measurement filtering works correctly
- [ ] Gracefully handles missing config files
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted with `cargo fmt`
- [ ] Documentation generated and committed
- [ ] Manual testing with various configurations

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Complex config structure | Hard to display clearly | Use hierarchical indentation, clear labels |
| Large number of measurements | Output too verbose | Default to summary view, detailed flag for full info |
| Config parsing errors | Command fails | Graceful error handling, clear messages |
| Not in git repository | Command fails | Clear error message with helpful suggestion |
| Missing serde dependency | Build failures | Add dependencies early, test build |

## Related Work

- **Existing Commands**:
  - `git perf bump-epoch` - Updates epoch in config
  - Various audit/report commands - Read from config

- **Complementary Tools**:
  - Manual `.gitperfconfig` file inspection
  - TOML editors and validators

- **Similar Features in Other Tools**:
  - `git config --list` - List git configuration
  - `npm config list` - Show npm configuration
  - `cargo config get` - Show cargo configuration

## References

### Codebase References

- `git_perf/src/config.rs` - Configuration system implementation
- `git_perf/src/git/git_interop.rs` - Git wrapper functions
- `cli_types/src/lib.rs` - CLI command definitions
- `docs/plans/size-subcommand.md` - Template for this design doc

### Documentation

- TOML specification
- Serde documentation for JSON serialization
- Clap documentation for CLI argument parsing

## Appendix: Alternative Approaches Considered

### Approach 1: Nested subcommands (config with list subcommand)

**Example**: Nested structure with `config` parent and `list` child subcommand

**Pros**: Could allow future `config edit`, `config validate` commands
**Cons**: More complex, requires nested subcommand structure in Clap
**Decision**: Use simpler `ConfigList` variant that becomes `config-list` command, consistent with `ListCommits` → `list-commits` pattern

### Approach 2: Multiple separate commands

**Example**: `git perf show-context`, `git perf show-config`, etc.

**Pros**: Very specific commands
**Cons**: Too many commands, harder to discover
**Decision**: Single `config list` command with optional filtering

### Approach 3: Only JSON output

**Pros**: Simpler implementation
**Cons**: Not user-friendly for quick checks
**Decision**: Support both human-readable and JSON formats
