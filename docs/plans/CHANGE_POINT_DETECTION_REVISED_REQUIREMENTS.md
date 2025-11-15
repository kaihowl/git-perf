# Change Point Detection - Revised Requirements

**Date**: November 15, 2025
**Status**: Requirements Update
**Previous Document**: CHANGE_POINT_DETECTION_PROPOSAL.md

---

## Revised Priority: HTML Report Visualization

The primary focus shifts from audit system integration to **HTML report visualization** with Plotly. The goal is to visually flag:

1. **Epoch Boundaries** - Where measurement epochs change
2. **Change Points** - Where statistically significant performance shifts occurred

Both visualizations should be **disabled by default** and toggleable via the legend.

---

## Key Behavioral Changes

### 1. Audit System: Warning Only (Not Blocking)

**Previous Behavior** (proposed):
```
✅ 'build_time'
z-score (stddev): ↑ 2.34
...
Change Points Detected:
  ↑ Commit a1b2c3d: +44.7% regression
```

**Revised Behavior**:
```
✅ 'build_time'
z-score (stddev): ↑ 2.34
...
⚠️  WARNING: Change point detected in current epoch at commit a1b2c3d
    Historical z-score comparison may be unreliable due to regime shift.
    Consider bumping epoch or investigating the change.
```

**Rationale**:
- A change point in the current epoch means the historical baseline is inconsistent
- Z-score comparison between different regimes is misleading
- User should be warned that audit results may be irrelevant, not given additional analysis

### 2. HTML Report: Primary Visualization Output

**Focus**: Per-measurement visualization of:
- Epoch boundaries (vertical markers showing where epochs changed)
- Change points (vertical markers showing detected regime shifts)
- Both hidden by default, user-toggleable via legend

---

## Plotly Visualization Design

### Visual Representation

```
┌─────────────────────────────────────────────────────────────┐
│  Performance Measurements                      Legend:      │
│                                               [●] build_time│
│                                               [ ] Epochs    │
│                                               [ ] Changes   │
│                                                             │
│  ms                                                         │
│  25│                                      █ █              │
│    │                                    █ █ █              │
│  20│                        ┊         █ █   █ ┊            │
│    │                        ┊       █ █       ┊            │
│  15│          ●   ●   ●   ●┊● ● ● █           ┊●           │
│    │        ●   ●   ●   ●  ┊● ● ●             ┊  ●         │
│  10│      ●                 ┊                  ┊    ●       │
│    │    ●                   ┊                  ┊            │
│   5│  ●                     ┊                  ┊            │
│    └────────────────────────┴──────────────────┴──────────→ │
│      abc  def  ghi  jkl   mno  pqr  stu  vwx  yz1  234     │
│                             ▲                   ▲           │
│                       Epoch Boundary      Change Point      │
│                       (dashed line)       (solid line)      │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Details

**Trace Types**:

1. **Measurement Data** - Scatter/Box plot (existing)
   - Visible by default
   - Primary data visualization

2. **Epoch Boundaries** - Vertical dashed lines
   - Hidden by default (`visible: 'legendonly'`)
   - Per measurement (same legend group)
   - Color: Gray/Blue (neutral, informational)
   - Style: Dashed line

3. **Change Points** - Vertical solid lines
   - Hidden by default (`visible: 'legendonly'`)
   - Per measurement (same legend group)
   - Color: Red (warning/regression) or Green (improvement)
   - Style: Solid or dash-dot line

### Plotly Configuration

```rust
use plotly::{
    common::{Line, Marker, Mode, DashType},
    layout::{Shape, ShapeLine, ShapeType},
    Scatter,
};

// Example: Adding epoch boundary markers
fn add_epoch_boundary_trace(
    plot: &mut Plot,
    measurement_name: &str,
    epoch_indices: Vec<usize>,
    total_commits: usize,
) {
    // Create invisible scatter points for legend entry
    let trace = Scatter::new(vec![], vec![])
        .name("Epoch Boundaries")
        .legend_group(format!("{}_epochs", measurement_name))
        .visible(plotly::common::Visible::LegendOnly)
        .mode(Mode::Lines)
        .line(Line::new().color("gray").dash(DashType::Dash));

    plot.add_trace(trace);

    // Add shapes for actual vertical lines (linked to the trace)
    for idx in epoch_indices {
        let x_pos = total_commits - idx - 1;
        let shape = Shape::new()
            .shape_type(ShapeType::Line)
            .x0(x_pos)
            .x1(x_pos)
            .y0(0.0)
            .y1(1.0)
            .y_ref("paper")
            .line(
                ShapeLine::new()
                    .color("gray")
                    .width(2.0)
                    .dash(DashType::Dash),
            );
        // Add to layout shapes
    }
}

// Example: Adding change point markers
fn add_change_point_trace(
    plot: &mut Plot,
    measurement_name: &str,
    change_points: Vec<ChangePoint>,
    total_commits: usize,
) {
    // Regression change points (red)
    let regression_trace = Scatter::new(vec![], vec![])
        .name("Change Points (Regression)")
        .legend_group(format!("{}_changes", measurement_name))
        .visible(plotly::common::Visible::LegendOnly)
        .mode(Mode::Lines)
        .line(Line::new().color("red").dash(DashType::DashDot));

    plot.add_trace(regression_trace);

    // Improvement change points (green)
    let improvement_trace = Scatter::new(vec![], vec![])
        .name("Change Points (Improvement)")
        .legend_group(format!("{}_changes", measurement_name))
        .visible(plotly::common::Visible::LegendOnly)
        .mode(Mode::Lines)
        .line(Line::new().color("green").dash(DashType::DashDot));

    plot.add_trace(improvement_trace);

    // Add shapes for vertical lines
    for cp in change_points {
        let x_pos = total_commits - cp.index - 1;
        let color = match cp.direction {
            ChangeDirection::Increase => "red",
            ChangeDirection::Decrease => "green",
        };

        let shape = Shape::new()
            .shape_type(ShapeType::Line)
            .x0(x_pos)
            .x1(x_pos)
            .y0(0.0)
            .y1(1.0)
            .y_ref("paper")
            .line(ShapeLine::new().color(color).width(2.0));
        // Add to layout shapes
    }
}
```

### Alternative Approach: Transparent Scatter Markers

Since Plotly shapes don't have legend toggle control, use scatter traces with vertical line segments:

```rust
// Epoch boundary as vertical line trace
fn create_epoch_boundary_trace(
    x_position: usize,
    y_min: f64,
    y_max: f64,
    measurement_name: &str,
) -> Box<dyn Trace> {
    Scatter::new(vec![x_position, x_position], vec![y_min, y_max])
        .name(format!("{} (Epoch)", measurement_name))
        .legend_group(format!("{}_markers", measurement_name))
        .visible(plotly::common::Visible::LegendOnly)
        .mode(Mode::Lines)
        .line(
            Line::new()
                .color("rgba(100, 100, 100, 0.7)")
                .width(2.0)
                .dash(DashType::Dash),
        )
        .show_legend(true)
}

// Change point as vertical line trace
fn create_change_point_trace(
    x_position: usize,
    y_min: f64,
    y_max: f64,
    magnitude_pct: f64,
    direction: ChangeDirection,
    measurement_name: &str,
) -> Box<dyn Trace> {
    let (color, label) = match direction {
        ChangeDirection::Increase if magnitude_pct > 0.0 => {
            ("rgba(220, 53, 69, 0.8)", format!("{} (+{:.1}%)", measurement_name, magnitude_pct))
        }
        ChangeDirection::Decrease => {
            ("rgba(40, 167, 69, 0.8)", format!("{} ({:.1}%)", measurement_name, magnitude_pct))
        }
        _ => ("rgba(108, 117, 125, 0.8)", format!("{} (Change)", measurement_name)),
    };

    Scatter::new(vec![x_position, x_position], vec![y_min, y_max])
        .name(label)
        .legend_group(format!("{}_changes", measurement_name))
        .visible(plotly::common::Visible::LegendOnly)
        .mode(Mode::Lines)
        .line(Line::new().color(color).width(3.0))
        .show_legend(true)
}
```

---

## Implementation Architecture

### Reporter Trait Extension

```rust
trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        measurement_name: &str,
        group_values: &[String],
    );

    fn add_summarized_trace(
        &mut self,
        indexed_measurements: Vec<(usize, MeasurementSummary)>,
        measurement_name: &str,
        group_values: &[String],
    );

    // NEW: Add epoch boundary markers
    fn add_epoch_boundaries(
        &mut self,
        epoch_transitions: Vec<EpochTransition>,
        measurement_name: &str,
    );

    // NEW: Add change point markers
    fn add_change_points(
        &mut self,
        change_points: Vec<ChangePoint>,
        measurement_name: &str,
    );

    fn as_bytes(&self) -> Vec<u8>;
}

#[derive(Debug, Clone)]
pub struct EpochTransition {
    pub index: usize,        // Commit index where epoch changes
    pub from_epoch: u32,     // Previous epoch number
    pub to_epoch: u32,       // New epoch number
}

#[derive(Debug, Clone)]
pub struct ChangePoint {
    pub index: usize,           // Commit index
    pub magnitude_pct: f64,     // Percentage change
    pub direction: ChangeDirection,
    pub confidence: f64,        // Statistical confidence
}
```

### Data Collection in Report Generation

```rust
pub fn report(
    output: PathBuf,
    separate_by: Vec<String>,
    num_commits: usize,
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
    combined_patterns: &[String],
    // NEW: Enable epoch/change point visualization
    show_epoch_boundaries: bool,
    detect_change_points: bool,
) -> Result<()> {
    // ... existing code ...

    for measurement_name in unique_measurement_names {
        // ... existing trace addition ...

        // NEW: Detect and add epoch boundaries
        if show_epoch_boundaries {
            let epoch_transitions = detect_epoch_transitions(
                &commits,
                measurement_name,
                key_values,
            );
            plot.add_epoch_boundaries(epoch_transitions, measurement_name);
        }

        // NEW: Detect and add change points
        if detect_change_points {
            let measurements_for_cpd: Vec<f64> = /* collect measurement values */;
            let change_points = change_point::detect(&measurements_for_cpd)?;
            plot.add_change_points(change_points, measurement_name);
        }
    }

    // ... rest of function ...
}

fn detect_epoch_transitions(
    commits: &[Commit],
    measurement_name: &str,
    key_values: &[(String, String)],
) -> Vec<EpochTransition> {
    let mut transitions = Vec::new();
    let mut prev_epoch: Option<u32> = None;

    for (index, commit) in commits.iter().enumerate() {
        for measurement in &commit.measurements {
            if measurement.name == measurement_name
                && measurement.key_values_is_superset_of(key_values)
            {
                if let Some(prev) = prev_epoch {
                    if measurement.epoch != prev {
                        transitions.push(EpochTransition {
                            index,
                            from_epoch: prev,
                            to_epoch: measurement.epoch,
                        });
                    }
                }
                prev_epoch = Some(measurement.epoch);
                break;
            }
        }
    }

    transitions
}
```

---

## User Interface

### CLI Flags

```bash
# Generate report with epoch boundaries shown (still hidden by default in legend)
git perf report output.html --show-epoch-boundaries

# Generate report with change point detection
git perf report output.html --detect-change-points

# Both
git perf report output.html --show-epoch-boundaries --detect-change-points

# With customization
git perf report output.html \
  --detect-change-points \
  --cpd-algorithm pelt \
  --cpd-min-confidence 0.9
```

### In HTML Report

1. User opens HTML report
2. Sees measurement traces (enabled by default)
3. In legend, sees grayed-out entries:
   - "build_time (Epochs)"
   - "build_time (Changes)"
4. Clicks on "build_time (Epochs)" - vertical dashed lines appear at epoch boundaries
5. Clicks on "build_time (Changes)" - vertical solid lines appear at detected change points
6. Hovers over change point line - sees tooltip: "+15.3% at commit abc123"

---

## Audit System Changes

### Warning Message Format

```rust
pub fn audit_with_warning_for_change_points(
    measurements: Vec<f64>,
    commits: Vec<CommitSummary>,
    config: &ChangePointConfig,
) -> AuditResult {
    let change_points = detect_change_points(&measurements, &commits, config)?;

    // Check if any change point exists in current epoch
    let current_epoch = commits.first().map(|c| c.epoch).unwrap_or(0);
    let change_in_epoch = change_points.iter().any(|cp| {
        commits.get(cp.index).map(|c| c.epoch == current_epoch).unwrap_or(false)
    });

    if change_in_epoch {
        eprintln!("⚠️  WARNING: Change point detected within current epoch");
        eprintln!("    Historical baseline may be inconsistent.");
        eprintln!("    Z-score comparison results may be unreliable.");
        eprintln!("    Consider:");
        eprintln!("      - Investigating the detected change point");
        eprintln!("      - Bumping the epoch if the change is intentional");
        eprintln!("      - Using --no-change-point-warning to suppress this warning");
    }

    // Continue with normal audit...
}
```

### Example Output

```
$ git perf audit -m build_time

⚠️  WARNING: Change point detected within current epoch
    Historical baseline may be inconsistent.
    Z-score comparison results may be unreliable.
    Consider:
      - Investigating the detected change point at commit a1b2c3d (+23.5%)
      - Bumping the epoch if the change is intentional
      - Using --no-change-point-warning to suppress this warning

✅ 'build_time'
z-score (stddev): ↑ 0.45
Head: μ: 12.34 ms σ: 0.23 MAD: 0.15 n: 1
Tail: μ: 11.89 ms σ: 1.52 MAD: 1.28 n: 25
 [+3.2% – +4.1%] ▃▃▄▅▆███

Note: Results above may be misleading due to regime shift in historical data.
```

---

## Implementation Priority Order

### Phase 1: MVP - HTML Visualization (Week 1-2)

**Priority Tasks**:
1. ✅ Implement epoch boundary detection from measurement data
2. ✅ Add vertical line traces to PlotlyReporter
3. ✅ Make traces hidden by default (legendonly)
4. ✅ Group traces by measurement name in legend
5. ✅ Implement basic PELT change point detection
6. ✅ Add change point visualization traces
7. ✅ Test with real git repository

**Code Estimate**:
- Epoch detection: ~50 lines
- Plotly traces: ~150 lines
- Change point detection: ~300 lines
- Integration: ~100 lines
- Tests: ~200 lines
- **Total: ~800 lines**

### Phase 2: Audit Warning (Week 3)

**Priority Tasks**:
1. ✅ Add change point detection call in audit
2. ✅ Detect if change point exists in current epoch
3. ✅ Display warning message (non-blocking)
4. ✅ Add CLI flag to suppress warning

**Code Estimate**:
- Warning logic: ~50 lines
- CLI integration: ~20 lines
- Tests: ~50 lines
- **Total: ~120 lines**

### Phase 3: Refinement (Week 4)

**Priority Tasks**:
1. Add hover tooltips with change point details
2. Color coding by confidence level
3. CSV export with epoch/change point columns
4. Documentation and examples

---

## Testing Strategy

### Unit Tests for Epoch Detection

```rust
#[test]
fn test_epoch_transition_detection() {
    let commits = vec![
        Commit {
            commit: "c1".to_string(),
            measurements: vec![MeasurementData {
                epoch: 0,
                name: "test".to_string(),
                ..Default::default()
            }],
        },
        Commit {
            commit: "c2".to_string(),
            measurements: vec![MeasurementData {
                epoch: 0,
                name: "test".to_string(),
                ..Default::default()
            }],
        },
        Commit {
            commit: "c3".to_string(),
            measurements: vec![MeasurementData {
                epoch: 1, // Epoch change!
                name: "test".to_string(),
                ..Default::default()
            }],
        },
    ];

    let transitions = detect_epoch_transitions(&commits, "test", &[]);
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].index, 2);
    assert_eq!(transitions[0].from_epoch, 0);
    assert_eq!(transitions[0].to_epoch, 1);
}
```

### Integration Tests for Visualization

```rust
#[test]
fn test_plotly_report_has_hidden_epoch_traces() {
    let mut reporter = PlotlyReporter::new();
    // ... setup ...

    reporter.add_epoch_boundaries(
        vec![EpochTransition {
            index: 5,
            from_epoch: 0,
            to_epoch: 1,
        }],
        "build_time",
    );

    let html = String::from_utf8_lossy(&reporter.as_bytes());

    // Check that trace is added but hidden
    assert!(html.contains("legendonly"));
    assert!(html.contains("Epoch"));
}
```

---

## Summary of Changes from Original Proposal

| Aspect | Original Proposal | Revised Requirement |
|--------|-------------------|---------------------|
| **Primary Focus** | Audit system integration | HTML report visualization |
| **Audit Behavior** | Display change points as analysis | Warn if change point exists in epoch |
| **Visualization** | Phase 3 enhancement | **Phase 1 priority** |
| **Default Visibility** | Not specified | Hidden by default, legend toggle |
| **Epoch Boundaries** | Not mentioned | **Must visualize in reports** |
| **User Control** | Config/CLI flags | Legend click to show/hide |

---

## Files to Modify

### New Files
- `git_perf/src/change_point.rs` - Change point detection algorithms
- `git_perf/src/epoch_detection.rs` - Epoch transition detection (optional, could be in reporting.rs)

### Modified Files
- `git_perf/src/reporting.rs` - Add epoch/change point visualization traces
- `git_perf/src/audit.rs` - Add warning for change points in current epoch
- `git_perf/src/cli.rs` - Add CLI flags for visualization options
- `git_perf/src/config.rs` - Add configuration for change point detection

---

## Conclusion

The revised requirements shift focus to **visualization first**, with audit integration as a warning mechanism rather than additional analysis. The key insight is that:

1. **Epoch boundaries** are critical for understanding measurement context
2. **Change points** indicate regime shifts that invalidate historical comparisons
3. **Visual inspection** via HTML reports is more valuable than automated blocking
4. **User control** (legend toggle) allows selective visualization

This approach maintains the value of z-score regression detection while adding contextual information about when historical data may not be reliable.

---

**Next Steps**:
1. Review and approve this revised requirements document
2. Begin implementation of Phase 1 (HTML visualization)
3. Focus on Plotly trace integration with legend toggle
4. Add audit warning as Phase 2
