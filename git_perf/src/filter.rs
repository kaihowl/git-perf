use anyhow::{Context, Result};
use regex::Regex;

/// Compile filter patterns into regex objects
pub fn compile_filters(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| {
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: '{}'", pattern))
        })
        .collect()
}

/// Convert measurement names to anchored regex patterns for exact matching
/// This escapes special regex characters and adds ^ and $ anchors
#[must_use]
pub fn measurements_to_anchored_regex(measurements: &[String]) -> Vec<String> {
    measurements
        .iter()
        .map(|m| format!("^{}$", regex::escape(m)))
        .collect()
}

/// Combine measurements (as exact matches) and filter patterns into a single list
/// Measurements are converted to anchored regex patterns for exact matching
#[must_use]
pub fn combine_measurements_and_filters(
    measurements: &[String],
    filter_patterns: &[String],
) -> Vec<String> {
    let mut combined = measurements_to_anchored_regex(measurements);
    combined.extend_from_slice(filter_patterns);
    combined
}

/// Check if a measurement name matches any of the compiled filters
/// Returns true if filters is empty (no filters = match all)
#[must_use]
pub fn matches_any_filter(name: &str, filters: &[Regex]) -> bool {
    if filters.is_empty() {
        return true; // No filters = match all
    }
    filters.iter().any(|re| re.is_match(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_valid_filters() {
        let patterns = vec!["bench.*".to_string(), "test_.*".to_string()];
        let result = compile_filters(&patterns);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_compile_invalid_regex() {
        let patterns = vec!["[invalid".to_string()];
        let result = compile_filters(&patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_matches_any_filter_empty() {
        let filters = vec![];
        assert!(matches_any_filter("anything", &filters));
    }

    #[test]
    fn test_matches_any_filter_single_match() {
        let patterns = vec!["bench.*".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(!matches_any_filter("test_foo", &filters));
    }

    #[test]
    fn test_matches_any_filter_or_logic() {
        let patterns = vec!["bench.*".to_string(), "test_.*".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(matches_any_filter("test_foo", &filters));
        assert!(!matches_any_filter("other_thing", &filters));
    }

    #[test]
    fn test_anchored_patterns() {
        let patterns = vec!["^bench.*$".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("benchmark_x64", &filters));
        assert!(!matches_any_filter("my_benchmark_x64", &filters));
    }

    #[test]
    fn test_complex_regex() {
        let patterns = vec![r"bench_.*_v\d+".to_string()];
        let filters = compile_filters(&patterns).unwrap();
        assert!(matches_any_filter("bench_foo_v1", &filters));
        assert!(matches_any_filter("bench_bar_v23", &filters));
        assert!(!matches_any_filter("bench_baz_vX", &filters));
    }

    #[test]
    fn test_measurements_to_anchored_regex() {
        let measurements = vec![
            "benchmark_x64".to_string(),
            "test.with.dots".to_string(),
            "name[with]brackets".to_string(),
        ];
        let anchored = measurements_to_anchored_regex(&measurements);

        assert_eq!(anchored.len(), 3);
        assert_eq!(anchored[0], "^benchmark_x64$");
        assert_eq!(anchored[1], r"^test\.with\.dots$");
        assert_eq!(anchored[2], r"^name\[with\]brackets$");
    }

    #[test]
    fn test_measurements_to_anchored_regex_matches_exactly() {
        let measurements = vec!["benchmark".to_string()];
        let anchored = measurements_to_anchored_regex(&measurements);
        let filters = compile_filters(&anchored).unwrap();

        // Should match exact name
        assert!(matches_any_filter("benchmark", &filters));

        // Should NOT match partial matches
        assert!(!matches_any_filter("benchmark_x64", &filters));
        assert!(!matches_any_filter("my_benchmark", &filters));
    }

    #[test]
    fn test_combine_measurements_and_filters() {
        let measurements = vec!["exact_match".to_string()];
        let filters = vec!["pattern.*".to_string()];

        let combined = combine_measurements_and_filters(&measurements, &filters);

        assert_eq!(combined.len(), 2);
        assert_eq!(combined[0], "^exact_match$");
        assert_eq!(combined[1], "pattern.*");
    }

    #[test]
    fn test_combine_measurements_and_filters_matching() {
        let measurements = vec!["benchmark_x64".to_string()];
        let filters = vec!["test_.*".to_string()];

        let combined = combine_measurements_and_filters(&measurements, &filters);
        let compiled = compile_filters(&combined).unwrap();

        // Should match exact measurement name
        assert!(matches_any_filter("benchmark_x64", &compiled));

        // Should match filter pattern
        assert!(matches_any_filter("test_foo", &compiled));
        assert!(matches_any_filter("test_bar", &compiled));

        // Should NOT match other names
        assert!(!matches_any_filter("benchmark_arm64", &compiled));
        assert!(!matches_any_filter("other", &compiled));
    }
}
