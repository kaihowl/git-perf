//! Import measurements from external test runners and benchmarks
//!
//! This module provides functionality to import measurements from various
//! test and benchmark formats into git-perf's measurement storage.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};

use git_perf_cli_types::ImportFormat;

use crate::config;
use crate::converters::{convert_to_measurements, ConversionOptions};
use crate::data::MeasurementData;
use crate::parsers::{CriterionJsonParser, JunitXmlParser, Parser};
use crate::serialization::serialize_multiple;

/// Handle the import command
///
/// Reads input from stdin or file, parses it according to the specified format,
/// converts to MeasurementData, and stores in git notes.
pub fn handle_import(
    format: ImportFormat,
    file: Option<String>,
    prefix: Option<String>,
    metadata: Vec<(String, String)>,
    filter: Option<String>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    // Read input from stdin or file
    let input = read_input(file.as_deref())?;

    // Select parser based on format
    let parsed = match format {
        ImportFormat::Junit => {
            let parser = JunitXmlParser;
            parser.parse(&input).context("Failed to parse JUnit XML")?
        }
        ImportFormat::CriterionJson => {
            let parser = CriterionJsonParser;
            parser
                .parse(&input)
                .context("Failed to parse criterion JSON")?
        }
    };

    if verbose {
        println!("Parsed {} measurements", parsed.len());
    }

    // Apply regex filter if specified
    let filtered = if let Some(filter_pattern) = filter {
        let regex = Regex::new(&filter_pattern).context("Invalid regex pattern for filter")?;

        let original_count = parsed.len();
        let filtered_parsed: Vec<_> = parsed
            .into_iter()
            .filter(|p| {
                let name = match p {
                    crate::parsers::ParsedMeasurement::Test(t) => &t.name,
                    crate::parsers::ParsedMeasurement::Benchmark(b) => &b.id,
                };
                regex.is_match(name)
            })
            .collect();

        if verbose {
            println!(
                "Filtered to {} measurements (from {}) using pattern: {}",
                filtered_parsed.len(),
                original_count,
                filter_pattern
            );
        }

        filtered_parsed
    } else {
        parsed
    };

    // Build conversion options
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get system time")?
        .as_secs_f64();

    let extra_metadata: HashMap<String, String> = metadata.into_iter().collect();

    let options = ConversionOptions {
        prefix,
        extra_metadata,
        epoch: 0, // Will be determined per-measurement in conversion
        timestamp,
    };

    // Convert to MeasurementData
    let measurements = convert_to_measurements(filtered, &options);

    if measurements.is_empty() {
        println!("No measurements to import (tests without durations are skipped)");
        return Ok(());
    }

    // Update epoch for each measurement based on config
    let measurements: Vec<MeasurementData> = measurements
        .into_iter()
        .map(|mut m| {
            m.epoch = config::determine_epoch_from_config(&m.name).unwrap_or(0);
            m
        })
        .collect();

    if verbose || dry_run {
        println!("\nMeasurements to import:");
        for m in &measurements {
            println!("  {} = {} (epoch: {})", m.name, m.val, m.epoch);
            if verbose {
                for (k, v) in &m.key_values {
                    println!("    {}: {}", k, v);
                }
            }
        }
        println!("\nTotal: {} measurements", measurements.len());
    }

    // Store measurements (unless dry run)
    if dry_run {
        println!("\n[DRY RUN] Measurements not stored");
    } else {
        store_measurements(&measurements)?;
        println!("Successfully imported {} measurements", measurements.len());
    }

    Ok(())
}

/// Read input from stdin or file
fn read_input(file: Option<&str>) -> Result<String> {
    match file {
        None | Some("-") => {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            Ok(buffer)
        }
        Some(path) => {
            // Read from file
            std::fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))
        }
    }
}

/// Store multiple measurements to git notes
///
/// This is similar to `measurement_storage::add_multiple` but handles
/// measurements with different names and metadata.
fn store_measurements(measurements: &[MeasurementData]) -> Result<()> {
    let serialized = serialize_multiple(measurements);
    crate::git::git_interop::add_note_line_to_head(&serialized)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_input_from_stdin() {
        // This test would require mocking stdin, so we skip it
        // Integration tests will cover stdin reading
    }

    #[test]
    fn test_read_input_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();

        let content = read_input(Some(file.path().to_str().unwrap())).unwrap();
        assert_eq!(content.trim(), "test content");
    }

    #[test]
    fn test_read_input_dash_is_stdin() {
        // Verify that "-" is treated as stdin (not a file named "-")
        // This would require mocking stdin, covered in integration tests
    }
}
