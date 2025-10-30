use anyhow::{anyhow, bail, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::str::FromStr;

use chrono::prelude::*;
use chrono::Duration;

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReductionFunc {
    Min,
    Max,
    Median,
    Mean,
}

impl FromStr for ReductionFunc {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "min" => Ok(ReductionFunc::Min),
            "max" => Ok(ReductionFunc::Max),
            "median" => Ok(ReductionFunc::Median),
            "mean" => Ok(ReductionFunc::Mean),
            _ => Err(anyhow!(
                "Invalid reduction function: {}. Valid values are 'min', 'max', 'median', or 'mean'",
                s
            )),
        }
    }
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum DispersionMethod {
    #[value(name = "stddev")]
    StandardDeviation,
    #[value(name = "mad")]
    MedianAbsoluteDeviation,
}

impl FromStr for DispersionMethod {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stddev" => Ok(DispersionMethod::StandardDeviation),
            "mad" => Ok(DispersionMethod::MedianAbsoluteDeviation),
            _ => Err(anyhow!(
                "Invalid dispersion method: {}. Valid values are 'stddev' or 'mad'",
                s
            )),
        }
    }
}

#[derive(Parser)]
#[command(version, name = "git-perf")]
pub struct Cli {
    /// Increase verbosity level (can be specified multiple times.) The first level sets level
    /// "info", second sets level "debug", and third sets level "trace" for the logger.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Create a versionless command for manpage generation
    pub fn command_without_version() -> clap::Command {
        let mut cmd = Self::command();
        cmd = cmd.version(None::<&str>);
        cmd
    }
}

#[derive(Args)]
pub struct CliMeasurement {
    /// Name of the measurement
    #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string)]
    pub name: String,

    /// Key-value pairs separated by '='
    #[arg(short, long, value_parser=parse_key_value)]
    pub key_value: Vec<(String, String)>,
}

#[derive(Args)]
pub struct CliReportHistory {
    /// Limit the number of previous commits considered.
    /// HEAD is included in this count.
    #[arg(short = 'n', long, default_value = "40")]
    pub max_count: usize,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    /// JUnit XML format (nextest, pytest, Jest, etc.)
    Junit,
    /// cargo-criterion JSON format
    CriterionJson,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum SizeFormat {
    /// Human-readable format (e.g., "1.2 MB")
    Human,
    /// Raw bytes as integer
    Bytes,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Measure the runtime of the supplied command (in nanoseconds)
    Measure {
        /// Repetitions
        #[arg(short = 'n', long, value_parser=clap::value_parser!(u16).range(1..), default_value = "1")]
        repetitions: u16,

        #[command(flatten)]
        measurement: CliMeasurement,

        /// Command to measure
        #[arg(required(true), last(true))]
        command: Vec<String>,
    },

    /// Add single measurement
    Add {
        /// Measured value to be added
        value: f64,

        #[command(flatten)]
        measurement: CliMeasurement,
    },

    /// Import measurements from test runners and benchmarks
    ///
    /// Parse and store runtime measurements from external tools like cargo-nextest
    /// (JUnit XML) and cargo-criterion (JSON). This allows tracking test and benchmark
    /// performance over time using git-perf's measurement infrastructure.
    ///
    /// ## Supported Formats
    ///
    /// **junit** - JUnit XML format
    /// - Works with: cargo-nextest, pytest, Jest, JUnit, and many other test frameworks
    /// - Requires: Configure nextest with JUnit output in `.config/nextest.toml`
    /// - Command: `cargo nextest run --profile ci` (outputs to target/nextest/ci/junit.xml)
    ///
    /// **criterion-json** - cargo-criterion JSON format
    /// - Works with: cargo-criterion benchmarks
    /// - Command: `cargo criterion --message-format json`
    ///
    /// ## Measurement Naming
    ///
    /// Tests: `test::<test_name>`
    /// Benchmarks: `bench::<benchmark_id>::<statistic>` (mean, median, slope, mad)
    ///
    /// ## Examples
    ///
    /// ```bash
    /// # Import test results from file
    /// git-perf import junit target/nextest/ci/junit.xml
    ///
    /// # Import from stdin
    /// cat junit.xml | git-perf import junit
    ///
    /// # Import with metadata
    /// git-perf import junit junit.xml --metadata ci=true --metadata branch=main
    ///
    /// # Import with filtering (regex)
    /// git-perf import junit junit.xml --filter "^integration::"
    ///
    /// # Dry run to preview
    /// git-perf import junit junit.xml --dry-run --verbose
    ///
    /// # Import benchmarks
    /// cargo criterion --message-format json > bench.json
    /// git-perf import criterion-json bench.json
    /// ```
    Import {
        /// Format of the input data
        format: ImportFormat,

        /// Input file path (use '-' or omit for stdin)
        file: Option<String>,

        /// Optional prefix to prepend to measurement names
        #[arg(short, long)]
        prefix: Option<String>,

        /// Key-value pairs separated by '=' to add as metadata to all measurements
        #[arg(short, long, value_parser=parse_key_value)]
        metadata: Vec<(String, String)>,

        /// Regex filter to select specific tests/benchmarks
        #[arg(short = 'f', long)]
        filter: Option<String>,

        /// Preview what would be imported without storing
        #[arg(long)]
        dry_run: bool,

        /// Show detailed information about imported measurements
        #[arg(short, long)]
        verbose: bool,
    },

    /// Publish performance results to remote
    Push {
        /// Remote to push to (defaults to git-perf-origin)
        #[arg(short, long)]
        remote: Option<String>,
    },

    /// Pull performance results from remote
    Pull {},

    /// Create an HTML performance report
    Report {
        /// HTML output file
        #[arg(short, long, default_value = "output.html")]
        output: PathBuf,

        #[command(flatten)]
        report_history: CliReportHistory,

        /// Select an individual measurements instead of all
        #[arg(short, long)]
        measurement: Vec<String>,

        /// Key-value pairs separated by '=', select only matching measurements
        #[arg(short, long, value_parser=parse_key_value)]
        key_value: Vec<(String, String)>,

        /// Filter measurements by regex pattern (can be specified multiple times).
        /// If any filter matches, the measurement is included (OR logic).
        /// Patterns are unanchored by default. Use ^pattern$ for exact matches.
        /// Example: -f "bench.*" -f "test_.*"
        #[arg(short = 'f', long = "filter")]
        filter: Vec<String>,

        /// Create individual traces in the graph by grouping with the value of this selector
        #[arg(short, long, value_parser=parse_spaceless_string)]
        separate_by: Option<String>,

        /// What to aggregate the measurements in each group with
        #[arg(short, long)]
        aggregate_by: Option<ReductionFunc>,
    },

    /// For given measurements, check perfomance deviations of the HEAD commit
    /// against `<n>` previous commits. Group previous results and aggregate their
    /// results before comparison.
    ///
    /// The audit can be configured to ignore statistically significant deviations
    /// if they are below a minimum relative deviation threshold. This helps filter
    /// out noise while still catching meaningful performance changes.
    ///
    /// ## Statistical Dispersion Methods
    ///
    /// The audit supports two methods for calculating statistical dispersion:
    ///
    /// **Standard Deviation (stddev)**: Traditional method that is sensitive to
    /// outliers. Use when your performance data is normally distributed and you
    /// want to detect all performance changes, including those caused by outliers.
    ///
    /// **Median Absolute Deviation (MAD)**: Robust method that is less sensitive
    /// to outliers. Use when your performance data has occasional outliers or
    /// spikes, or when you want to focus on typical performance changes rather
    /// than extreme values.
    ///
    /// ## Configuration
    ///
    /// Configuration is done via the `.gitperfconfig` file:
    ///
    /// **Default settings:**
    /// - `[measurement].min_relative_deviation = 5.0`
    /// - `[measurement].dispersion_method = "mad"`
    /// - `[measurement].min_measurements = 3`
    /// - `[measurement].aggregate_by = "median"`
    /// - `[measurement].sigma = 3.5`
    ///
    /// **Measurement-specific settings (override defaults):**
    /// - `[measurement."name"].min_relative_deviation = 10.0`
    /// - `[measurement."name"].dispersion_method = "stddev"`
    /// - `[measurement."name"].min_measurements = 5`
    /// - `[measurement."name"].aggregate_by = "mean"`
    /// - `[measurement."name"].sigma = 4.5`
    ///
    /// ## Precedence
    ///
    /// All audit options follow the same precedence order:
    /// 1. CLI option (if specified) - highest priority
    /// 2. Measurement-specific config - overrides default
    /// 3. Default config - overrides built-in default
    /// 4. Built-in default - lowest priority
    ///
    /// **Note:** When `--min-measurements` is specified on CLI, it applies to ALL
    /// measurements in the audit, overriding any per-measurement config values.
    ///
    /// Built-in defaults:
    /// - `min_measurements`: 2
    /// - `aggregate_by`: min
    /// - `sigma`: 4.0
    /// - `dispersion_method`: stddev
    ///
    /// When the relative deviation is below the threshold, the audit passes even
    /// if the z-score exceeds the sigma threshold. The relative deviation is
    /// calculated as: `|(head_value / tail_median - 1.0) * 100%|` where tail_median is
    /// the median of historical measurements (excluding HEAD).
    ///
    /// The sparkline visualization shows the range of measurements relative to
    /// the tail median (historical measurements only).
    Audit {
        /// Specific measurement names to audit (can be specified multiple times).
        /// At least one of --measurement or --filter must be provided.
        /// Multiple measurements use OR logic.
        /// Example: -m timer -m memory
        #[arg(short, long, value_parser=parse_spaceless_string, action = clap::ArgAction::Append, required_unless_present = "filter")]
        measurement: Vec<String>,

        #[command(flatten)]
        report_history: CliReportHistory,

        /// Key-value pair separated by "=" with no whitespaces to subselect measurements
        #[arg(short, long, value_parser=parse_key_value)]
        selectors: Vec<(String, String)>,

        /// Filter measurements by regex pattern (can be specified multiple times).
        /// At least one of --measurement or --filter must be provided.
        /// If any filter matches, the measurement is included (OR logic).
        /// Patterns are unanchored by default. Use ^pattern$ for exact matches.
        /// Examples: -f "bench_.*" (prefix), -f ".*_x64$" (suffix), -f "^perf_" (anchored prefix)
        #[arg(short = 'f', long = "filter", required_unless_present = "measurement")]
        filter: Vec<String>,

        /// Minimum number of measurements needed. If less, pass test and assume
        /// more measurements are needed.
        /// A minimum of two historic measurements are needed for proper evaluation of standard
        /// deviation.
        /// If specified on CLI, applies to ALL measurements (overrides config).
        /// If not specified, uses per-measurement config or defaults to 2.
        #[arg(long, value_parser=clap::value_parser!(u16).range(2..))]
        min_measurements: Option<u16>,

        /// What to aggregate the measurements in each group with.
        /// If not specified, uses the value from .gitperfconfig file, or defaults to min.
        #[arg(short, long)]
        aggregate_by: Option<ReductionFunc>,

        /// Multiple of the dispersion after which an outlier is detected.
        /// If the HEAD measurement is within the acceptable range based on this threshold,
        /// it is considered acceptable.
        /// If not specified, uses the value from .gitperfconfig file, or defaults to 4.0.
        #[arg(short = 'd', long)]
        sigma: Option<f64>,

        /// Method for calculating statistical dispersion. Choose between:
        ///
        /// **stddev**: Standard deviation - sensitive to outliers, use for normally
        /// distributed data where you want to detect all changes.
        ///
        /// **mad**: Median Absolute Deviation - robust to outliers, use when data
        /// has occasional spikes or you want to focus on typical changes.
        ///
        /// If not specified, uses the value from .gitperfconfig file, or defaults
        /// to stddev.
        #[arg(short = 'D', long, value_enum)]
        dispersion_method: Option<DispersionMethod>,
    },

    /// Accept HEAD commit's measurement for audit, even if outside of range.
    /// This is allows to accept expected performance changes.
    /// This is accomplished by starting a new epoch for the given measurement.
    /// The epoch is configured in the git perf config file.
    /// A change to the epoch therefore has to be committed and will result in a new HEAD for which
    /// new measurements have to be taken.
    BumpEpoch {
        #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string, required = true)]
        measurements: Vec<String>,
    },

    /// Remove all performance measurements for commits that have been committed
    /// at or before the specified time (inclusive boundary, uses <=).
    ///
    /// By default, this command automatically prunes orphaned measurements after
    /// removal (measurements for commits that no longer exist or are unreachable).
    /// Use --no-prune to skip this automatic cleanup.
    ///
    /// Note: Only published measurements (i.e., those that have been pushed to the
    /// remote repository) can be removed. Local unpublished measurements are not
    /// affected by this operation.
    Remove {
        #[arg(long = "older-than", value_parser = parse_datetime_value_now)]
        older_than: DateTime<Utc>,

        /// Skip automatic pruning of orphaned measurements after removal
        #[arg(long = "no-prune", default_value = "false")]
        no_prune: bool,
    },

    /// Remove all performance measurements for non-existent/unreachable objects.
    /// Will refuse to work if run on a shallow clone.
    Prune {},

    /// List all commits that have performance measurements.
    ///
    /// Outputs one commit SHA-1 hash per line. This can be used to identify
    /// which commits have measurements stored in the performance notes branch.
    ///
    /// Example:
    ///   git perf list-commits | wc -l  # Count commits with measurements
    ///   git perf list-commits | head   # Show first few commits
    ListCommits {},

    /// Estimate storage size of live performance measurements
    ///
    /// This command calculates the total size of performance measurement data
    /// stored in git notes (refs/notes/perf-v3). Use --detailed to see a
    /// breakdown by measurement name.
    ///
    /// By default, shows logical object sizes (uncompressed). Use --disk-size
    /// to see actual on-disk sizes accounting for compression.
    ///
    /// Examples:
    ///   git perf size                    # Show total size in human-readable format
    ///   git perf size --detailed         # Show breakdown by measurement name
    ///   git perf size --format bytes     # Show size in raw bytes
    ///   git perf size --disk-size        # Show actual on-disk sizes
    ///   git perf size --include-objects  # Include git repository statistics
    Size {
        /// Show detailed breakdown by measurement name
        #[arg(short, long)]
        detailed: bool,

        /// Output format (human-readable or bytes)
        #[arg(short, long, value_enum, default_value = "human")]
        format: SizeFormat,

        /// Use on-disk size (compressed) instead of logical size
        #[arg(long)]
        disk_size: bool,

        /// Include git repository statistics for context
        #[arg(long)]
        include_objects: bool,
    },
}

fn parse_key_value(s: &str) -> Result<(String, String)> {
    let pos = s
        .find('=')
        .ok_or_else(|| anyhow!("invalid key=value: no '=' found in '{}'", s))?;
    let key = parse_spaceless_string(&s[..pos])?;
    let value = parse_spaceless_string(&s[pos + 1..])?;
    Ok((key, value))
}

fn parse_spaceless_string(s: &str) -> Result<String> {
    if s.split_whitespace().count() > 1 {
        Err(anyhow!("invalid string/key/value: found space in '{}'", s))
    } else {
        Ok(String::from(s))
    }
}

fn parse_datetime_value_now(input: &str) -> Result<DateTime<Utc>> {
    parse_datetime_value(&Utc::now(), input)
}

fn parse_datetime_value(now: &DateTime<Utc>, input: &str) -> Result<DateTime<Utc>> {
    if input.len() < 2 {
        bail!("Invalid datetime format");
    }

    let (num, unit) = input.split_at(input.len() - 1);
    let num: i64 = num.parse()?;
    let subtractor = match unit {
        "w" => Duration::weeks(num),
        "d" => Duration::days(num),
        "h" => Duration::hours(num),
        _ => bail!("Unsupported datetime format"),
    };
    Ok(*now - subtractor)
}

#[cfg(test)]
mod test {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }

    #[test]
    fn verify_date_parsing() {
        let now = Utc::now();

        assert_eq!(
            now - Duration::weeks(2),
            parse_datetime_value(&now, "2w").unwrap()
        );

        assert_eq!(
            now - Duration::days(30),
            parse_datetime_value(&now, "30d").unwrap()
        );

        assert_eq!(
            now - Duration::hours(72),
            parse_datetime_value(&now, "72h").unwrap()
        );

        assert!(parse_datetime_value(&now, " 2w ").is_err());

        assert!(parse_datetime_value(&now, "").is_err());

        assert!(parse_datetime_value(&now, "945kjfg").is_err());
    }
}
