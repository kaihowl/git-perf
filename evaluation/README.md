# Dispersion Method Evaluation

This directory contains tools for evaluating the statistical robustness of different dispersion methods used in git-perf's audit functionality.

**Location**: `evaluation/` (moved from project root for better organization)

## Overview

The `evaluate_dispersion_methods.py` script compares Standard Deviation (stddev) vs Median Absolute Deviation (MAD) for outlier detection in performance measurements. It generates synthetic data with controlled outliers and compares the detection accuracy of both methods.

## Usage

### Basic Usage

```bash
# Navigate to the evaluation directory
cd evaluation/

# Run the evaluation with default settings
python3 evaluate_dispersion_methods.py

# Save results to a specific file
python3 evaluate_dispersion_methods.py --output my_results.json

# Verbose output
python3 evaluate_dispersion_methods.py --verbose
```

### Command Line Options

- `--output`, `-o`: Output JSON file for results (default: `dispersion_evaluation_results.json`)
- `--verbose`, `-v`: Verbose output
- `--help`, `-h`: Show help message

## Test Scenarios

The evaluation script runs 14 different test scenarios:

### Clean Data
- **clean_data**: No outliers, baseline performance

### Single Outlier Scenarios
- **single_outlier_1pct**: 1% outliers at 3σ
- **single_outlier_5pct**: 5% outliers at 3σ

### Multiple Outlier Scenarios
- **multiple_outliers_5pct**: 5% outliers at 2σ
- **multiple_outliers_10pct**: 10% outliers at 2σ (true regression)
- **multiple_outliers_20pct**: 20% outliers at 2σ (true regression)

### Extreme Outlier Scenarios
- **extreme_outliers**: 5% outliers at 5σ
- **very_extreme_outliers**: 5% outliers at 8σ

### Different Noise Levels
- **high_noise**: 15ms noise, 10% outliers at 2σ (true regression)
- **low_noise**: 2ms noise, 10% outliers at 2σ (true regression)
- **different_base**: 50ms base, 3ms noise, 10% outliers at 2σ (true regression)

### Clear Regression Scenarios
- **clear_regression**: 15% outliers at 3σ (true regression)
- **moderate_regression**: 10% outliers at 2.5σ (true regression)
- **strong_regression**: 20% outliers at 4σ (true regression)

## Output

The script provides:

1. **Console Output**: Summary statistics and detailed results for each scenario
2. **JSON File**: Complete results in machine-readable format

### Key Metrics

- **True Positive Rate**: Percentage of actual regressions correctly detected
- **False Positive Rate**: Percentage of non-regressions incorrectly flagged
- **False Negative Rate**: Percentage of actual regressions missed
- **Precision**: Percentage of detected regressions that are actually regressions
- **Average Z-scores**: Mean z-score for each method across all scenarios

## Example Results

```
================================================================================
DISPERSION METHOD EVALUATION RESULTS
================================================================================

Total scenarios tested: 14
Scenarios with true regression: 8
Scenarios without true regression: 6

--------------------------------------------------
DETECTION PERFORMANCE
--------------------------------------------------

Standard Deviation (stddev):
  True Positive Rate:  0.000
  False Positive Rate: 0.000
  False Negative Rate: 1.000
  Precision:           0.000

Median Absolute Deviation (MAD):
  True Positive Rate:  0.125
  False Positive Rate: 0.000
  False Negative Rate: 0.875
  Precision:           1.000

Average Z-scores:
  Standard Deviation: 0.696
  MAD:                1.417
```

## Interpretation

### When MAD Performs Better

MAD typically shows:
- **Higher precision**: Fewer false positives when it does detect regressions
- **More conservative detection**: Higher z-scores, requiring more significant changes
- **Better robustness**: Less affected by extreme outliers in the data

### When Standard Deviation Performs Better

Standard deviation typically shows:
- **Higher sensitivity**: More likely to detect small changes
- **Lower z-scores**: More sensitive to all types of performance changes
- **Better for clean data**: When data is normally distributed without outliers

## Recommendations

Based on the evaluation results:

1. **Use MAD when**:
   - Performance data has occasional outliers or spikes
   - You want to focus on typical performance changes
   - You're measuring in environments with variable system load
   - You want more robust regression detection

2. **Use Standard Deviation when**:
   - Performance data is normally distributed
   - You want to detect all performance changes, including outliers
   - You have consistent, stable performance measurements
   - You need maximum sensitivity to changes

## Technical Details

### MAD Calculation

The script implements MAD calculation as:
```python
def calculate_mad(data: List[float]) -> float:
    if not data:
        return 0.0
    
    data_median = median(data)
    abs_deviations = [abs(x - data_median) for x in data]
    return median(abs_deviations)
```

### Z-Score Calculation

Z-scores are calculated as:
- **Standard Deviation**: `|head_value - tail_mean| / tail_stddev`
- **MAD**: `|head_value - tail_median| / tail_mad`

### True Regression Definition

A scenario is considered to have a "true regression" if:
- Outlier ratio > 5% AND
- Outlier magnitude > 1.5σ

This represents a significant performance degradation that should be detected.

## Integration with git-perf

The evaluation results support the implementation of MAD as an alternative dispersion method in git-perf. The configuration allows users to choose between stddev and MAD based on their specific use case and data characteristics.

## Future Enhancements

Potential improvements to the evaluation:

1. **More test scenarios**: Additional edge cases and real-world data patterns
2. **Statistical significance testing**: Formal hypothesis testing of method differences
3. **Real data integration**: Testing with actual performance measurement data
4. **Threshold optimization**: Finding optimal sigma thresholds for each method
5. **Visualization**: Charts showing method performance across different scenarios