//! Converters for transforming parsed measurements into MeasurementData
//!
//! This module provides functionality to convert parsed test and benchmark
//! measurements into the `MeasurementData` format used by git-perf's storage layer.

use std::collections::HashMap;

use crate::config;
use crate::data::MeasurementData;
use crate::parsers::{BenchmarkMeasurement, ParsedMeasurement, TestMeasurement};

/// Options for converting parsed measurements to MeasurementData
#[derive(Debug, Clone)]
pub struct ConversionOptions {
    /// Optional prefix to prepend to measurement names
    pub prefix: Option<String>,
    /// Extra metadata to add to all measurements
    pub extra_metadata: HashMap<String, String>,
    /// Epoch value for measurements
    pub epoch: u32,
    /// Timestamp for measurements (seconds since UNIX epoch)
    pub timestamp: f64,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            prefix: None,
            extra_metadata: HashMap::new(),
            epoch: 0,
            timestamp: 0.0,
        }
    }
}

/// Convert parsed measurements to MeasurementData
///
/// This function takes a list of parsed measurements and converts them to
/// the MeasurementData format, applying the specified conversion options.
///
/// **Test Measurements:**
/// - Only converted if duration is present (tests with performance data)
/// - Tests WITHOUT duration are skipped (no performance to track)
/// - Value stored in nanoseconds for consistency with benchmarks
/// - Unit stored in metadata as "ns"
///
/// **Benchmark Measurements:**
/// - Value stored in nanoseconds (converts us/ms/s → ns)
/// - Creates one measurement per statistic (mean, median, slope, MAD)
/// - Unit validation warnings logged for mismatches with config
///
/// # Arguments
///
/// * `parsed` - Vector of parsed measurements to convert
/// * `options` - Conversion options (prefix, metadata, epoch, timestamp)
///
/// # Returns
///
/// A vector of MeasurementData ready for storage
pub fn convert_to_measurements(
    parsed: Vec<ParsedMeasurement>,
    options: &ConversionOptions,
) -> Vec<MeasurementData> {
    parsed
        .into_iter()
        .flat_map(|p| match p {
            ParsedMeasurement::Test(test) => convert_test(test, options),
            ParsedMeasurement::Benchmark(bench) => convert_benchmark(bench, options),
        })
        .collect()
}

/// Convert a test measurement to MeasurementData
///
/// **IMPORTANT**: Only converts tests that HAVE a duration.
/// Tests without durations are skipped entirely (returns empty vec).
///
/// This is because:
/// - We want to track test performance over time
/// - Tests without duration (failed/skipped before execution) have no performance data
/// - Duration is stored in nanoseconds for consistency with benchmarks
///
/// Creates a single MeasurementData entry with:
/// - Name: `[prefix::]test::<test_name>`
/// - Value: duration in nanoseconds
/// - Metadata: type=test, status, unit=ns, classname (if present), plus extra metadata
fn convert_test(test: TestMeasurement, options: &ConversionOptions) -> Vec<MeasurementData> {
    // Skip tests that don't have duration - no performance data to track
    let Some(duration) = test.duration else {
        return vec![];
    };

    let name = format_measurement_name("test", &test.name, None, options);

    // Convert duration to nanoseconds for consistency with benchmarks
    let val = duration.as_nanos() as f64;

    // Validate unit consistency with config
    validate_unit(&name, "ns");

    // Build metadata
    let mut key_values = HashMap::new();
    key_values.insert("type".to_string(), "test".to_string());
    key_values.insert("status".to_string(), test.status.as_str().to_string());
    key_values.insert("unit".to_string(), "ns".to_string());

    // Add test's own metadata (like classname)
    for (k, v) in test.metadata {
        key_values.insert(k, v);
    }

    // Add extra metadata from options
    for (k, v) in &options.extra_metadata {
        key_values.insert(k.clone(), v.clone());
    }

    vec![MeasurementData {
        epoch: options.epoch,
        name,
        timestamp: options.timestamp,
        val,
        key_values,
    }]
}

/// Convert benchmark unit to a standard representation and value
///
/// Criterion reports units as: "ns", "us", "ms", "s"
/// We convert the value to the base unit and normalize the unit string.
///
/// Returns (value, normalized_unit_string)
fn convert_benchmark_unit(value: f64, unit: &str) -> (f64, String) {
    match unit.to_lowercase().as_str() {
        "ns" => (value, "ns".to_string()),
        "us" | "μs" => (value * 1_000.0, "ns".to_string()), // Convert to ns
        "ms" => (value * 1_000_000.0, "ns".to_string()),    // Convert to ns
        "s" => (value * 1_000_000_000.0, "ns".to_string()), // Convert to ns
        _ => {
            // Unknown unit - preserve as-is and warn
            log::warn!("Unknown benchmark unit '{}', storing value as-is", unit);
            (value, unit.to_string())
        }
    }
}

/// Validate unit consistency with config and log warnings if needed
fn validate_unit(measurement_name: &str, unit: &str) {
    if let Some(configured_unit) = config::measurement_unit(measurement_name) {
        if configured_unit != unit {
            log::warn!(
                "Unit mismatch for '{}': importing '{}' but config specifies '{}'. \
                 Consider updating .gitperfconfig to match.",
                measurement_name,
                unit,
                configured_unit
            );
        }
    } else {
        log::info!(
            "No unit configured for '{}'. Importing with unit '{}'. \
             Consider adding to .gitperfconfig: [measurement.\"{}\"]\nunit = \"{}\"",
            measurement_name,
            unit,
            measurement_name,
            unit
        );
    }
}

/// Convert a benchmark measurement to MeasurementData
///
/// Creates multiple MeasurementData entries (one per statistic):
/// - Name: `[prefix::]bench::<bench_id>::<statistic>`
/// - Value: statistic value in ORIGINAL UNIT from criterion
/// - Unit: Normalized to "ns" for time-based benchmarks
/// - Metadata: type=bench, group, bench_name, input (if present), statistic, unit, plus extra metadata
///
/// **Unit Handling:**
/// - Preserves the unit from criterion's output
/// - Normalizes time units to "ns" (converts us/ms/s → ns)
/// - Stores unit in metadata for validation and display
/// - Validates against configured unit and logs warnings
fn convert_benchmark(
    bench: BenchmarkMeasurement,
    options: &ConversionOptions,
) -> Vec<MeasurementData> {
    let mut measurements = Vec::new();

    // Parse benchmark ID to extract group, name, and optional input
    // Format: "group/name/input" or "group/name"
    let parts: Vec<&str> = bench.id.split('/').collect();
    let (group, bench_name, input) = match parts.len() {
        2 => (parts[0], parts[1], None),
        3 => (parts[0], parts[1], Some(parts[2])),
        _ => {
            // Fallback: use full ID as bench_name
            ("unknown", bench.id.as_str(), None)
        }
    };

    // Helper to create a measurement for a specific statistic
    let create_measurement =
        |stat_name: &str, value: Option<f64>, unit: &str| -> Option<MeasurementData> {
            value.map(|v| {
                let name = format_measurement_name("bench", &bench.id, Some(stat_name), options);

                // Convert value and normalize unit
                let (converted_value, normalized_unit) = convert_benchmark_unit(v, unit);

                // Validate unit consistency with config
                validate_unit(&name, &normalized_unit);

                let mut key_values = HashMap::new();
                key_values.insert("type".to_string(), "bench".to_string());
                key_values.insert("group".to_string(), group.to_string());
                key_values.insert("bench_name".to_string(), bench_name.to_string());
                if let Some(input_val) = input {
                    key_values.insert("input".to_string(), input_val.to_string());
                }
                key_values.insert("statistic".to_string(), stat_name.to_string());
                key_values.insert("unit".to_string(), normalized_unit);

                // Add benchmark's own metadata
                for (k, v) in &bench.metadata {
                    key_values.insert(k.clone(), v.clone());
                }

                // Add extra metadata from options
                for (k, v) in &options.extra_metadata {
                    key_values.insert(k.clone(), v.clone());
                }

                MeasurementData {
                    epoch: options.epoch,
                    name,
                    timestamp: options.timestamp,
                    val: converted_value,
                    key_values,
                }
            })
        };

    // Create measurements for available statistics
    let unit = &bench.statistics.unit;
    if let Some(m) = create_measurement("mean", bench.statistics.mean_ns, unit) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("median", bench.statistics.median_ns, unit) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("slope", bench.statistics.slope_ns, unit) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("mad", bench.statistics.mad_ns, unit) {
        measurements.push(m);
    }

    measurements
}

/// Format a measurement name with optional prefix and suffix
///
/// # Arguments
///
/// * `type_prefix` - "test" or "bench"
/// * `id` - The test/benchmark identifier
/// * `suffix` - Optional suffix (e.g., statistic name for benchmarks)
/// * `options` - Conversion options (for user-provided prefix)
///
/// # Returns
///
/// Formatted name like: `[prefix::]type_prefix::id[::suffix]`
fn format_measurement_name(
    type_prefix: &str,
    id: &str,
    suffix: Option<&str>,
    options: &ConversionOptions,
) -> String {
    let mut parts = Vec::new();

    if let Some(prefix) = &options.prefix {
        parts.push(prefix.as_str());
    }

    parts.push(type_prefix);
    parts.push(id);

    if let Some(s) = suffix {
        parts.push(s);
    }

    parts.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{BenchStatistics, TestStatus};
    use std::time::Duration;

    #[test]
    fn test_format_measurement_name_no_prefix_no_suffix() {
        let options = ConversionOptions::default();
        let name = format_measurement_name("test", "my_test", None, &options);
        assert_eq!(name, "test::my_test");
    }

    #[test]
    fn test_format_measurement_name_with_prefix() {
        let mut options = ConversionOptions::default();
        options.prefix = Some("custom".to_string());
        let name = format_measurement_name("test", "my_test", None, &options);
        assert_eq!(name, "custom::test::my_test");
    }

    #[test]
    fn test_format_measurement_name_with_suffix() {
        let options = ConversionOptions::default();
        let name = format_measurement_name("bench", "my_bench", Some("mean"), &options);
        assert_eq!(name, "bench::my_bench::mean");
    }

    #[test]
    fn test_format_measurement_name_with_prefix_and_suffix() {
        let mut options = ConversionOptions::default();
        options.prefix = Some("perf".to_string());
        let name = format_measurement_name("bench", "my_bench", Some("median"), &options);
        assert_eq!(name, "perf::bench::my_bench::median");
    }

    #[test]
    fn test_convert_test_with_duration() {
        // Tests WITH duration should be converted
        let test = TestMeasurement {
            name: "test_one".to_string(),
            duration: Some(Duration::from_secs_f64(1.5)),
            status: TestStatus::Passed,
            metadata: {
                let mut map = HashMap::new();
                map.insert("classname".to_string(), "module::tests".to_string());
                map
            },
        };

        let options = ConversionOptions {
            epoch: 1,
            timestamp: 1234567890.0,
            prefix: None,
            extra_metadata: HashMap::new(),
        };

        let result = convert_test(test, &options);
        // Should convert test with duration
        assert_eq!(result.len(), 1);

        let measurement = &result[0];
        assert_eq!(measurement.name, "test::test_one");
        // 1.5 seconds = 1,500,000,000 nanoseconds
        assert_eq!(measurement.val, 1_500_000_000.0);
        assert_eq!(measurement.epoch, 1);
        assert_eq!(measurement.timestamp, 1234567890.0);
        assert_eq!(
            measurement.key_values.get("type"),
            Some(&"test".to_string())
        );
        assert_eq!(
            measurement.key_values.get("status"),
            Some(&"passed".to_string())
        );
        assert_eq!(measurement.key_values.get("unit"), Some(&"ns".to_string()));
        assert_eq!(
            measurement.key_values.get("classname"),
            Some(&"module::tests".to_string())
        );
    }

    #[test]
    fn test_convert_test_without_duration_is_skipped() {
        // Tests WITHOUT duration should be skipped
        let test = TestMeasurement {
            name: "test_skipped".to_string(),
            duration: None,
            status: TestStatus::Skipped,
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_test(test, &options);

        // Should return empty vec - test without duration is skipped
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_convert_test_failed_without_duration_is_skipped() {
        let test = TestMeasurement {
            name: "test_failed".to_string(),
            duration: None,
            status: TestStatus::Failed,
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_test(test, &options);

        // Should be skipped - no duration available
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_convert_test_with_extra_metadata() {
        let test = TestMeasurement {
            name: "test_ci".to_string(),
            duration: Some(Duration::from_millis(250)), // Add duration so test is converted
            status: TestStatus::Passed,
            metadata: HashMap::new(),
        };

        let mut extra_metadata = HashMap::new();
        extra_metadata.insert("ci".to_string(), "true".to_string());
        extra_metadata.insert("branch".to_string(), "main".to_string());

        let options = ConversionOptions {
            extra_metadata,
            ..Default::default()
        };

        let result = convert_test(test, &options);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key_values.get("ci"), Some(&"true".to_string()));
        assert_eq!(
            result[0].key_values.get("branch"),
            Some(&"main".to_string())
        );
        assert_eq!(result[0].key_values.get("unit"), Some(&"ns".to_string()));
        // 250 ms = 250,000,000 ns
        assert_eq!(result[0].val, 250_000_000.0);
    }

    #[test]
    fn test_convert_benchmark_all_statistics_nanoseconds() {
        let bench = BenchmarkMeasurement {
            id: "group/bench_name/100".to_string(),
            statistics: BenchStatistics {
                mean_ns: Some(15000.0),
                median_ns: Some(14500.0),
                slope_ns: Some(15200.0),
                mad_ns: Some(100.0),
                unit: "ns".to_string(),
            },
            metadata: HashMap::new(),
        };

        let options = ConversionOptions {
            epoch: 2,
            timestamp: 9876543210.0,
            ..Default::default()
        };

        let result = convert_benchmark(bench, &options);
        assert_eq!(result.len(), 4);

        // Check mean measurement - should be in nanoseconds
        let mean = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::mean")
            .unwrap();
        assert_eq!(mean.val, 15000.0); // Stored in nanoseconds
        assert_eq!(mean.key_values.get("type"), Some(&"bench".to_string()));
        assert_eq!(mean.key_values.get("group"), Some(&"group".to_string()));
        assert_eq!(
            mean.key_values.get("bench_name"),
            Some(&"bench_name".to_string())
        );
        assert_eq!(mean.key_values.get("input"), Some(&"100".to_string()));
        assert_eq!(mean.key_values.get("statistic"), Some(&"mean".to_string()));
        assert_eq!(mean.key_values.get("unit"), Some(&"ns".to_string()));

        // Check median measurement
        let median = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::median")
            .unwrap();
        assert_eq!(median.val, 14500.0); // Stored in nanoseconds
        assert_eq!(
            median.key_values.get("statistic"),
            Some(&"median".to_string())
        );
        assert_eq!(median.key_values.get("unit"), Some(&"ns".to_string()));
    }

    #[test]
    fn test_convert_benchmark_unit_conversion() {
        // Test microseconds converted to nanoseconds
        let (val, unit) = convert_benchmark_unit(15.5, "us");
        assert_eq!(val, 15500.0); // 15.5 us = 15500 ns
        assert_eq!(unit, "ns");

        // Test milliseconds converted to nanoseconds
        let (val, unit) = convert_benchmark_unit(2.5, "ms");
        assert_eq!(val, 2_500_000.0); // 2.5 ms = 2,500,000 ns
        assert_eq!(unit, "ns");

        // Test seconds converted to nanoseconds
        let (val, unit) = convert_benchmark_unit(1.5, "s");
        assert_eq!(val, 1_500_000_000.0); // 1.5 s = 1,500,000,000 ns
        assert_eq!(unit, "ns");

        // Test nanoseconds preserved
        let (val, unit) = convert_benchmark_unit(1000.0, "ns");
        assert_eq!(val, 1000.0);
        assert_eq!(unit, "ns");
    }

    #[test]
    fn test_convert_benchmark_partial_statistics() {
        let bench = BenchmarkMeasurement {
            id: "group/bench_name".to_string(),
            statistics: BenchStatistics {
                mean_ns: Some(10000.0),
                median_ns: None,
                slope_ns: Some(10500.0),
                mad_ns: None,
                unit: "ns".to_string(),
            },
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_benchmark(bench, &options);

        // Only mean and slope should be present
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .any(|m| m.name == "bench::group/bench_name::mean"));
        assert!(result
            .iter()
            .any(|m| m.name == "bench::group/bench_name::slope"));

        // Verify unit is stored
        assert!(result
            .iter()
            .all(|m| m.key_values.get("unit") == Some(&"ns".to_string())));
    }

    #[test]
    fn test_convert_benchmark_no_input() {
        let bench = BenchmarkMeasurement {
            id: "my_group/my_bench".to_string(),
            statistics: BenchStatistics {
                mean_ns: Some(5000.0),
                median_ns: None,
                slope_ns: None,
                mad_ns: None,
                unit: "ns".to_string(),
            },
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_benchmark(bench, &options);

        assert_eq!(result.len(), 1);
        let measurement = &result[0];
        assert_eq!(
            measurement.key_values.get("group"),
            Some(&"my_group".to_string())
        );
        assert_eq!(
            measurement.key_values.get("bench_name"),
            Some(&"my_bench".to_string())
        );
        assert_eq!(measurement.key_values.get("input"), None);
        assert_eq!(measurement.key_values.get("unit"), Some(&"ns".to_string()));
    }

    #[test]
    fn test_convert_to_measurements_mixed() {
        let parsed = vec![
            ParsedMeasurement::Test(TestMeasurement {
                name: "test_one".to_string(),
                duration: Some(Duration::from_millis(100)), // Has duration so it gets converted
                status: TestStatus::Passed,
                metadata: HashMap::new(),
            }),
            ParsedMeasurement::Benchmark(BenchmarkMeasurement {
                id: "group/bench".to_string(),
                statistics: BenchStatistics {
                    mean_ns: Some(1000.0),
                    median_ns: Some(900.0),
                    slope_ns: None,
                    mad_ns: None,
                    unit: "ns".to_string(),
                },
                metadata: HashMap::new(),
            }),
        ];

        let options = ConversionOptions::default();
        let result = convert_to_measurements(parsed, &options);

        // 1 test (with duration) + 2 bench statistics = 3 measurements
        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|m| m.name == "test::test_one"));
        assert!(result.iter().any(|m| m.name == "bench::group/bench::mean"));
        assert!(result
            .iter()
            .any(|m| m.name == "bench::group/bench::median"));
    }

    #[test]
    fn test_convert_to_measurements_skips_tests_without_duration() {
        let parsed = vec![
            ParsedMeasurement::Test(TestMeasurement {
                name: "test_passing".to_string(),
                duration: Some(Duration::from_secs(1)), // Has duration - should be converted
                status: TestStatus::Passed,
                metadata: HashMap::new(),
            }),
            ParsedMeasurement::Test(TestMeasurement {
                name: "test_failed".to_string(),
                duration: None, // No duration - should be skipped
                status: TestStatus::Failed,
                metadata: HashMap::new(),
            }),
        ];

        let options = ConversionOptions::default();
        let result = convert_to_measurements(parsed, &options);

        // Only the passing test (with duration) should be converted
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test::test_passing");
        // 1 second = 1,000,000,000 nanoseconds
        assert_eq!(result[0].val, 1_000_000_000.0);
        assert_eq!(result[0].key_values.get("unit"), Some(&"ns".to_string()));
    }

    #[test]
    fn test_convert_with_prefix() {
        let parsed = vec![ParsedMeasurement::Test(TestMeasurement {
            name: "my_test".to_string(),
            duration: Some(Duration::from_millis(50)), // Add duration so test is converted
            status: TestStatus::Passed,
            metadata: HashMap::new(),
        })];

        let mut options = ConversionOptions::default();
        options.prefix = Some("ci".to_string());

        let result = convert_to_measurements(parsed, &options);
        assert_eq!(result[0].name, "ci::test::my_test");
        assert_eq!(result[0].val, 50_000_000.0); // 50 ms in nanoseconds
    }

    #[test]
    fn test_benchmark_preserves_unit() {
        let bench = BenchmarkMeasurement {
            id: "group/bench".to_string(),
            statistics: BenchStatistics {
                mean_ns: Some(1500.0), // 1500 microseconds
                median_ns: None,
                slope_ns: None,
                mad_ns: None,
                unit: "us".to_string(), // Microseconds
            },
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_benchmark(bench, &options);

        assert_eq!(result.len(), 1);
        // Value should be converted to nanoseconds: 1500 us = 1,500,000 ns
        assert_eq!(result[0].val, 1_500_000.0);
        // Unit should be normalized to ns
        assert_eq!(result[0].key_values.get("unit"), Some(&"ns".to_string()));
    }
}
