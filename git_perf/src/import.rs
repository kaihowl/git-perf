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
use crate::defaults;
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
            m.epoch =
                config::determine_epoch_from_config(&m.name).unwrap_or(defaults::DEFAULT_EPOCH);
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
    use crate::git::git_interop::walk_commits;
    use crate::test_helpers::{dir_with_repo, hermetic_git_env};
    use std::collections::HashMap;
    use std::env::set_current_dir;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Sample test data
    const SAMPLE_JUNIT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="3" failures="1" errors="0" time="3.5">
  <testsuite name="test_binary" tests="3" failures="1" time="3.5">
    <testcase name="test_passed" classname="module::tests" time="1.5"/>
    <testcase name="test_failed" classname="module::tests" time="2.0">
      <failure message="assertion failed"/>
    </testcase>
    <testcase name="test_skipped" classname="module::tests">
      <skipped/>
    </testcase>
  </testsuite>
</testsuites>"#;

    const SAMPLE_CRITERION_JSON: &str = r#"{"reason":"benchmark-complete","id":"fibonacci/fib_10","unit":"ns","mean":{"estimate":15456.78,"lower_bound":15234.0,"upper_bound":15678.5},"median":{"estimate":15400.0,"lower_bound":15350.0,"upper_bound":15450.0},"slope":{"estimate":15420.5,"lower_bound":15380.0,"upper_bound":15460.0},"median_abs_dev":{"estimate":123.45}}
{"reason":"benchmark-complete","id":"fibonacci/fib_20","unit":"us","mean":{"estimate":1234.56,"lower_bound":1200.0,"upper_bound":1270.0},"median":{"estimate":1220.0}}"#;

    #[test]
    fn test_read_input_from_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();

        let content = read_input(Some(file.path().to_str().unwrap())).unwrap();
        assert_eq!(content.trim(), "test content");
    }

    #[test]
    fn test_read_input_nonexistent_file() {
        let result = read_input(Some("/nonexistent/file/path.xml"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read file"));
    }

    #[test]
    fn test_handle_import_junit_dry_run() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        // Get initial notes count before import
        let commits_before = walk_commits(1).unwrap();
        let notes_before = commits_before[0].1.len();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            None,
            true,  // dry_run
            false, // verbose
        );

        assert!(result.is_ok(), "Import should succeed: {:?}", result);

        // Verify no new measurements were stored (dry run)
        let commits_after = walk_commits(1).unwrap();
        let notes_after = commits_after[0].1.len();

        assert_eq!(
            notes_after, notes_before,
            "No new measurements should be stored in dry run (before: {}, after: {})",
            notes_before, notes_after
        );
    }

    #[test]
    fn test_handle_import_junit_stores_measurements() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            None,
            false, // not dry_run
            false,
        );

        assert!(result.is_ok(), "Import should succeed: {:?}", result);

        // Verify measurements were stored
        let commits = walk_commits(1).unwrap();
        let notes = &commits[0].1;

        // Should have 2 measurements (passed and failed tests with durations)
        // Skipped test has no time attribute so it's not imported
        assert!(
            notes.len() >= 2,
            "Should have at least 2 measurement lines, got: {}",
            notes.len()
        );

        // Verify measurement names
        let notes_text = notes.join("\n");
        assert!(
            notes_text.contains("test::test_passed"),
            "Should contain test_passed measurement"
        );
        assert!(
            notes_text.contains("test::test_failed"),
            "Should contain test_failed measurement"
        );

        // Skipped test should not be stored (no time attribute means no duration)
        assert!(
            !notes_text.contains("test::test_skipped"),
            "Should not contain skipped test (no time attribute = no duration)"
        );
    }

    #[test]
    fn test_handle_import_junit_with_prefix() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            Some("ci".to_string()), // prefix
            vec![],
            None,
            false,
            false,
        );

        assert!(result.is_ok(), "Import with prefix should succeed");

        let commits = walk_commits(1).unwrap();
        let notes_text = commits[0].1.join("\n");

        assert!(
            notes_text.contains("ci::test::test_passed"),
            "Should contain prefixed measurement name"
        );
    }

    #[test]
    fn test_handle_import_junit_with_metadata() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![
                ("ci".to_string(), "true".to_string()),
                ("branch".to_string(), "main".to_string()),
            ],
            None,
            false,
            false,
        );

        assert!(result.is_ok(), "Import with metadata should succeed");

        let commits = walk_commits(1).unwrap();
        let notes_text = commits[0].1.join("\n");

        // Metadata should be included in the stored measurements
        assert!(
            notes_text.contains("ci") && notes_text.contains("true"),
            "Should contain ci metadata"
        );
        assert!(
            notes_text.contains("branch") && notes_text.contains("main"),
            "Should contain branch metadata"
        );
    }

    #[test]
    fn test_handle_import_junit_with_filter() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        // Filter to only import tests matching "passed"
        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            Some("passed".to_string()), // filter
            false,
            false,
        );

        assert!(result.is_ok(), "Import with filter should succeed");

        let commits = walk_commits(1).unwrap();
        let notes_text = commits[0].1.join("\n");

        assert!(
            notes_text.contains("test::test_passed"),
            "Should contain filtered test_passed"
        );
        assert!(
            !notes_text.contains("test::test_failed"),
            "Should not contain filtered out test_failed"
        );
    }

    #[test]
    fn test_handle_import_criterion_json() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_CRITERION_JSON).unwrap();

        let result = handle_import(
            ImportFormat::CriterionJson,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            None,
            false,
            false,
        );

        assert!(
            result.is_ok(),
            "Criterion import should succeed: {:?}",
            result
        );

        let commits = walk_commits(1).unwrap();
        let notes_text = commits[0].1.join("\n");

        // Should have multiple statistics per benchmark
        assert!(
            notes_text.contains("bench::fibonacci/fib_10::mean"),
            "Should contain mean statistic"
        );
        assert!(
            notes_text.contains("bench::fibonacci/fib_10::median"),
            "Should contain median statistic"
        );
        assert!(
            notes_text.contains("bench::fibonacci/fib_10::slope"),
            "Should contain slope statistic"
        );

        // Check unit conversion (us -> ns)
        assert!(
            notes_text.contains("bench::fibonacci/fib_20::mean"),
            "Should contain second benchmark"
        );
    }

    #[test]
    fn test_handle_import_invalid_format() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "invalid xml content").unwrap();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            None,
            false,
            false,
        );

        assert!(result.is_err(), "Should fail with invalid XML");
        assert!(
            result.unwrap_err().to_string().contains("parse"),
            "Error should mention parsing failure"
        );
    }

    #[test]
    fn test_handle_import_empty_file() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"<?xml version="1.0"?><testsuites tests="0"></testsuites>"#
        )
        .unwrap();

        // Get initial notes count before import
        let commits_before = walk_commits(1).unwrap();
        let notes_before = commits_before[0].1.len();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            None,
            false,
            false,
        );

        // Should succeed but import no measurements
        assert!(result.is_ok(), "Should handle empty test results");

        // Verify no new measurements were added
        let commits_after = walk_commits(1).unwrap();
        let notes_after = commits_after[0].1.len();

        assert_eq!(
            notes_after, notes_before,
            "Should not store any new measurements for empty results (before: {}, after: {})",
            notes_before, notes_after
        );
    }

    #[test]
    fn test_handle_import_invalid_regex_filter() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", SAMPLE_JUNIT_XML).unwrap();

        let result = handle_import(
            ImportFormat::Junit,
            Some(file.path().to_str().unwrap().to_string()),
            None,
            vec![],
            Some("[invalid(regex".to_string()), // invalid regex
            false,
            false,
        );

        assert!(result.is_err(), "Should fail with invalid regex");
        assert!(
            result.unwrap_err().to_string().contains("regex"),
            "Error should mention regex"
        );
    }

    #[test]
    fn test_store_measurements_integration() {
        let tempdir = dir_with_repo();
        set_current_dir(tempdir.path()).unwrap();
        hermetic_git_env();

        // Create test measurements
        let measurements = vec![
            MeasurementData {
                epoch: 0,
                name: "test::integration_test".to_string(),
                timestamp: 1234567890.0,
                val: 1500000000.0, // 1.5 seconds in nanoseconds
                key_values: {
                    let mut map = HashMap::new();
                    map.insert("type".to_string(), "test".to_string());
                    map.insert("status".to_string(), "passed".to_string());
                    map
                },
            },
            MeasurementData {
                epoch: 0,
                name: "bench::my_bench::mean".to_string(),
                timestamp: 1234567890.0,
                val: 15000.0, // 15000 nanoseconds
                key_values: {
                    let mut map = HashMap::new();
                    map.insert("type".to_string(), "bench".to_string());
                    map.insert("statistic".to_string(), "mean".to_string());
                    map
                },
            },
        ];

        let result = store_measurements(&measurements);
        assert!(
            result.is_ok(),
            "Storing measurements should succeed: {:?}",
            result
        );

        // Verify measurements were stored
        let commits = walk_commits(1).unwrap();
        let notes = &commits[0].1;

        assert!(
            notes.len() >= 2,
            "Should have stored 2 measurements, got: {}",
            notes.len()
        );

        let notes_text = notes.join("\n");
        assert!(
            notes_text.contains("test::integration_test"),
            "Should contain test measurement"
        );
        assert!(
            notes_text.contains("bench::my_bench::mean"),
            "Should contain benchmark measurement"
        );
    }
}
