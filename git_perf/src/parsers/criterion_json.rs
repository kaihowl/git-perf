use crate::parsers::types::{BenchStatistics, BenchmarkMeasurement, ParsedMeasurement, Parser};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// Parser for cargo-criterion JSON format (line-delimited JSON)
pub struct CriterionJsonParser;

impl Parser for CriterionJsonParser {
    fn parse(&self, input: &str) -> Result<Vec<ParsedMeasurement>> {
        let mut measurements = Vec::new();

        for (line_num, line) in input.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let message: CriterionMessage = serde_json::from_str(line)
                .with_context(|| format!("Failed to parse JSON on line {}", line_num + 1))?;

            // Only process benchmark-complete messages
            if message.reason == "benchmark-complete" {
                if let Some(measurement) = message.into_measurement()? {
                    measurements.push(measurement);
                }
            }
        }

        Ok(measurements)
    }
}

#[derive(Debug, Deserialize)]
struct CriterionMessage {
    reason: String,
    #[serde(default)]
    id: String,
    group: Option<String>,
    unit: Option<String>,
    mean: Option<Estimate>,
    median: Option<Estimate>,
    slope: Option<Estimate>,
    median_abs_dev: Option<Estimate>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Estimate {
    estimate: f64,
    lower_bound: Option<f64>,
    upper_bound: Option<f64>,
}

impl CriterionMessage {
    fn into_measurement(self) -> Result<Option<ParsedMeasurement>> {
        if self.reason != "benchmark-complete" {
            return Ok(None);
        }

        let unit = self.unit.unwrap_or_else(|| "ns".to_string());

        // Extract group, name, and input from the benchmark ID
        // Format: "group/name/input" or "group/name" or just "name"
        // Prefer the explicit group field from criterion if available
        let (parsed_group, bench_name, input) = parse_benchmark_id(&self.id);
        let group = self.group.or(parsed_group);

        let statistics = BenchStatistics {
            mean_ns: self.mean.map(|e| convert_to_nanoseconds(e.estimate, &unit)),
            median_ns: self
                .median
                .map(|e| convert_to_nanoseconds(e.estimate, &unit)),
            slope_ns: self
                .slope
                .map(|e| convert_to_nanoseconds(e.estimate, &unit)),
            mad_ns: self
                .median_abs_dev
                .map(|e| convert_to_nanoseconds(e.estimate, &unit)),
            unit,
        };

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "bench".to_string());
        if let Some(g) = group {
            metadata.insert("group".to_string(), g);
        }
        if let Some(n) = bench_name {
            metadata.insert("bench_name".to_string(), n);
        }
        if let Some(i) = input {
            metadata.insert("input".to_string(), i);
        }

        Ok(Some(ParsedMeasurement::Benchmark(BenchmarkMeasurement {
            id: self.id,
            statistics,
            metadata,
        })))
    }
}

/// Parse benchmark ID into group, name, and input components
/// Examples:
///   "add_measurements/add_measurement/50" -> (Some("add_measurements"), Some("add_measurement"), Some("50"))
///   "add_measurements/add_measurement" -> (Some("add_measurements"), Some("add_measurement"), None)
///   "simple_bench" -> (None, Some("simple_bench"), None)
fn parse_benchmark_id(id: &str) -> (Option<String>, Option<String>, Option<String>) {
    let parts: Vec<&str> = id.split('/').collect();

    match parts.len() {
        0 => (None, None, None),
        1 => (None, Some(parts[0].to_string()), None),
        2 => (Some(parts[0].to_string()), Some(parts[1].to_string()), None),
        _ => (
            Some(parts[0].to_string()),
            Some(parts[1].to_string()),
            Some(parts[2..].join("/")),
        ),
    }
}

/// Convert criterion measurement to nanoseconds
fn convert_to_nanoseconds(value: f64, unit: &str) -> f64 {
    match unit {
        "ns" => value,
        "us" => value * 1_000.0,
        "ms" => value * 1_000_000.0,
        "s" => value * 1_000_000_000.0,
        _ => value, // Assume nanoseconds if unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_benchmark_complete() {
        let json = r#"{"reason":"benchmark-complete","id":"add_measurements/add_measurement/50","unit":"ns","mean":{"estimate":15456.78,"lower_bound":15234.0,"upper_bound":15678.5},"median":{"estimate":15400.0},"slope":{"estimate":15420.5},"median_abs_dev":{"estimate":123.45}}"#;

        let parser = CriterionJsonParser;
        let result = parser.parse(json).unwrap();

        assert_eq!(result.len(), 1);

        if let ParsedMeasurement::Benchmark(bench) = &result[0] {
            assert_eq!(bench.id, "add_measurements/add_measurement/50");
            assert_eq!(bench.statistics.unit, "ns");
            assert_eq!(bench.statistics.mean_ns, Some(15456.78));
            assert_eq!(bench.statistics.median_ns, Some(15400.0));
            assert_eq!(bench.statistics.slope_ns, Some(15420.5));
            assert_eq!(bench.statistics.mad_ns, Some(123.45));
            assert_eq!(bench.metadata.get("group").unwrap(), "add_measurements");
            assert_eq!(bench.metadata.get("bench_name").unwrap(), "add_measurement");
            assert_eq!(bench.metadata.get("input").unwrap(), "50");
        } else {
            panic!("Expected Benchmark measurement");
        }
    }

    #[test]
    fn test_parse_multiple_lines() {
        let json = r#"{"reason":"group-start","group":"fibonacci"}
{"reason":"benchmark-complete","id":"fibonacci_10","unit":"us","mean":{"estimate":1.234}}
{"reason":"benchmark-complete","id":"fibonacci_20","unit":"us","mean":{"estimate":56.789}}
{"reason":"group-complete","group":"fibonacci"}"#;

        let parser = CriterionJsonParser;
        let result = parser.parse(json).unwrap();

        assert_eq!(result.len(), 2);

        if let ParsedMeasurement::Benchmark(bench) = &result[0] {
            assert_eq!(bench.id, "fibonacci_10");
            // us to ns conversion
            assert_eq!(bench.statistics.mean_ns, Some(1234.0));
        } else {
            panic!("Expected Benchmark measurement");
        }
    }

    #[test]
    fn test_parse_benchmark_id_three_parts() {
        let (group, name, input) = parse_benchmark_id("add_measurements/add_measurement/50");
        assert_eq!(group, Some("add_measurements".to_string()));
        assert_eq!(name, Some("add_measurement".to_string()));
        assert_eq!(input, Some("50".to_string()));
    }

    #[test]
    fn test_parse_benchmark_id_two_parts() {
        let (group, name, input) = parse_benchmark_id("fibonacci/fib_10");
        assert_eq!(group, Some("fibonacci".to_string()));
        assert_eq!(name, Some("fib_10".to_string()));
        assert_eq!(input, None);
    }

    #[test]
    fn test_parse_benchmark_id_one_part() {
        let (group, name, input) = parse_benchmark_id("simple_bench");
        assert_eq!(group, None);
        assert_eq!(name, Some("simple_bench".to_string()));
        assert_eq!(input, None);
    }

    #[test]
    fn test_convert_units() {
        assert_eq!(convert_to_nanoseconds(1.0, "ns"), 1.0);
        assert_eq!(convert_to_nanoseconds(1.0, "us"), 1000.0);
        assert_eq!(convert_to_nanoseconds(1.0, "ms"), 1_000_000.0);
        assert_eq!(convert_to_nanoseconds(1.0, "s"), 1_000_000_000.0);
    }

    #[test]
    fn test_parse_empty_lines() {
        let json = r#"
{"reason":"benchmark-complete","id":"test","unit":"ns","mean":{"estimate":100.0}}

{"reason":"benchmark-complete","id":"test2","unit":"ns","mean":{"estimate":200.0}}
"#;

        let parser = CriterionJsonParser;
        let result = parser.parse(json).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json";
        let parser = CriterionJsonParser;
        assert!(parser.parse(json).is_err());
    }

    #[test]
    fn test_parse_missing_unit_defaults_to_ns() {
        let json = r#"{"reason":"benchmark-complete","id":"test","mean":{"estimate":15456.78}}"#;

        let parser = CriterionJsonParser;
        let result = parser.parse(json).unwrap();

        if let ParsedMeasurement::Benchmark(bench) = &result[0] {
            assert_eq!(bench.statistics.unit, "ns");
            assert_eq!(bench.statistics.mean_ns, Some(15456.78));
        } else {
            panic!("Expected Benchmark measurement");
        }
    }
}
