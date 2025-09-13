#!/usr/bin/env python3
"""
Statistical robustness evaluation script for git-perf dispersion methods.

This script evaluates the performance of Standard Deviation vs Median Absolute Deviation
for outlier detection in performance measurements. It generates synthetic data with
controlled outliers and compares the detection accuracy of both methods.

Based on the MAD Implementation Plan for git-perf.
"""

import random
import argparse
import json
from typing import List, Tuple, Dict, Any
from dataclasses import dataclass
from statistics import median, stdev


@dataclass
class TestResult:
    """Results from a single test scenario."""
    scenario_name: str
    base_value: float
    noise_level: float
    outlier_ratio: float
    outlier_magnitude: float
    stddev_z_score: float
    mad_z_score: float
    stddev_detected: bool
    mad_detected: bool
    true_regression: bool


def calculate_mad(data: List[float]) -> float:
    """Calculate Median Absolute Deviation."""
    if not data:
        return 0.0
    
    data_median = median(data)
    abs_deviations = [abs(x - data_median) for x in data]
    return median(abs_deviations)


def calculate_z_score_stddev(head_value: float, tail_data: List[float]) -> float:
    """Calculate z-score using standard deviation."""
    if len(tail_data) < 2:
        return float('inf') if head_value != tail_data[0] else 0.0
    
    tail_mean = sum(tail_data) / len(tail_data)
    tail_stddev = stdev(tail_data)
    
    if tail_stddev == 0:
        return float('inf') if head_value != tail_mean else 0.0
    
    return abs(head_value - tail_mean) / tail_stddev


def calculate_z_score_mad(head_value: float, tail_data: List[float]) -> float:
    """Calculate z-score using Median Absolute Deviation."""
    if not tail_data:
        return float('inf')
    
    tail_median = median(tail_data)
    tail_mad = calculate_mad(tail_data)
    
    if tail_mad == 0:
        return float('inf') if head_value != tail_median else 0.0
    
    return abs(head_value - tail_median) / tail_mad


def normal_random(mean: float, stddev: float) -> float:
    """Generate a random number from normal distribution using Box-Muller transform."""
    import math
    u1 = random.random()
    u2 = random.random()
    z0 = math.sqrt(-2 * math.log(u1)) * math.cos(2 * math.pi * u2)
    return mean + stddev * z0


def generate_test_data(base_value: float, noise_level: float, outlier_ratio: float, 
                      outlier_magnitude: float, n_samples: int = 20) -> Tuple[List[float], bool]:
    """
    Generate synthetic performance data with controlled outliers.
    
    Args:
        base_value: Base performance value (e.g., 100ms)
        noise_level: Standard deviation of normal noise (e.g., 5ms)
        outlier_ratio: Fraction of data that are outliers (0.0 to 1.0)
        outlier_magnitude: How many standard deviations outliers are from base
        n_samples: Number of samples to generate
    
    Returns:
        Tuple of (data_list, has_true_regression)
    """
    # Generate normal data
    normal_samples = int(n_samples * (1 - outlier_ratio))
    outlier_samples = n_samples - normal_samples
    
    # Normal data with noise
    normal_data = [normal_random(base_value, noise_level) for _ in range(normal_samples)]
    
    # Outlier data (simulating performance regression)
    outlier_data = [normal_random(base_value + outlier_magnitude * noise_level, 
                                noise_level) for _ in range(outlier_samples)]
    
    # Combine and shuffle
    all_data = normal_data + outlier_data
    random.shuffle(all_data)
    
    # Determine if there's a true regression (if outliers are significant enough)
    has_true_regression = outlier_ratio > 0.05 and outlier_magnitude > 1.5
    
    return all_data, has_true_regression


def run_test_scenario(scenario_name: str, base_value: float, noise_level: float,
                     outlier_ratio: float, outlier_magnitude: float, 
                     sigma_threshold: float = 4.0) -> TestResult:
    """Run a single test scenario and return results."""
    
    # Generate test data
    tail_data, true_regression = generate_test_data(base_value, noise_level, 
                                                   outlier_ratio, outlier_magnitude)
    
    # Use the last value as HEAD (simulating current measurement)
    head_value = tail_data[-1]
    tail_data = tail_data[:-1]
    
    # Calculate z-scores
    stddev_z_score = calculate_z_score_stddev(head_value, tail_data)
    mad_z_score = calculate_z_score_mad(head_value, tail_data)
    
    # Determine if outliers were detected
    stddev_detected = stddev_z_score > sigma_threshold
    mad_detected = mad_z_score > sigma_threshold
    
    return TestResult(
        scenario_name=scenario_name,
        base_value=base_value,
        noise_level=noise_level,
        outlier_ratio=outlier_ratio,
        outlier_magnitude=outlier_magnitude,
        stddev_z_score=stddev_z_score,
        mad_z_score=mad_z_score,
        stddev_detected=stddev_detected,
        mad_detected=mad_detected,
        true_regression=true_regression
    )


def run_evaluation_suite() -> List[TestResult]:
    """Run a comprehensive evaluation suite."""
    results = []
    
    # Test scenarios based on the implementation plan
    scenarios = [
        # Clean data (no outliers)
        ("clean_data", 100.0, 5.0, 0.0, 0.0),
        
        # Single outlier scenarios
        ("single_outlier_1pct", 100.0, 5.0, 0.01, 3.0),
        ("single_outlier_5pct", 100.0, 5.0, 0.05, 3.0),
        
        # Multiple outliers - some should be true regressions
        ("multiple_outliers_5pct", 100.0, 5.0, 0.05, 2.0),
        ("multiple_outliers_10pct", 100.0, 5.0, 0.10, 2.0),
        ("multiple_outliers_20pct", 100.0, 5.0, 0.20, 2.0),
        
        # Extreme outliers - should be true regressions
        ("extreme_outliers", 100.0, 5.0, 0.05, 5.0),
        ("very_extreme_outliers", 100.0, 5.0, 0.05, 8.0),
        
        # Different base values and noise levels
        ("high_noise", 100.0, 15.0, 0.10, 2.0),
        ("low_noise", 100.0, 2.0, 0.10, 2.0),
        ("different_base", 50.0, 3.0, 0.10, 2.0),
        
        # Additional scenarios that should definitely be true regressions
        ("clear_regression", 100.0, 5.0, 0.15, 3.0),
        ("moderate_regression", 100.0, 5.0, 0.10, 2.5),
        ("strong_regression", 100.0, 5.0, 0.20, 4.0),
    ]
    
    print("Running evaluation scenarios...")
    for scenario_name, base_value, noise_level, outlier_ratio, outlier_magnitude in scenarios:
        print(f"  Running {scenario_name}...")
        result = run_test_scenario(scenario_name, base_value, noise_level, 
                                 outlier_ratio, outlier_magnitude)
        results.append(result)
    
    return results


def analyze_results(results: List[TestResult]) -> Dict[str, Any]:
    """Analyze the test results and compute metrics."""
    analysis = {
        "total_scenarios": len(results),
        "scenarios_with_true_regression": sum(1 for r in results if r.true_regression),
        "scenarios_without_true_regression": sum(1 for r in results if not r.true_regression),
    }
    
    # Calculate detection rates
    true_positives_stddev = sum(1 for r in results if r.true_regression and r.stddev_detected)
    true_positives_mad = sum(1 for r in results if r.true_regression and r.mad_detected)
    
    false_positives_stddev = sum(1 for r in results if not r.true_regression and r.stddev_detected)
    false_positives_mad = sum(1 for r in results if not r.true_regression and r.mad_detected)
    
    false_negatives_stddev = sum(1 for r in results if r.true_regression and not r.stddev_detected)
    false_negatives_mad = sum(1 for r in results if r.true_regression and not r.mad_detected)
    
    # Calculate rates
    true_regression_count = analysis["scenarios_with_true_regression"]
    no_regression_count = analysis["scenarios_without_true_regression"]
    
    analysis["stddev"] = {
        "true_positive_rate": true_positives_stddev / true_regression_count if true_regression_count > 0 else 0,
        "false_positive_rate": false_positives_stddev / no_regression_count if no_regression_count > 0 else 0,
        "false_negative_rate": false_negatives_stddev / true_regression_count if true_regression_count > 0 else 0,
        "precision": true_positives_stddev / (true_positives_stddev + false_positives_stddev) if (true_positives_stddev + false_positives_stddev) > 0 else 0,
    }
    
    analysis["mad"] = {
        "true_positive_rate": true_positives_mad / true_regression_count if true_regression_count > 0 else 0,
        "false_positive_rate": false_positives_mad / no_regression_count if no_regression_count > 0 else 0,
        "false_negative_rate": false_negatives_mad / true_regression_count if true_regression_count > 0 else 0,
        "precision": true_positives_mad / (true_positives_mad + false_positives_mad) if (true_positives_mad + false_positives_mad) > 0 else 0,
    }
    
    # Calculate average z-scores for comparison
    analysis["average_z_scores"] = {
        "stddev": sum(r.stddev_z_score for r in results if r.stddev_z_score != float('inf')) / len([r for r in results if r.stddev_z_score != float('inf')]),
        "mad": sum(r.mad_z_score for r in results if r.mad_z_score != float('inf')) / len([r for r in results if r.mad_z_score != float('inf')]),
    }
    
    return analysis


def print_results(results: List[TestResult], analysis: Dict[str, Any]):
    """Print the evaluation results in a readable format."""
    print("\n" + "="*80)
    print("DISPERSION METHOD EVALUATION RESULTS")
    print("="*80)
    
    print(f"\nTotal scenarios tested: {analysis['total_scenarios']}")
    print(f"Scenarios with true regression: {analysis['scenarios_with_true_regression']}")
    print(f"Scenarios without true regression: {analysis['scenarios_without_true_regression']}")
    
    print("\n" + "-"*50)
    print("DETECTION PERFORMANCE")
    print("-"*50)
    
    print(f"\nStandard Deviation (stddev):")
    print(f"  True Positive Rate:  {analysis['stddev']['true_positive_rate']:.3f}")
    print(f"  False Positive Rate: {analysis['stddev']['false_positive_rate']:.3f}")
    print(f"  False Negative Rate: {analysis['stddev']['false_negative_rate']:.3f}")
    print(f"  Precision:           {analysis['stddev']['precision']:.3f}")
    
    print(f"\nMedian Absolute Deviation (MAD):")
    print(f"  True Positive Rate:  {analysis['mad']['true_positive_rate']:.3f}")
    print(f"  False Positive Rate: {analysis['mad']['false_positive_rate']:.3f}")
    print(f"  False Negative Rate: {analysis['mad']['false_negative_rate']:.3f}")
    print(f"  Precision:           {analysis['mad']['precision']:.3f}")
    
    print(f"\nAverage Z-scores:")
    print(f"  Standard Deviation: {analysis['average_z_scores']['stddev']:.3f}")
    print(f"  MAD:                {analysis['average_z_scores']['mad']:.3f}")
    
    print("\n" + "-"*50)
    print("DETAILED SCENARIO RESULTS")
    print("-"*50)
    
    for result in results:
        print(f"\n{result.scenario_name}:")
        print(f"  Base: {result.base_value}, Noise: {result.noise_level}, "
              f"Outliers: {result.outlier_ratio:.1%} @ {result.outlier_magnitude}σ")
        print(f"  True regression: {result.true_regression}")
        print(f"  stddev z-score: {result.stddev_z_score:.3f} (detected: {result.stddev_detected})")
        print(f"  MAD z-score:    {result.mad_z_score:.3f} (detected: {result.mad_detected})")


def save_results_json(results: List[TestResult], analysis: Dict[str, Any], filename: str):
    """Save results to a JSON file."""
    data = {
        "analysis": analysis,
        "scenarios": [
            {
                "scenario_name": r.scenario_name,
                "base_value": r.base_value,
                "noise_level": r.noise_level,
                "outlier_ratio": r.outlier_ratio,
                "outlier_magnitude": r.outlier_magnitude,
                "stddev_z_score": r.stddev_z_score,
                "mad_z_score": r.mad_z_score,
                "stddev_detected": r.stddev_detected,
                "mad_detected": r.mad_detected,
                "true_regression": r.true_regression,
            }
            for r in results
        ]
    }
    
    with open(filename, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"\nResults saved to {filename}")


def main():
    """Main evaluation function."""
    parser = argparse.ArgumentParser(description="Evaluate dispersion methods for git-perf")
    parser.add_argument("--output", "-o", default="dispersion_evaluation_results.json",
                       help="Output JSON file for results")
    parser.add_argument("--verbose", "-v", action="store_true",
                       help="Verbose output")
    
    args = parser.parse_args()
    
    print("Starting dispersion method evaluation...")
    print("This may take a moment as we run multiple test scenarios...")
    
    # Run the evaluation
    results = run_evaluation_suite()
    
    # Analyze results
    analysis = analyze_results(results)
    
    # Print results
    print_results(results, analysis)
    
    # Save to JSON
    save_results_json(results, analysis, args.output)
    
    # Print conclusions
    print("\n" + "="*80)
    print("CONCLUSIONS")
    print("="*80)
    
    if analysis['mad']['false_positive_rate'] < analysis['stddev']['false_positive_rate']:
        print("✅ MAD shows lower false positive rate - better at avoiding false alarms")
    else:
        print("⚠️  MAD does not show lower false positive rate")
    
    if analysis['mad']['precision'] > analysis['stddev']['precision']:
        print("✅ MAD shows higher precision - better at correctly identifying regressions")
    else:
        print("⚠️  MAD does not show higher precision")
    
    if analysis['average_z_scores']['mad'] < analysis['average_z_scores']['stddev']:
        print("✅ MAD shows lower average z-scores - more conservative detection")
    else:
        print("⚠️  MAD does not show lower average z-scores")
    
    print("\nRecommendation:")
    if (analysis['mad']['false_positive_rate'] < analysis['stddev']['false_positive_rate'] and
        analysis['mad']['precision'] >= analysis['stddev']['precision']):
        print("🎯 MAD appears to be more robust for performance regression detection")
        print("   Consider using MAD as the default dispersion method for git-perf")
    else:
        print("🤔 Results are mixed - both methods have their place")
        print("   Consider using stddev for sensitive detection, MAD for robust detection")


if __name__ == "__main__":
    main()