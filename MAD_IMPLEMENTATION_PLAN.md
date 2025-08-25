# MAD Implementation Plan for git-perf

## Overview

This document outlines the plan to implement Median Absolute Deviation (MAD) as an alternative to standard deviation in git-perf's auditing functionality. MAD is a robust measure of statistical dispersion that is less sensitive to outliers than standard deviation, making it potentially more suitable for performance measurement analysis.

## Current Implementation Analysis

### Current Audit System
- Uses standard deviation (σ) for outlier detection
- Calculates z-score: `|head_value - tail_mean| / tail_stddev`
- Compares against configurable sigma threshold (default: 4.0)
- Includes relative deviation threshold as noise filter
- Supports measurement-specific and global configurations

### Key Components
1. **Stats Module** (`git_perf/src/stats.rs`): Core statistical calculations
2. **Audit Module** (`git_perf/src/audit.rs`): Main audit logic
3. **CLI Types** (`cli_types/src/lib.rs`): Command-line interface definitions
4. **Configuration** (`git_perf/src/config.rs`): Settings management

## MAD Implementation Plan

### Phase 1: Core MAD Implementation

#### 1.1 Extend Stats Module
**File**: `git_perf/src/stats.rs`

**Changes**:
- Add MAD calculation to `Stats` struct
- Implement MAD-based z-score calculation
- Add enum for dispersion method selection

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DispersionMethod {
    StandardDeviation,
    MedianAbsoluteDeviation,
}

#[derive(Debug)]
pub struct Stats {
    pub mean: f64,
    pub stddev: f64,
    pub mad: f64,  // New field
    pub len: usize,
}

impl Stats {
    pub fn z_score(&self, other: &Stats, method: DispersionMethod) -> f64 {
        // Implementation for both stddev and MAD
    }
    
    pub fn calculate_mad(measurements: &[f64]) -> f64 {
        // MAD = median(|x_i - median(x)|)
    }
}
```

#### 1.2 Update Aggregate Function
**File**: `git_perf/src/stats.rs`

**Changes**:
- Modify `aggregate_measurements` to calculate both stddev and MAD
- Ensure backward compatibility

### Phase 2: CLI Interface Enhancement

#### 2.1 Add Dispersion Method Option
**File**: `cli_types/src/lib.rs`

**Changes**:
- Add new enum for dispersion method
- Extend `Audit` command with `--dispersion-method` option
- Update help text and documentation

```rust
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum DispersionMethod {
    #[value(name = "stddev")]
    StandardDeviation,
    #[value(name = "mad")]
    MedianAbsoluteDeviation,
}

// In Audit command:
/// Method for calculating statistical dispersion
#[arg(long, value_enum, default_value = "stddev")]
dispersion_method: DispersionMethod,
```

#### 2.2 Update CLI Handler
**File**: `git_perf/src/cli.rs`

**Changes**:
- Pass dispersion method to audit functions
- Update function signatures

### Phase 3: Audit Logic Updates

#### 3.1 Modify Audit Functions
**File**: `git_perf/src/audit.rs`

**Changes**:
- Update `audit_multiple` and `audit` function signatures
- Modify z-score calculation to use selected dispersion method
- Update output formatting to show dispersion method used
- Ensure relative deviation calculations remain unchanged

#### 3.2 Update Output Format
**Changes**:
- Modify audit result messages to indicate dispersion method
- Update z-score display format
- Add dispersion method to summary output

### Phase 4: Configuration Support

#### 4.1 Extend Configuration
**File**: `git_perf/src/config.rs`

**Changes**:
- Add default dispersion method configuration
- Support measurement-specific dispersion method settings
- Maintain backward compatibility

```toml
[audit.global]
dispersion_method = "mad"  # or "stddev"

[audit.measurement."build_time"]
dispersion_method = "mad"
min_relative_deviation = 10.0
```

#### 4.2 Configuration Functions
**Changes**:
- Add `audit_dispersion_method_from_str` function
- Add `audit_dispersion_method` function
- Implement precedence: measurement-specific > global > default

### Phase 5: Testing and Validation

#### 5.1 Unit Tests
**Files**: `git_perf/src/stats.rs`, `git_perf/src/audit.rs`

**Test Cases**:
- MAD calculation accuracy
- Z-score calculation with MAD
- Edge cases (zero MAD, single measurement)
- Comparison with standard deviation results

#### 5.2 Integration Tests
**Files**: `test/test_audit_*.sh`

**Test Cases**:
- CLI interface with new dispersion method option
- Configuration file parsing
- Backward compatibility
- Mixed dispersion methods for different measurements

### Phase 6: Documentation Updates

#### 6.1 Manpage Updates
**File**: `docs/manpage.md`

**Changes**:
- Document new `--dispersion-method` option
- Explain MAD vs stddev differences
- Update examples

#### 6.2 README Updates
**File**: `README.md`

**Changes**:
- Add MAD explanation and benefits
- Update configuration examples
- Add usage examples

#### 6.3 Configuration Documentation
**File**: `docs/example_config.toml`

**Changes**:
- Add dispersion method configuration examples
- Document when to use MAD vs stddev

## Evaluation Plan

### Objective 1: Statistical Robustness Comparison

#### 1.1 Outlier Sensitivity Analysis
**Method**: Synthetic data generation with controlled outliers

**Test Scenarios**:
- Clean data (no outliers)
- Single outlier (1-5% contamination)
- Multiple outliers (5-20% contamination)
- Extreme outliers (beyond 3σ)

**Metrics**:
- False positive rate (detecting non-existent regressions)
- False negative rate (missing actual regressions)
- Detection threshold stability

**Implementation**:
```python
# Example evaluation script
def generate_test_data(base_value, noise_level, outlier_ratio, outlier_magnitude):
    # Generate synthetic performance data
    pass

def compare_detection_methods(data, true_regression):
    # Compare stddev vs MAD detection accuracy
    pass
```

