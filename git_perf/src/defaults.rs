//! Centralized default values for git-perf configuration.
//!
//! This module defines all default values used throughout the application
//! to avoid magic numbers scattered in the codebase. These defaults are used
//! as fallback values when configuration is not explicitly provided.

// ============================================================================
// Audit Configuration Defaults
// ============================================================================

/// Default minimum number of historical measurements required for audit.
///
/// When auditing a measurement, at least this many historical data points
/// are required to establish a statistical baseline. If fewer measurements
/// are available, the audit will be skipped with a warning.
///
/// This value is used when neither CLI options nor configuration file
/// specify `min_measurements`.
pub const DEFAULT_MIN_MEASUREMENTS: u16 = 2;

/// Default sigma (standard deviation threshold) for statistical significance.
///
/// Measurements are considered significantly different if their z-score
/// exceeds this threshold. A value of 4.0 means that the measurement must
/// be more than 4 standard deviations (or MAD units) away from the mean
/// to be flagged as a regression.
///
/// This value is used when neither CLI options nor configuration file
/// specify `sigma`.
pub const DEFAULT_SIGMA: f64 = 4.0;

// ============================================================================
// Backoff Configuration Defaults
// ============================================================================

/// Default maximum elapsed time (in seconds) for exponential backoff retries.
///
/// When operations fail (e.g., network requests for git operations), the system
/// will retry with exponential backoff up to this maximum duration before
/// giving up.
///
/// This value is used when configuration file does not specify
/// `backoff.max_elapsed_seconds`.
pub const DEFAULT_BACKOFF_MAX_ELAPSED_SECONDS: u64 = 60;

// ============================================================================
// Epoch Configuration Defaults
// ============================================================================

/// Default epoch value when no epoch is configured.
///
/// An epoch is a commit hash that marks the start of a new measurement series.
/// When measurements are incompatible between different periods (e.g., after
/// a significant change to the benchmark), a new epoch can be set to separate
/// the data.
///
/// A value of 0 indicates no epoch boundary, meaning all measurements are
/// considered part of the same series.
pub const DEFAULT_EPOCH: u32 = 0;

// ============================================================================
// Calculation Defaults
// ============================================================================

/// Default value for median when no measurements are available.
///
/// This is used in calculations where a median is required but the dataset
/// is empty. Using 0.0 allows calculations to proceed without errors while
/// making it clear that no data is available.
pub const DEFAULT_MEDIAN_EMPTY: f64 = 0.0;

// ============================================================================
// Git Configuration Defaults
// ============================================================================

/// Default remote name for git-perf notes.
///
/// This is defined in git/git_interop.rs as GIT_PERF_REMOTE constant.
/// Documenting it here for completeness.
pub const DEFAULT_GIT_REMOTE: &str = "origin";

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns the default minimum number of measurements for audit.
#[inline]
pub const fn default_min_measurements() -> u16 {
    DEFAULT_MIN_MEASUREMENTS
}

/// Returns the default sigma threshold for statistical significance.
#[inline]
pub const fn default_sigma() -> f64 {
    DEFAULT_SIGMA
}

/// Returns the default backoff max elapsed seconds.
#[inline]
pub const fn default_backoff_max_elapsed_seconds() -> u64 {
    DEFAULT_BACKOFF_MAX_ELAPSED_SECONDS
}

/// Returns the default epoch value.
#[inline]
pub const fn default_epoch() -> u32 {
    DEFAULT_EPOCH
}

/// Returns the default median value for empty datasets.
#[inline]
pub const fn default_median_empty() -> f64 {
    DEFAULT_MEDIAN_EMPTY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_min_measurements() {
        assert_eq!(DEFAULT_MIN_MEASUREMENTS, 2);
        assert_eq!(default_min_measurements(), 2);
    }

    #[test]
    fn test_default_sigma() {
        assert_eq!(DEFAULT_SIGMA, 4.0);
        assert_eq!(default_sigma(), 4.0);
    }

    #[test]
    fn test_default_backoff_max_elapsed_seconds() {
        assert_eq!(DEFAULT_BACKOFF_MAX_ELAPSED_SECONDS, 60);
        assert_eq!(default_backoff_max_elapsed_seconds(), 60);
    }

    #[test]
    fn test_default_epoch() {
        assert_eq!(DEFAULT_EPOCH, 0);
        assert_eq!(default_epoch(), 0);
    }

    #[test]
    fn test_default_median_empty() {
        assert_eq!(DEFAULT_MEDIAN_EMPTY, 0.0);
        assert_eq!(default_median_empty(), 0.0);
    }
}
