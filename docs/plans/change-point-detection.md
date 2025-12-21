# Change Point Detection Implementation Proposal

**Date**: November 15, 2025
**Status**: Ready for Implementation

---

## Overview

Add change point detection to git-perf to identify **when** performance shifts occurred in historical data. This complements existing z-score regression testing by providing historical context.

**Key Features**:
1. Visualize epoch boundaries and change points in HTML reports (hidden by default, legend-toggleable)
2. Warn in audit if change points exist in current epoch (makes z-score results potentially unreliable)
3. Use PELT algorithm for efficient, exact change point detection

---

## Algorithm Choice: PELT

**Why PELT (Pruned Exact Linear Time)?**
- O(n) complexity (fast)
- Mathematically optimal segmentation
- Handles 1000+ data points
- Well-studied (Killick et al., 2012)

**Important**: PELT is an **offline algorithm** requiring historical data. It cannot immediately classify a single new value as a change point. **Z-score regression detection remains essential** for real-time alerts.

---

## Implementation Requirements

### 1. HTML Report Visualization (Primary Focus)

**Epoch Boundaries**:
- Vertical dashed gray lines where measurement epochs change
- Hidden by default (`visible: 'legendonly'`)
- Per-measurement grouping in legend
- User clicks legend to toggle visibility

**Change Points**:
- Vertical solid lines at detected regime shifts
- Red for regressions, green for improvements
- Hidden by default, legend-toggleable
- Hover shows magnitude and commit info

```
Legend (in HTML report):
[●] build_time           ← visible by default
[ ] build_time (Epochs)  ← hidden, click to show
[ ] build_time (Changes) ← hidden, click to show
```

### 2. Audit System Warning

When change point detected in current epoch:

```
⚠️  WARNING: Change point detected in current epoch at commit a1b2c3d (+23.5%)
    Historical z-score comparison may be unreliable due to regime shift.
    Consider bumping epoch or investigating the change.

✅ 'build_time'
z-score (stddev): ↑ 0.45
...
```

**Rationale**: A change point means the historical baseline is inconsistent, making z-score comparisons across different regimes misleading.

---

## Technical Design

### Core Data Structures

```rust
#[derive(Debug, Clone)]
pub struct ChangePoint {
    pub index: usize,           // Position in time series
    pub commit_sha: String,     // Git SHA
    pub magnitude_pct: f64,     // Percentage change
    pub confidence: f64,        // [0.0, 1.0]
    pub direction: ChangeDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum ChangeDirection {
    Increase,  // Regression (slower/larger)
    Decrease,  // Improvement (faster/smaller)
}

#[derive(Debug, Clone)]
pub struct EpochTransition {
    pub index: usize,        // Commit index
    pub from_epoch: u32,
    pub to_epoch: u32,
}

pub struct ChangePointConfig {
    pub min_data_points: usize,      // Default: 10
    pub min_magnitude_pct: f64,      // Default: 5.0
    pub confidence_threshold: f64,   // Default: 0.8
    pub penalty: f64,                // Default: 0.5 (lower = more sensitive)
}
```

### PELT Algorithm Core

```rust
pub fn detect_change_points(
    measurements: &[f64],
    config: &ChangePointConfig,
) -> Vec<usize> {
    let n = measurements.len();
    if n < config.min_data_points {
        return vec![];
    }

    let mut F = vec![-config.penalty; n + 1];
    let mut cp = vec![0usize; n + 1];
    let mut R = vec![0usize];

    for t in 1..=n {
        let (min_cost, best_tau) = R.iter()
            .map(|&tau| {
                let cost = F[tau] + segment_cost(measurements, tau, t) + config.penalty;
                (cost, tau)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();

        F[t] = min_cost;
        cp[t] = best_tau;

        // Pruning step
        R.retain(|&tau| F[tau] + segment_cost(measurements, tau, t) <= min_cost);
        R.push(t);
    }

    // Backtrack to find change points
    let mut result = vec![];
    let mut current = n;
    while cp[current] > 0 {
        result.push(cp[current]);
        current = cp[current];
    }
    result.reverse();
    result
}

fn segment_cost(measurements: &[f64], start: usize, end: usize) -> f64 {
    let segment = &measurements[start..end];
    let mean = segment.iter().sum::<f64>() / segment.len() as f64;
    segment.iter().map(|x| (x - mean).powi(2)).sum()
}
```

### Plotly Integration

```rust
impl PlotlyReporter {
    fn add_epoch_boundary_trace(
        &mut self,
        transitions: Vec<EpochTransition>,
        measurement_name: &str,
    ) {
        for transition in transitions {
            let x_pos = self.size - transition.index - 1;
            let trace = Scatter::new(
                vec![x_pos, x_pos],
                vec![self.y_min, self.y_max],
            )
            .name(format!("{} (Epoch {}→{})", measurement_name,
                         transition.from_epoch, transition.to_epoch))
            .legend_group(format!("{}_epochs", measurement_name))
            .visible(plotly::common::Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color("gray").dash(DashType::Dash).width(2.0))
            .show_legend(true);

            self.plot.add_trace(trace);
        }
    }

    fn add_change_point_trace(
        &mut self,
        change_points: Vec<ChangePoint>,
        measurement_name: &str,
    ) {
        for cp in change_points {
            let x_pos = self.size - cp.index - 1;
            let color = match cp.direction {
                ChangeDirection::Increase => "rgba(220, 53, 69, 0.8)", // red
                ChangeDirection::Decrease => "rgba(40, 167, 69, 0.8)",  // green
            };
            let label = format!("{} ({:+.1}%)", measurement_name, cp.magnitude_pct);

            let trace = Scatter::new(
                vec![x_pos, x_pos],
                vec![self.y_min, self.y_max],
            )
            .name(label)
            .legend_group(format!("{}_changes", measurement_name))
            .visible(plotly::common::Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color(color).width(3.0))
            .show_legend(true);

            self.plot.add_trace(trace);
        }
    }
}
```

---

## Files to Modify

### New Files
- `git_perf/src/change_point.rs` (~400 lines) - PELT algorithm and data structures

### Modified Files
- `git_perf/src/reporting.rs` (~150 lines) - Add epoch/change point visualization
- `git_perf/src/audit.rs` (~50 lines) - Add warning for change points in epoch
- `git_perf/src/cli.rs` (~30 lines) - Add CLI flags
- `git_perf/src/config.rs` (~30 lines) - Add configuration options

**Total**: ~660 lines of code

---

## CLI Interface

```bash
# Report with visualizations
git perf report output.html --show-epochs --show-changes

# Audit with change point warning
git perf audit -m build_time  # warns if change point in epoch

# Suppress warning
git perf audit -m build_time --no-change-point-warning
```

---

## Configuration

```toml
[change_point]
enabled = true
min_data_points = 10
min_magnitude_pct = 5.0
penalty = 0.5  # Default: balanced sensitivity (lower = more sensitive)

[change_point."build_time"]
penalty = 1.0  # Less sensitive for this measurement (if needed)

[change_point."memory_usage"]
penalty = 0.3  # More sensitive for detecting subtle memory changes
```

### Penalty Parameter Tuning Guide

The `penalty` parameter controls PELT's sensitivity to change points:

- **0.3-0.5**: High sensitivity - detects multiple change points, may catch smaller shifts
- **0.5-1.0**: Balanced (default 0.5) - good for most use cases
- **1.0-3.0**: Conservative - only detects major regime shifts
- **3.0+**: Very conservative - minimal change point detection

The algorithm scales this by `log(n) * variance` internally, so the same penalty value
adapts automatically to different data sizes and variability levels.

---

## Implementation Phases

### Phase 1: Core (Week 1-2)
1. Implement PELT algorithm in `change_point.rs`
2. Add epoch boundary detection in `reporting.rs`
3. Add visualization traces to PlotlyReporter
4. Unit tests for algorithm and visualization
5. Integration test with real repository

### Phase 2: Integration (Week 3)
1. Add audit warning for change points in epoch
2. CLI flags for report and audit
3. Configuration support
4. Documentation updates

### Phase 3: Polish (Week 4)
1. Hover tooltips with details
2. Confidence scoring refinement
3. CSV export with change point metadata
4. Manpage updates

---

## Testing

### Unit Test: Change Point Detection

```rust
#[test]
fn test_single_change_point() {
    let data = vec![10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0];
    let config = ChangePointConfig::default();
    let cps = detect_change_points(&data, &config);
    assert_eq!(cps, vec![4]);
}

#[test]
fn test_no_change_points() {
    let data = vec![10.0, 10.1, 9.9, 10.2, 10.0];
    let config = ChangePointConfig::default();
    let cps = detect_change_points(&data, &config);
    assert!(cps.is_empty());
}
```

### Integration Test: Visualization

```rust
#[test]
fn test_epoch_traces_hidden_by_default() {
    let mut reporter = PlotlyReporter::new();
    reporter.add_epoch_boundary_trace(vec![...], "test");
    let html = String::from_utf8_lossy(&reporter.as_bytes());
    assert!(html.contains("legendonly"));
}
```

---

## Success Criteria

1. **Visualization**: Epoch boundaries and change points toggleable in HTML legend
2. **Audit Warning**: Warns when change point exists in current epoch
3. **Performance**: PELT runs in <100ms for n=100
4. **Accuracy**: Correctly identifies regime shifts in test data
5. **User Experience**: Hidden by default, easy to enable via legend click

---

## References

1. Killick et al. (2012): "Optimal Detection of Changepoints With a Linear Computational Cost"
2. Netflix Tech Blog: "Fixing Performance Regressions Before They Happen"
3. Python ruptures library (reference implementation)
