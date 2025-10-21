//! Converters for transforming parsed measurements into MeasurementData
//!
//! This module provides functionality to convert parsed test and benchmark
//! measurements into the `MeasurementData` format used by git-perf's storage layer.

use std::collections::HashMap;

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
/// Creates a single MeasurementData entry with:
/// - Name: `[prefix::]test::<test_name>`
/// - Value: duration in seconds (0.0 if missing)
/// - Metadata: type=test, status, classname (if present), plus extra metadata
fn convert_test(test: TestMeasurement, options: &ConversionOptions) -> Vec<MeasurementData> {
    let name = format_measurement_name("test", &test.name, None, options);

    // Convert duration to seconds (default to 0.0 if missing)
    let val = test.duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);

    // Build metadata
    let mut key_values = HashMap::new();
    key_values.insert("type".to_string(), "test".to_string());
    key_values.insert("status".to_string(), test.status.as_str().to_string());

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

/// Convert a benchmark measurement to MeasurementData
///
/// Creates multiple MeasurementData entries (one per statistic):
/// - Name: `[prefix::]bench::<bench_id>::<statistic>`
/// - Value: statistic value in seconds (converted from nanoseconds)
/// - Metadata: type=bench, group, bench_name, input (if present), statistic, plus extra metadata
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
    let create_measurement = |stat_name: &str, value_ns: Option<f64>| -> Option<MeasurementData> {
        value_ns.map(|ns| {
            let name = format_measurement_name("bench", &bench.id, Some(stat_name), options);

            // Convert nanoseconds to seconds
            let val = ns / 1_000_000_000.0;

            let mut key_values = HashMap::new();
            key_values.insert("type".to_string(), "bench".to_string());
            key_values.insert("group".to_string(), group.to_string());
            key_values.insert("bench_name".to_string(), bench_name.to_string());
            if let Some(input_val) = input {
                key_values.insert("input".to_string(), input_val.to_string());
            }
            key_values.insert("statistic".to_string(), stat_name.to_string());

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
                val,
                key_values,
            }
        })
    };

    // Create measurements for available statistics
    if let Some(m) = create_measurement("mean", bench.statistics.mean_ns) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("median", bench.statistics.median_ns) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("slope", bench.statistics.slope_ns) {
        measurements.push(m);
    }
    if let Some(m) = create_measurement("mad", bench.statistics.mad_ns) {
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
    fn test_convert_test_passed() {
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
        assert_eq!(result.len(), 1);

        let measurement = &result[0];
        assert_eq!(measurement.name, "test::test_one");
        assert_eq!(measurement.val, 1.5);
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
        assert_eq!(
            measurement.key_values.get("classname"),
            Some(&"module::tests".to_string())
        );
    }

    #[test]
    fn test_convert_test_missing_duration() {
        let test = TestMeasurement {
            name: "test_skipped".to_string(),
            duration: None,
            status: TestStatus::Skipped,
            metadata: HashMap::new(),
        };

        let options = ConversionOptions::default();
        let result = convert_test(test, &options);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].val, 0.0);
        assert_eq!(
            result[0].key_values.get("status"),
            Some(&"skipped".to_string())
        );
    }

    #[test]
    fn test_convert_test_with_extra_metadata() {
        let test = TestMeasurement {
            name: "test_ci".to_string(),
            duration: Some(Duration::from_secs_f64(2.0)),
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
        assert_eq!(result[0].key_values.get("ci"), Some(&"true".to_string()));
        assert_eq!(
            result[0].key_values.get("branch"),
            Some(&"main".to_string())
        );
    }

    #[test]
    fn test_convert_benchmark_all_statistics() {
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

        // Check mean measurement
        let mean = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::mean")
            .unwrap();
        assert_eq!(mean.val, 15000.0 / 1_000_000_000.0);
        assert_eq!(mean.key_values.get("type"), Some(&"bench".to_string()));
        assert_eq!(mean.key_values.get("group"), Some(&"group".to_string()));
        assert_eq!(
            mean.key_values.get("bench_name"),
            Some(&"bench_name".to_string())
        );
        assert_eq!(mean.key_values.get("input"), Some(&"100".to_string()));
        assert_eq!(mean.key_values.get("statistic"), Some(&"mean".to_string()));

        // Check median measurement
        let median = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::median")
            .unwrap();
        assert_eq!(median.val, 14500.0 / 1_000_000_000.0);
        assert_eq!(
            median.key_values.get("statistic"),
            Some(&"median".to_string())
        );

        // Check slope measurement
        let slope = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::slope")
            .unwrap();
        assert_eq!(slope.val, 15200.0 / 1_000_000_000.0);
        assert_eq!(
            slope.key_values.get("statistic"),
            Some(&"slope".to_string())
        );

        // Check MAD measurement
        let mad = result
            .iter()
            .find(|m| m.name == "bench::group/bench_name/100::mad")
            .unwrap();
        assert_eq!(mad.val, 100.0 / 1_000_000_000.0);
        assert_eq!(mad.key_values.get("statistic"), Some(&"mad".to_string()));
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
    }

    #[test]
    fn test_convert_to_measurements_mixed() {
        let parsed = vec![
            ParsedMeasurement::Test(TestMeasurement {
                name: "test_one".to_string(),
                duration: Some(Duration::from_secs(1)),
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

        // 1 test + 2 bench statistics = 3 measurements
        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|m| m.name == "test::test_one"));
        assert!(result.iter().any(|m| m.name == "bench::group/bench::mean"));
        assert!(result
            .iter()
            .any(|m| m.name == "bench::group/bench::median"));
    }

    #[test]
    fn test_convert_with_prefix() {
        let parsed = vec![ParsedMeasurement::Test(TestMeasurement {
            name: "my_test".to_string(),
            duration: Some(Duration::from_secs(1)),
            status: TestStatus::Passed,
            metadata: HashMap::new(),
        })];

        let mut options = ConversionOptions::default();
        options.prefix = Some("ci".to_string());

        let result = convert_to_measurements(parsed, &options);
        assert_eq!(result[0].name, "ci::test::my_test");
    }

    #[test]
    fn test_nanoseconds_to_seconds_conversion() {
        let bench = BenchmarkMeasurement {
            id: "group/bench".to_string(),
            statistics: BenchStatistics {
                mean_ns: Some(1_500_000_000.0), // 1.5 seconds in nanoseconds
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
        assert_eq!(result[0].val, 1.5); // Should be converted to seconds
    }
}
