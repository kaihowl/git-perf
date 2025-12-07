use anyhow::{anyhow, bail, Result};
use regex::Regex;
use std::{path::PathBuf, sync::OnceLock};

use crate::stats::ReductionFunc;

/// Cached regex for parsing section placeholders (compiled once)
static SECTION_PLACEHOLDER_REGEX: OnceLock<Regex> = OnceLock::new();

/// Cached regex for finding all section blocks in a template (compiled once)
static SECTION_FINDER_REGEX: OnceLock<Regex> = OnceLock::new();

/// Get or compile the section placeholder parsing regex
fn section_placeholder_regex() -> &'static Regex {
    SECTION_PLACEHOLDER_REGEX.get_or_init(|| {
        Regex::new(r"(?s)\{\{SECTION\[([^\]]+)\](.*?)\}\}")
            .expect("Invalid section placeholder regex pattern")
    })
}

/// Get or compile the section finder regex
fn section_finder_regex() -> &'static Regex {
    SECTION_FINDER_REGEX.get_or_init(|| {
        Regex::new(r"(?s)\{\{SECTION\[[^\]]+\].*?\}\}")
            .expect("Invalid section finder regex pattern")
    })
}

/// Configuration for report templates
#[derive(Debug, Clone)]
pub struct ReportTemplateConfig {
    pub template_path: Option<PathBuf>,
    pub custom_css_path: Option<PathBuf>,
    pub title: Option<String>,
}

/// Configuration for a single report section in a multi-section template
#[derive(Debug, Clone)]
pub struct SectionConfig {
    /// Section identifier (e.g., "test-overview", "bench-median")
    pub id: String,
    /// Original placeholder text to replace (e.g., "{{SECTION[id] param: value }}")
    pub placeholder: String,
    /// Regex pattern for selecting measurements
    pub measurement_filter: Option<String>,
    /// Key-value pairs to match (e.g., os=linux,arch=x64)
    pub key_value_filter: Vec<(String, String)>,
    /// Metadata keys to split traces by (e.g., ["os", "arch"])
    pub separate_by: Vec<String>,
    /// Aggregation function (none means raw data)
    pub aggregate_by: Option<ReductionFunc>,
    /// Number of commits (overrides global depth)
    pub depth: Option<usize>,
    /// Show epoch boundaries for this section
    pub show_epochs: bool,
    /// Detect and show change points for this section
    pub detect_changes: bool,
}

impl SectionConfig {
    /// Parse a single section placeholder into a SectionConfig
    /// Format: {{SECTION[id] param: value, param2: value2 }}
    pub fn parse(placeholder: &str) -> Result<Self> {
        // Use cached regex to extract section ID and parameters
        let section_regex = section_placeholder_regex();

        let captures = section_regex
            .captures(placeholder)
            .ok_or_else(|| anyhow!("Invalid section placeholder format: {}", placeholder))?;

        let id = captures
            .get(1)
            .expect("Regex capture group 1 (section ID) must exist")
            .as_str()
            .trim()
            .to_string();
        let params_str = captures
            .get(2)
            .expect("Regex capture group 2 (parameters) must exist")
            .as_str()
            .trim();

        // Parse parameters
        let mut measurement_filter = None;
        let mut key_value_filter = Vec::new();
        let mut separate_by = Vec::new();
        let mut aggregate_by = None;
        let mut depth = None;
        let mut show_epochs = false;
        let mut detect_changes = false;

        if !params_str.is_empty() {
            // Split by newlines and trim each line
            for line in params_str.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Parse "param: value" format
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();

                    match key {
                        "measurement-filter" => {
                            measurement_filter = Some(value.to_string());
                        }
                        "key-value-filter" => {
                            key_value_filter = parse_key_value_filter(value)?;
                        }
                        "separate-by" => {
                            separate_by = parse_comma_separated_list(value);
                        }
                        "aggregate-by" => {
                            aggregate_by = parse_aggregate_by(value)?;
                        }
                        "depth" => {
                            depth = Some(parse_depth(value)?);
                        }
                        "show-epochs" => {
                            show_epochs = parse_boolean(value, "show-epochs")?;
                        }
                        "detect-changes" => {
                            detect_changes = parse_boolean(value, "detect-changes")?;
                        }
                        _ => {
                            // Unknown parameter - warn but don't fail
                            log::warn!("Unknown section parameter: {}", key);
                        }
                    }
                }
            }
        }

        Ok(SectionConfig {
            id,
            placeholder: placeholder.to_string(),
            measurement_filter,
            key_value_filter,
            separate_by,
            aggregate_by,
            depth,
            show_epochs,
            detect_changes,
        })
    }
}

/// Parse key-value filter pairs from a comma-separated string
/// Format: "key1=value1,key2=value2"
fn parse_key_value_filter(value: &str) -> Result<Vec<(String, String)>> {
    value
        .split(',')
        .map(|pair| {
            let pair = pair.trim();
            let (k, v) = pair
                .split_once('=')
                .ok_or_else(|| anyhow!("Invalid key-value-filter format: {}", pair))?;
            let k = k.trim();
            let v = v.trim();
            if k.is_empty() {
                bail!("Empty key in key-value-filter: '{}'", pair);
            }
            if v.is_empty() {
                bail!("Empty value in key-value-filter: '{}'", pair);
            }
            Ok((k.to_string(), v.to_string()))
        })
        .collect()
}

/// Parse a comma-separated list into a vector of strings
fn parse_comma_separated_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse an aggregate-by value into a ReductionFunc
fn parse_aggregate_by(value: &str) -> Result<Option<ReductionFunc>> {
    match value {
        "none" => Ok(None),
        "min" => Ok(Some(ReductionFunc::Min)),
        "max" => Ok(Some(ReductionFunc::Max)),
        "median" => Ok(Some(ReductionFunc::Median)),
        "mean" => Ok(Some(ReductionFunc::Mean)),
        _ => bail!("Invalid aggregate-by value: {}", value),
    }
}

/// Parse a depth value into usize
fn parse_depth(value: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| anyhow!("Invalid depth value: {}", value))
}

/// Parse a boolean value from various string representations
fn parse_boolean(value: &str, param_name: &str) -> Result<bool> {
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") || value == "1" {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("no")
        || value == "0"
    {
        Ok(false)
    } else {
        bail!("Invalid {} value: {} (use true/false)", param_name, value)
    }
}

/// Parse all section placeholders from a template
pub fn parse_template_sections(template: &str) -> Result<Vec<SectionConfig>> {
    // Use cached regex to find all {{SECTION[...] ...}} blocks
    let section_regex = section_finder_regex();

    let mut sections = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for captures in section_regex.find_iter(template) {
        let placeholder = captures.as_str();
        let section = SectionConfig::parse(placeholder)?;

        // Check for duplicate section IDs
        if !seen_ids.insert(section.id.clone()) {
            bail!("Duplicate section ID found: {}", section.id);
        }

        sections.push(section);
    }

    Ok(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_value_filter_valid() {
        let result = parse_key_value_filter("os=linux,arch=x64").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("os".to_string(), "linux".to_string()));
        assert_eq!(result[1], ("arch".to_string(), "x64".to_string()));
    }

    #[test]
    fn test_parse_key_value_filter_invalid() {
        assert!(parse_key_value_filter("invalid").is_err());
        assert!(parse_key_value_filter("=value").is_err());
        assert!(parse_key_value_filter("key=").is_err());
    }

    #[test]
    fn test_parse_comma_separated_list() {
        let result = parse_comma_separated_list("os, arch, version");
        assert_eq!(result, vec!["os", "arch", "version"]);
    }

    #[test]
    fn test_parse_aggregate_by() {
        assert_eq!(parse_aggregate_by("none").unwrap(), None);
        assert_eq!(parse_aggregate_by("min").unwrap(), Some(ReductionFunc::Min));
        assert!(parse_aggregate_by("invalid").is_err());
    }

    #[test]
    fn test_parse_boolean() {
        assert!(parse_boolean("true", "test").unwrap());
        assert!(parse_boolean("True", "test").unwrap());
        assert!(parse_boolean("yes", "test").unwrap());
        assert!(parse_boolean("1", "test").unwrap());
        assert!(!parse_boolean("false", "test").unwrap());
        assert!(!parse_boolean("False", "test").unwrap());
        assert!(!parse_boolean("no", "test").unwrap());
        assert!(!parse_boolean("0", "test").unwrap());
        assert!(parse_boolean("invalid", "test").is_err());
    }

    #[test]
    fn test_parse_depth() {
        assert_eq!(parse_depth("100").unwrap(), 100);
        assert!(parse_depth("invalid").is_err());
        assert!(parse_depth("-5").is_err());
    }
}
