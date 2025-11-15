# Change Point Detection Implementation Proposal for git-perf

**Date**: November 12, 2025
**Author**: Terry (Terragon Labs)
**Reference**: Netflix Tech Blog - "Fixing Performance Regressions Before They Happen"

---

## Executive Summary

This proposal outlines a comprehensive implementation plan for integrating **change point detection** into the git-perf performance measurement tracking system. The implementation will enable automatic detection of performance regime changes in historical data, complementing the existing z-score regression testing.

**Key Benefits**:
- **Proactive Detection**: Identify when performance changes occurred, not just if they occurred
- **Historical Analysis**: Understand performance evolution over time
- **Root Cause Analysis**: Pinpoint commits that introduced regressions or improvements
- **Reduced False Positives**: Statistical rigor in separating noise from real changes

---

## Background: Netflix's Approach

### The Problem
Netflix's TVUI team needed to detect performance anomalies before release—ideally before commits reach the codebase. Traditional threshold-based approaches generated too many false positives.

### The Solution: E-Divisive Algorithm
Netflix implemented the **E-Divisive** (Energy Divisive) algorithm to:
1. Analyze the 100 most recent test runs
2. Detect change points (significant shifts in performance)
3. Focus on regressions (ignore improvements by default)
4. Reduce false positives through statistical rigor

### Key Insight
E-Divisive is a **non-parametric, hierarchical divisive** algorithm that:
- Doesn't assume normal distribution
- Handles multivariate data
- Detects multiple change points
- Works well with time series performance data

### Limitations Identified
- **Computational Cost**: E-Divisive becomes infeasible for n ≥ 1000 observations
- **False Positives**: Even with E-Divisive, human review remains important
- **Configuration Complexity**: Requires tuning of significance levels

---

## Proposed Algorithms for git-perf

Based on research and the git-perf codebase characteristics, we propose a **multi-algorithm approach** with three options:

### 1. PELT (Pruned Exact Linear Time) - **RECOMMENDED FOR MVP**

**Why PELT?**
- **Optimal Speed**: O(n) complexity under mild conditions (vs O(n³) for E-Divisive)
- **Exact Detection**: Mathematically proven optimal segmentation
- **Scalable**: Handles 1000+ data points efficiently
- **Multiple Change Points**: Detects all significant changes in one pass
- **Well-Studied**: Strong theoretical foundation (Killick et al., 2012)

**How It Works**:
1. Dynamic programming approach with pruning
2. Minimizes penalized cost function
3. Cost = segment fit quality + penalty for each change point
4. Pruning eliminates impossible solutions, maintaining exactness

**Implementation Complexity**: ~300-400 lines of Rust code

**Example Output**:
```
Analysis: 3 change points detected (PELT algorithm)
  → Commit a1b2c3d (20 commits ago): +15.2% regression
  → Commit d4e5f6g (12 commits ago): -3.1% improvement
  → Commit h7i8j9k (5 commits ago): +8.7% regression
```

### 2. E-Divisive (Energy Divisive) - **For Robustness**

**Why E-Divisive?**
- **Non-parametric**: No distribution assumptions
- **Industry Proven**: Used by Netflix, MongoDB
- **Robust to Outliers**: Energy statistics handle noise well
- **Multivariate Ready**: Can extend to multiple metrics

**Limitations**:
- Slower: O(n²) to O(n³) complexity
- Limited to ~100-200 data points in practice
- More complex implementation

**Implementation Complexity**: ~500-700 lines of Rust code

**When to Use**: Noisy data, suspected non-normal distributions

### 3. Binary Segmentation - **For Simplicity**

**Why Binary Segmentation?**
- **Simple**: Easiest to understand and implement
- **Fast**: O(n log n) complexity
- **Tunable**: Easy to adjust sensitivity
- **Good Baseline**: Useful for testing infrastructure

**Limitations**:
- Suboptimal: May miss simultaneous changes
- Greedy: Makes locally optimal choices

**Implementation Complexity**: ~200-300 lines of Rust code

**When to Use**: Educational purposes, baseline comparisons

---

## Recommended Implementation Strategy

### Phase 1: MVP with PELT (Weeks 1-2)

**Goal**: Working change point detection integrated into audit system

**Deliverables**:
1. New module: `git_perf/src/change_point.rs` (~400 lines)
2. PELT algorithm implementation
3. Integration with existing audit system
4. Configuration support
5. Unit tests and documentation

**Integration Point**: Embedded in `audit.rs` (Option A from exploration docs)

**Code Changes**:
- New: `change_point.rs` (400 lines)
- Modified: `audit.rs` (20 lines)
- Modified: `config.rs` (30 lines)
- Tests: `change_point.rs` (100 lines)

### Phase 2: Multiple Algorithms (Weeks 3-4)

**Goal**: Add E-Divisive and Binary Segmentation, allow user selection

**Deliverables**:
1. E-Divisive implementation (~500 lines)
2. Binary Segmentation implementation (~200 lines)
3. Algorithm selection via config/CLI
4. Comparative benchmarking suite

### Phase 3: Enhanced Features (Weeks 5-8)

**Goal**: Production-ready with advanced capabilities

**Deliverables**:
1. Confidence scoring for change points
2. Trend analysis (slope detection)
3. Visual integration in HTML reports
4. CSV export of change point metadata
5. CI/CD integration examples

### Phase 4: Advanced Analytics (Future)

**Potential Extensions**:
1. Anomaly detection (single-point outliers)
2. Seasonal pattern recognition
3. Auto-correlation analysis
4. Predictive regression warnings
5. GitHub Actions annotations

---

## Technical Architecture

### Data Flow

```
┌────────────────────────────────────────────────────────┐
│ User runs: git perf audit -m build_time               │
└───────────────────────┬────────────────────────────────┘
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│ audit_with_commits() in audit.rs                       │
│ - Retrieve measurements via existing pipeline          │
│ - Filter by name, selectors, epoch                     │
│ - Aggregate by reduction function                      │
└───────────────────────┬────────────────────────────────┘
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│ Collect Vec<f64> time series (HEAD backwards)          │
│ Example: [15.2, 15.1, 15.0, 10.3, 10.2, 10.1, 10.0]   │
└───────────────────────┬────────────────────────────────┘
                        │
              ┌─────────┴─────────┐
              │                   │
              ▼                   ▼
┌──────────────────────┐  ┌──────────────────────┐
│ Z-Score Test         │  │ Change Point         │
│ (existing)           │  │ Detection (new)      │
│                      │  │                      │
│ Compares HEAD vs     │  │ Segments entire      │
│ historical tail      │  │ time series          │
│                      │  │                      │
│ Returns: Pass/Fail   │  │ Returns: Vec<CP>     │
└──────────────────────┘  └──────────────────────┘
              │                   │
              └─────────┬─────────┘
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│ Format and output results                              │
│                                                        │
│ ✅ 'build_time'                                        │
│ z-score (stddev): ↓ 0.42                              │
│ Head: μ: 9.87 ms σ: 0.12 MAD: 0.08 n: 1              │
│ Tail: μ: 10.23 ms σ: 0.34 MAD: 0.25 n: 20            │
│  [-3.52% – +0.73%] ▄▅▅▆▇█                            │
│                                                        │
│ Change Points Detected:                                │
│   → Commit a1b2c3d (5 commits ago): +15.2% increase   │
│   → Commit x7y8z9w (2 commits ago): -4.1% decrease    │
└────────────────────────────────────────────────────────┘
```

### Module Structure

```rust
// git_perf/src/change_point.rs

use crate::stats::{Stats, DispersionMethod};
use crate::measurement_retrieval::CommitSummary;
use anyhow::Result;

/// Represents a detected change point in time series data
#[derive(Debug, Clone)]
pub struct ChangePoint {
    /// Index in time series (0 = oldest, n-1 = newest)
    pub index: usize,

    /// Git commit SHA where change occurred
    pub commit_sha: String,

    /// Magnitude of change (percentage)
    pub magnitude_pct: f64,

    /// Statistical confidence [0.0, 1.0]
    pub confidence: f64,

    /// Direction of change
    pub direction: ChangeDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum ChangeDirection {
    Increase,  // Performance regression (slower)
    Decrease,  // Performance improvement (faster)
}

/// Algorithms for change point detection
#[derive(Debug, Clone, Copy)]
pub enum Algorithm {
    PELT,       // Pruned Exact Linear Time (recommended)
    EDivisive,  // Energy Divisive (robust)
    BinSeg,     // Binary Segmentation (simple)
}

/// Configuration for change point detection
pub struct ChangePointConfig {
    /// Minimum data points required
    pub min_data_points: usize,

    /// Algorithm to use
    pub algorithm: Algorithm,

    /// Minimum magnitude to report (percentage)
    pub min_magnitude_pct: f64,

    /// Confidence threshold for reporting
    pub confidence_threshold: f64,

    /// Penalty parameter for PELT (controls sensitivity)
    pub penalty: f64,

    /// Report improvements as well as regressions
    pub include_improvements: bool,
}

impl Default for ChangePointConfig {
    fn default() -> Self {
        Self {
            min_data_points: 10,
            algorithm: Algorithm::PELT,
            min_magnitude_pct: 5.0,
            confidence_threshold: 0.8,
            penalty: 3.0,  // Similar to BIC penalty
            include_improvements: true,
        }
    }
}

/// Detect change points in time series data
pub fn detect_change_points(
    measurements: &[f64],
    commits: &[CommitSummary],
    config: &ChangePointConfig,
) -> Result<Vec<ChangePoint>> {
    if measurements.len() < config.min_data_points {
        return Ok(Vec::new());
    }

    match config.algorithm {
        Algorithm::PELT => pelt::detect(measurements, commits, config),
        Algorithm::EDivisive => edivisive::detect(measurements, commits, config),
        Algorithm::BinSeg => binseg::detect(measurements, commits, config),
    }
}

/// PELT algorithm implementation
mod pelt {
    use super::*;

    pub fn detect(
        measurements: &[f64],
        commits: &[CommitSummary],
        config: &ChangePointConfig,
    ) -> Result<Vec<ChangePoint>> {
        // Implementation details below
        todo!()
    }

    /// Cost function for segment [start, end)
    fn segment_cost(measurements: &[f64], start: usize, end: usize) -> f64 {
        // Sum of squared errors from mean
        let segment = &measurements[start..end];
        let mean = segment.iter().sum::<f64>() / segment.len() as f64;
        segment.iter().map(|x| (x - mean).powi(2)).sum()
    }
}

mod edivisive {
    use super::*;

    pub fn detect(
        measurements: &[f64],
        commits: &[CommitSummary],
        config: &ChangePointConfig,
    ) -> Result<Vec<ChangePoint>> {
        todo!()
    }
}

mod binseg {
    use super::*;

    pub fn detect(
        measurements: &[f64],
        commits: &[CommitSummary],
        config: &ChangePointConfig,
    ) -> Result<Vec<ChangePoint>> {
        todo!()
    }
}

/// Format change points for output
pub fn format_change_points(change_points: &[ChangePoint]) -> String {
    if change_points.is_empty() {
        return String::from("No significant change points detected");
    }

    let mut output = String::from("Change Points Detected:\n");
    for cp in change_points {
        let direction = match cp.direction {
            ChangeDirection::Increase => "↑",
            ChangeDirection::Decrease => "↓",
        };
        output.push_str(&format!(
            "  {} Commit {} (index {}): {}{:.1}% (confidence: {:.0}%)\n",
            direction,
            &cp.commit_sha[..7],
            cp.index,
            if cp.magnitude_pct > 0.0 { "+" } else { "" },
            cp.magnitude_pct,
            cp.confidence * 100.0
        ));
    }
    output
}
```

---

## PELT Algorithm Implementation Details

### Mathematical Foundation

**Objective**: Find segmentation τ = {τ₀, τ₁, ..., τₘ} that minimizes:

```
F(τ) = Σ[C(yτᵢ₊₁:τᵢ)] + β·m
```

Where:
- `C(yτᵢ₊₁:τᵢ)` = cost of segment from τᵢ₊₁ to τᵢ
- `β` = penalty for each change point
- `m` = number of change points

**Cost Function** (for normally distributed data):
```
C(y₁:ₙ) = Σ(yᵢ - ȳ)²
```

### Dynamic Programming Approach

```rust
fn pelt_algorithm(measurements: &[f64], penalty: f64) -> Vec<usize> {
    let n = measurements.len();
    let mut F = vec![0.0; n + 1];  // Optimal cost up to point t
    let mut cp = vec![0; n + 1];   // Last change point before t
    let mut R = vec![0];           // Active set (pruning)

    for t in 1..=n {
        let mut min_cost = f64::INFINITY;
        let mut min_tau = 0;
        let mut new_R = Vec::new();

        for &tau in &R {
            let cost = F[tau] + segment_cost(measurements, tau, t) + penalty;

            // Pruning condition: if cost better than current min, keep in active set
            if cost + segment_cost(measurements, t, n) < min_cost {
                new_R.push(tau);
            }

            if cost < min_cost {
                min_cost = cost;
                min_tau = tau;
            }
        }

        new_R.push(t);  // Current point always active
        R = new_R;
        F[t] = min_cost;
        cp[t] = min_tau;
    }

    // Backtrack to find change points
    let mut change_points = Vec::new();
    let mut current = n;
    while current > 0 {
        let prev = cp[current];
        if prev > 0 {
            change_points.push(prev);
        }
        current = prev;
    }
    change_points.reverse();
    change_points
}
```

### Penalty Selection

**Recommended Approaches**:

1. **BIC (Bayesian Information Criterion)**:
   ```
   β = log(n) · σ²
   ```
   Where σ² is the variance of the data.

2. **AIC (Akaike Information Criterion)**:
   ```
   β = 2 · σ²
   ```

3. **Manual Tuning** (git-perf approach):
   ```toml
   [change_point]
   penalty_multiplier = 3.0  # Conservative (fewer change points)
   # penalty_multiplier = 1.0  # Aggressive (more change points)
   ```

---

## Configuration Integration

### .gitperfconfig Extension

```toml
# Global change point detection settings
[change_point]
enabled = true
algorithm = "pelt"              # pelt, edivisive, binseg
min_data_points = 10
min_magnitude_pct = 5.0
confidence_threshold = 0.8
penalty_multiplier = 3.0
include_improvements = true

# Measurement-specific overrides
[change_point."build_time"]
algorithm = "pelt"
penalty_multiplier = 2.0        # More sensitive for build times

[change_point."test_execution_time"]
algorithm = "edivisive"         # Use robust algorithm for noisy tests
min_magnitude_pct = 10.0        # Only report large changes

[change_point."memory_usage"]
enabled = false                 # Disable for this metric
```

### CLI Integration

```bash
# Embedded in audit (default behavior if enabled in config)
git perf audit -m build_time

# Force enable/disable change point detection
git perf audit -m build_time --detect-changes
git perf audit -m build_time --no-detect-changes

# Override algorithm
git perf audit -m build_time --cpd-algorithm edivisive

# Standalone analysis (Phase 2)
git perf analyze-change-points -m build_time -n 100
git perf analyze-change-points -m "test_*" --algorithm pelt
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_change_point() {
        // Clear regime shift: 10 → 20
        let data = vec![
            10.0, 10.1, 9.9, 10.2, 10.0,  // Regime 1
            20.0, 19.9, 20.1, 20.0, 19.8,  // Regime 2
        ];

        let config = ChangePointConfig::default();
        let commits = create_mock_commits(data.len());
        let cps = detect_change_points(&data, &commits, &config).unwrap();

        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0].index, 5);
        assert!(cps[0].magnitude_pct > 90.0);
        assert!(cps[0].confidence > 0.95);
    }

    #[test]
    fn test_multiple_change_points() {
        let data = vec![
            5.0, 5.0, 5.0,      // Regime 1
            10.0, 10.0, 10.0,   // Regime 2
            15.0, 15.0, 15.0,   // Regime 3
        ];

        let config = ChangePointConfig::default();
        let commits = create_mock_commits(data.len());
        let cps = detect_change_points(&data, &commits, &config).unwrap();

        assert_eq!(cps.len(), 2);
        assert_eq!(cps[0].index, 3);
        assert_eq!(cps[1].index, 6);
    }

    #[test]
    fn test_no_change_points() {
        // Stable with noise
        let data = vec![10.0, 10.1, 9.9, 10.2, 10.0, 10.1, 9.8, 10.3];

        let config = ChangePointConfig::default();
        let commits = create_mock_commits(data.len());
        let cps = detect_change_points(&data, &commits, &config).unwrap();

        assert!(cps.is_empty());
    }

    #[test]
    fn test_penalty_affects_sensitivity() {
        let data = vec![
            10.0, 10.0, 10.0,
            10.5, 10.5, 10.5,  // Small change
        ];

        let commits = create_mock_commits(data.len());

        // High penalty (conservative) - should miss small change
        let mut config = ChangePointConfig::default();
        config.penalty = 10.0;
        let cps_conservative = detect_change_points(&data, &commits, &config).unwrap();

        // Low penalty (sensitive) - should detect small change
        config.penalty = 1.0;
        let cps_sensitive = detect_change_points(&data, &commits, &config).unwrap();

        assert!(cps_sensitive.len() >= cps_conservative.len());
    }

    #[test]
    fn test_insufficient_data() {
        let data = vec![10.0, 10.0, 10.0];  // Only 3 points

        let mut config = ChangePointConfig::default();
        config.min_data_points = 5;
        let commits = create_mock_commits(data.len());
        let cps = detect_change_points(&data, &commits, &config).unwrap();

        assert!(cps.is_empty());
    }
}
```

### Integration Tests

```bash
#!/bin/bash
# Test with real git repository

set -e

# Setup test repository
git init test_repo
cd test_repo
git config user.name "Test"
git config user.email "test@test.com"

# Baseline performance (10ms)
for i in {1..5}; do
    git perf add build_time 10
    git commit --allow-empty -m "Baseline $i"
done

# Regression (15ms)
for i in {1..5}; do
    git perf add build_time 15
    git commit --allow-empty -m "Regression $i"
done

# Improvement (12ms)
for i in {1..5}; do
    git perf add build_time 12
    git commit --allow-empty -m "Improvement $i"
done

# Run audit with change point detection
output=$(git perf audit -m build_time --detect-changes)

# Verify change points detected
echo "$output" | grep "Change Points Detected"
echo "$output" | grep "2 change points"

# Verify magnitude calculations
echo "$output" | grep "+50.*%" # 10→15 is +50%
echo "$output" | grep "-20.*%" # 15→12 is -20%

echo "✅ Integration test passed"
```

### Benchmarking

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;

    #[test]
    fn bench_pelt_performance() {
        let sizes = vec![10, 50, 100, 500, 1000];

        for n in sizes {
            let data: Vec<f64> = (0..n).map(|i| {
                if i < n/2 { 10.0 } else { 20.0 }
            }).collect();

            let start = std::time::Instant::now();
            let config = ChangePointConfig::default();
            let commits = create_mock_commits(n);
            let _ = detect_change_points(&data, &commits, &config).unwrap();
            let elapsed = start.elapsed();

            println!("PELT n={}: {:?}", n, elapsed);

            // Should be roughly linear
            assert!(elapsed.as_millis() < n as u128 * 10);
        }
    }
}
```

---

## Output Format Examples

### Audit with Change Points (Embedded)

```
$ git perf audit -m build_time

✅ 'build_time'
z-score (stddev): ↑ 2.34
Head: μ: 15.12 ms σ: 0.23 MAD: 0.15 n: 1
Tail: μ: 10.45 ms σ: 0.52 MAD: 0.38 n: 25
 [+39.2% – +51.8%] ▃▃▃▄▄▅▅██

⚠️  Performance regression detected at HEAD

Change Points Detected (PELT, n=25):
  ↑ Commit a1b2c3d (commit 5): +44.7% (confidence: 99%)
     └─ Suspected regression point
  ↓ Commit d4e5f6g (commit 12): -3.2% (confidence: 87%)
     └─ Minor variation (within noise)
  ↑ Commit h7i8j9k (commit 20): +2.1% (confidence: 73%)
     └─ Insignificant (below 5% threshold)
```

### Standalone Analysis (Phase 2)

```
$ git perf analyze-change-points -m test_execution_time -n 100

Analyzing 'test_execution_time' over 100 commits...

Algorithm: PELT (Pruned Exact Linear Time)
Data points: 87 (13 excluded: epoch boundaries)
Penalty: 3.0 (BIC-based)

═══════════════════════════════════════════════════════════

Detected 4 significant change points:

1. Commit 5c3e8f2 (Jan 25, 2024) - 87 commits ago
   ├─ Magnitude: +34.7% (12.5s → 16.8s)
   ├─ Confidence: 99%
   ├─ Direction: ↑ Regression
   └─ Likely cause: Infrastructure change or test expansion

2. Commit a2f4d9e (Jan 18, 2024) - 64 commits ago
   ├─ Magnitude: -8.3% (16.8s → 15.4s)
   ├─ Confidence: 94%
   ├─ Direction: ↓ Improvement
   └─ Likely cause: Optimization or test removal

3. Commit f1e2d3c (Jan 12, 2024) - 42 commits ago
   ├─ Magnitude: +12.1% (15.4s → 17.2s)
   ├─ Confidence: 91%
   ├─ Direction: ↑ Regression
   └─ Likely cause: Feature addition

4. Commit x9y8z7w (Jan 5, 2024) - 18 commits ago
   ├─ Magnitude: -15.2% (17.2s → 14.6s)
   ├─ Confidence: 96%
   ├─ Direction: ↓ Improvement
   └─ Likely cause: Performance optimization

═══════════════════════════════════════════════════════════

Summary:
  • Net change: +16.8% (12.5s → 14.6s)
  • Regressions: 2 (+46.8% total)
  • Improvements: 2 (-23.5% total)
  • Current trend: Stable (last 18 commits)

Recommendation: Investigate commit 5c3e8f2 for largest regression
```

### HTML Report Integration (Phase 3)

```html
<!-- Enhanced Plotly graph with change point markers -->
<script>
var trace = {
    x: commits,
    y: measurements,
    type: 'scatter',
    mode: 'lines+markers',
    name: 'build_time'
};

var shapes = [
    // Vertical line at change point 1
    {
        type: 'line',
        x0: 5, x1: 5,
        y0: 0, y1: 1,
        yref: 'paper',
        line: {
            color: 'red',
            width: 2,
            dash: 'dash'
        }
    },
    // Vertical line at change point 2
    {
        type: 'line',
        x0: 12, x1: 12,
        y0: 0, y1: 1,
        yref: 'paper',
        line: {
            color: 'orange',
            width: 2,
            dash: 'dash'
        }
    }
];

var annotations = [
    {
        x: 5,
        y: measurements[5],
        text: 'Regression: +44.7%<br>Commit: a1b2c3d',
        showarrow: true,
        arrowhead: 2,
        bgcolor: 'rgba(255,0,0,0.8)'
    }
];

var layout = {
    title: 'Build Time - Change Point Analysis',
    shapes: shapes,
    annotations: annotations
};

Plotly.newPlot('chart', [trace], layout);
</script>
```

---

## Implementation Checklist

### Phase 1: MVP (Weeks 1-2)

#### Week 1: Core Implementation
- [ ] Create `git_perf/src/change_point.rs`
- [ ] Define data structures (`ChangePoint`, `Algorithm`, `Config`)
- [ ] Implement PELT algorithm
  - [ ] Dynamic programming core
  - [ ] Pruning logic
  - [ ] Cost function (sum of squared errors)
  - [ ] Backtracking for change point extraction
- [ ] Implement penalty calculation (BIC-based)
- [ ] Add magnitude and confidence calculation
- [ ] Write unit tests (10+ test cases)
  - [ ] Single change point
  - [ ] Multiple change points
  - [ ] No change points (stable data)
  - [ ] Noisy data
  - [ ] Edge cases (insufficient data, etc.)

#### Week 2: Integration & Configuration
- [ ] Integrate into `audit.rs`
  - [ ] Call change point detection after z-score test
  - [ ] Pass time series and commit data
  - [ ] Format and display results
- [ ] Extend configuration system
  - [ ] Add `[change_point]` section to config
  - [ ] Measurement-specific overrides
  - [ ] CLI flags (`--detect-changes`, `--cpd-algorithm`)
- [ ] Update documentation
  - [ ] User guide for change point detection
  - [ ] Configuration examples
  - [ ] Interpretation guide (what do results mean?)
- [ ] Run full test suite
  - [ ] `cargo fmt`
  - [ ] `cargo nextest run -- --skip slow`
  - [ ] `cargo clippy`
- [ ] Integration testing with real repository
- [ ] Generate manpages (`./scripts/generate-manpages.sh`)

### Phase 2: Multiple Algorithms (Weeks 3-4)

- [ ] Implement E-Divisive algorithm
  - [ ] Energy statistics calculation
  - [ ] Hierarchical divisive approach
  - [ ] Significance testing
- [ ] Implement Binary Segmentation
  - [ ] Greedy segmentation
  - [ ] Stopping criterion
- [ ] Add algorithm selection logic
- [ ] Benchmark algorithms (speed vs. accuracy)
- [ ] Comparative tests
- [ ] Update documentation

### Phase 3: Enhanced Features (Weeks 5-8)

- [ ] Confidence scoring refinement
- [ ] Trend analysis (segment slopes)
- [ ] HTML report integration
  - [ ] Plotly vertical line markers
  - [ ] Hover tooltips with change point info
  - [ ] Shaded regions between change points
- [ ] CSV export enhancement
  - [ ] Add change point metadata columns
  - [ ] Segment assignments
- [ ] Performance optimization
  - [ ] Profile PELT implementation
  - [ ] Optimize segment cost calculation
  - [ ] Consider caching strategies

---

## Dependencies and Libraries

### Rust Ecosystem

**Potential Dependencies**:
1. **Statistical Functions** (if needed beyond existing `stats.rs`):
   - `statrs` - Statistical distributions and tests
   - `ndarray` - N-dimensional arrays for matrix operations

2. **Optimization** (for E-Divisive):
   - `nalgebra` - Linear algebra
   - `ndarray-stats` - Statistical operations on arrays

**Recommendation**: Start with **zero external dependencies** for PELT MVP.
- Use existing `stats.rs` infrastructure
- Implement algorithms from scratch for full control
- Add dependencies only if complexity justifies them

### Reference Implementations

1. **Python - ruptures**:
   - Excellent reference for PELT implementation
   - MIT licensed, readable code
   - URL: https://github.com/deepcharles/ruptures

2. **R - changepoint**:
   - Original PELT paper authors' implementation
   - GPL licensed
   - URL: https://github.com/rkillick/changepoint

3. **Rust - fastpelt**:
   - Rust implementation of PELT
   - MIT licensed
   - URL: https://github.com/ritchie46/fastpelt

---

## Success Metrics

### Technical Metrics

1. **Performance**:
   - PELT runs in O(n) time for n ≤ 1000
   - Detection completes in < 100ms for typical datasets (n=40)
   - Memory usage < 10MB for n=1000

2. **Accuracy**:
   - True positive rate > 90% on synthetic data
   - False positive rate < 10%
   - Correct change point localization within ±2 commits

3. **Code Quality**:
   - All tests pass (`cargo nextest run`)
   - No clippy warnings (`cargo clippy`)
   - Code coverage > 80% for `change_point.rs`

### User Experience Metrics

1. **Discoverability**:
   - Change point detection mentioned in `--help`
   - Configuration documented in manpages
   - Examples in README

2. **Usability**:
   - Works with default configuration (no tuning required)
   - Clear, actionable output
   - Integration with existing workflows (audit)

3. **Reliability**:
   - No false positives on stable data
   - Graceful handling of insufficient data
   - Robust to outliers (when using MAD dispersion)

---

## Risk Mitigation

### Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| PELT complexity | High | Reference implementations, incremental development |
| False positives | High | Conservative default penalty, confidence thresholds |
| Performance issues | Medium | Profiling, benchmarking, optimization |
| Integration bugs | Medium | Comprehensive testing, gradual rollout |

### User Adoption Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Confusing output | High | Clear formatting, interpretation guide |
| Configuration complexity | Medium | Sensible defaults, progressive disclosure |
| Feature overload | Low | Optional (disabled by default initially), clear docs |

---

## Documentation Plan

### User Documentation

1. **README.md Update**:
   - Add "Change Point Detection" section
   - Quick start example
   - Link to detailed guide

2. **CHANGE_POINT_GUIDE.md** (new):
   - What is change point detection?
   - When to use it
   - How to interpret results
   - Configuration guide
   - Algorithm selection guide
   - Troubleshooting

3. **Manpage Updates**:
   - `git-perf-audit(1)`: Add change point detection flags
   - `git-perf-config(5)`: Document `[change_point]` section
   - `git-perf-analyze-change-points(1)`: New manpage (Phase 2)

### Developer Documentation

1. **ARCHITECTURE.md Update**:
   - Add change point detection module
   - Algorithm descriptions
   - Data flow diagrams

2. **CONTRIBUTING.md**:
   - How to add new algorithms
   - Testing requirements
   - Performance benchmarking

3. **Code Comments**:
   - Inline documentation for algorithms
   - Mathematical foundations explained
   - Performance considerations noted

---

## Timeline and Milestones

### Week 1-2: MVP
**Deliverable**: Working PELT implementation integrated into audit

**Milestones**:
- Day 1-3: PELT algorithm implementation + unit tests
- Day 4-5: Integration into audit.rs
- Day 6-7: Configuration support
- Day 8-10: Testing and documentation

### Week 3-4: Multiple Algorithms
**Deliverable**: E-Divisive and Binary Segmentation available

**Milestones**:
- Week 3: E-Divisive implementation
- Week 4: Binary Segmentation + comparative testing

### Week 5-8: Enhanced Features
**Deliverable**: Production-ready with visual integration

**Milestones**:
- Week 5-6: HTML report integration
- Week 7: CSV export and CI/CD examples
- Week 8: Performance optimization and polish

---

## Conclusion

This proposal outlines a comprehensive, phased approach to integrating change point detection into git-perf:

✅ **Proven Approach**: Based on Netflix's success and academic research
✅ **Right Algorithm**: PELT balances speed, accuracy, and scalability
✅ **Clean Integration**: Leverages existing infrastructure seamlessly
✅ **Low Risk**: Incremental rollout with conservative defaults
✅ **High Value**: Answers "when did performance change?" - critical for debugging

**Recommended Next Steps**:
1. Review and approve this proposal
2. Create feature branch: `feature/change-point-detection`
3. Begin Phase 1 implementation
4. Weekly check-ins to review progress

**Estimated Total Effort**: 6-8 weeks for full implementation (Phases 1-3)

---

## References

1. **Netflix Tech Blog**: "Fixing Performance Regressions Before They Happen"
   https://netflixtechblog.com/fixing-performance-regressions-before-they-happen-eab2602b86fe

2. **Killick et al. (2012)**: "Optimal Detection of Changepoints With a Linear Computational Cost"
   https://arxiv.org/abs/1101.1438

3. **Matteson & James (2014)**: "A Nonparametric Approach for Multiple Change Point Analysis"
   https://arxiv.org/abs/1306.4933

4. **Truong et al. (2020)**: "Selective review of offline change point detection methods"
   Signal Processing, Volume 167

5. **MongoDB Engineering**: "Change Point Detection for Time Series Performance Regression"
   Applied to production systems at MongoDB

6. **ACM ICPE 2020**: "The Use of Change Point Detection to Identify Software Performance Regressions in a Continuous Integration System"

---

**Document Version**: 1.0
**Last Updated**: November 12, 2025
**Approval Status**: Pending Review
