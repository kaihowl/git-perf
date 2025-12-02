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
// Git Configuration Defaults
// ============================================================================

// Default remote name for git-perf notes:
// This is already defined as GIT_PERF_REMOTE in git/git_definitions.rs
// and should not be duplicated. The actual value is "git-perf-origin".
// See git/git_definitions.rs:28 for the implementation.

// ============================================================================
// Reporting Configuration Defaults
// ============================================================================

/// Default number of characters to display from commit SHA in report metadata.
///
/// This value is used when displaying commit ranges in report headers,
/// providing a balance between readability and uniqueness.
pub const DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_METADATA: usize = 7;

/// Default number of characters to display from commit SHA in report x-axis.
///
/// This value is used when displaying commit hashes on the x-axis of plots,
/// optimized for display space and readability in interactive visualizations.
pub const DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_AXIS: usize = 6;

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

/// Returns the default commit hash display length for metadata.
#[inline]
pub const fn default_commit_hash_display_length_metadata() -> usize {
    DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_METADATA
}

/// Returns the default commit hash display length for axis.
#[inline]
pub const fn default_commit_hash_display_length_axis() -> usize {
    DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_AXIS
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
    fn test_default_commit_hash_display_length_metadata() {
        assert_eq!(DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_METADATA, 7);
        assert_eq!(default_commit_hash_display_length_metadata(), 7);
    }

    #[test]
    fn test_default_commit_hash_display_length_axis() {
        assert_eq!(DEFAULT_COMMIT_HASH_DISPLAY_LENGTH_AXIS, 6);
        assert_eq!(default_commit_hash_display_length_axis(), 6);
    }
}
