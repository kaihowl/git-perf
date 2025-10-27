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

/// Check if a measurement name matches any of the compiled filters
/// Returns true if filters is empty (no filters = match all)
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
}
