# Change Point Detection Integration Guide for git-perf

## Quick Reference

This document outlines where and how to integrate change point detection analysis into the git-perf codebase. The project is exceptionally well-structured for this enhancement.

## Current Capabilities vs. Proposed Enhancement

### Current (Z-Score Based Audit)
- **Question**: "Has there been a significant change at HEAD?"
- **Method**: Compare HEAD against historical tail using z-score
- **Time Frame**: Instantaneous - one threshold test
- **Data Used**: HEAD + historical aggregate statistics

### Proposed (Change Point Detection)
- **Question**: "Where in the history did changes occur?"
- **Method**: Segment time series, detect breakpoints algorithmically
- **Time Frame**: Historical - analyze entire series
- **Data Used**: Full ordered time series (10-40 points)
- **Benefit**: Identifies WHEN changes happened, not just IF

---

## Integration Architecture

### Option A: Embedded in Audit System (RECOMMENDED)

**Location**: `git_perf/src/audit.rs` lines 250-300

**Integration Point**:
```rust
// In audit_with_data() function, after line 286 (tail_summary calculation)
// Add change point analysis when sufficient data exists

if tail.len() >= 10 {  // Only with abundant historical data
    let change_points = detect_change_points(&tail, dispersion_method)?;
    // Append change point info to audit output
    for cp in change_points {
        eprintln!("  → Change detected at commit {} ({:.1}% shift)", 
                  cp.commit_index, cp.magnitude);
    }
}
```

**Advantages**:
- Reuses existing data pipeline
- Complements z-score analysis
- Single audit run produces both tests
- Leverages configuration system
- No new CLI commands needed (can use flag)

**Implementation Effort**: ~300-500 lines in new `change_point.rs`

---

### Option B: Alternative Analysis Mode

**Location**: New command `git perf analyze-change-points`

**Integration Point**: `git_perf/src/cli.rs` in `Commands` enum

```rust
pub enum Commands {
    // ... existing ...
    
    /// Analyze historical performance for change points
    AnalyzeChangePoints {
        #[arg(short = 'm', long = "measurement", value_parser=parse_spaceless_string, action = clap::ArgAction::Append, required_unless_present = "filter")]
        measurement: Vec<String>,
        
        #[arg(short = 'n', long, default_value = "40")]
        max_count: usize,
        
        #[arg(short, long, value_parser=parse_key_value)]
        selectors: Vec<(String, String)>,
        
        #[arg(short = 'f', long = "filter")]
        filter: Vec<String>,
        
        #[arg(short = 'a', long = "algorithm", default_value = "pelt")]
        algorithm: ChangePointAlgorithm,
    },
}
```

**Handler** in `cli.rs`:
```rust
Commands::AnalyzeChangePoints { measurement, max_count, selectors, filter, algorithm } => {
    change_point::analyze_multiple(
        max_count,
        selectors,
        &combine_measurements_and_filters(&measurement, &filter),
        algorithm,
    )
}
```

**Advantages**:
- Explicit, discoverable feature
- Can be separately documented
- Optional for users who don't need it
- Easier to iterate on algorithms

**Implementation Effort**: ~400-600 lines (includes CLI integration)

---

### Option C: Report Enhancement (COMPLEMENTARY)

**Location**: `git_perf/src/reporting.rs` around line 200

**Integration**: Add change point markers to Plotly graphs

```rust
// In PlotlyReporter::add_summarized_trace()
for (index, measurement) in indexed_measurements {
    if let Some(cp) = change_points.get(&index) {
        // Add vertical line marker at this commit
        let shape = Shape::new()
            .x_ref("x")
            .y_ref("paper")
            .x0(index as i32)
            .x1(index as i32)
            .line(Line::new().color("red").dash(Dash::Dash))
            .annotation(text(&format!("Change: {}%", cp.magnitude)));
    }
}
```

**Advantages**:
- Visual identification of changes
- Interactive hover info
- Works with existing report generation
- Non-invasive enhancement

**Implementation Effort**: ~100-200 lines

---

## Data Flow for Change Point Detection

```
┌──────────────────────────────────────────────────────────┐
│ audit_with_commits() called with measurement_name       │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────┐
│ measurement_retrieval::summarize_measurements()          │
│ - Walk commits HEAD backwards                             │
│ - Filter by measurement name + selectors                 │
│ - Aggregate by reduction function (Min/Max/Median/Mean)  │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────────┐
│ take_while_same_epoch() - Stop at epoch boundary         │
│ Result: Iterator<Result<CommitSummary>>                  │
│ Collect into Vec<f64> for analysis                       │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
        ┌────────────┴────────────┐
        │                         │
        ▼                         ▼
  ┌──────────────┐        ┌──────────────────┐
  │ Z-Score Test │        │ Change Point     │
  │ (existing)   │        │ Detection (new)  │
  └──────────────┘        └──────────────────┘
        │                         │
        ▼                         ▼
  ┌─────────────────────────────────────┐
  │ Format output & return to audit()   │
  └─────────────────────────────────────┘
```

**Key Points**:
- Same data pipeline for both analyses
- Data in correct order for CPD (HEAD backwards)
- Already filtered and aggregated
- Epoch boundaries respected

---

## Implementation Details

### New Module: `change_point.rs`

**Core Structure**:
```rust
#[derive(Debug, Clone)]
pub struct ChangePoint {
    pub index: usize,           // Position in time series (0 = oldest)
    pub magnitude: f64,         // Magnitude of change (%)
    pub commit: String,         // Git SHA of changed commit
    pub confidence: f64,        // Confidence score [0, 1]
}

pub enum ChangePointAlgorithm {
    PELT,              // Binary segmentation (fast)
    KernelCPD,         // Kernel-based (robust)
    BinSeg,            // Divisive (simple)
}

pub fn detect_change_points(
    measurements: &[f64],
    method: DispersionMethod,
) -> Result<Vec<ChangePoint>> {
    // Implementation
}

pub fn analyze_multiple(
    max_count: usize,
    selectors: &[(String, String)],
    combined_patterns: &[String],
    algorithm: ChangePointAlgorithm,
) -> Result<()> {
    // Reuse audit_multiple pattern but call detect_change_points
}
```

**Integration with Existing Code**:
```rust
// In audit.rs after statistical testing
if tail.len() >= MIN_CPD_DATA_POINTS {
    match crate::change_point::detect_change_points(&tail, dispersion_method) {
        Ok(points) => {
            for point in points {
                eprintln!("  Change point detected: {}", point);
            }
        }
        Err(e) => {
            warn!("Change point detection failed: {}", e);
            // Doesn't fail audit, just additional info
        }
    }
}
```

---

## Algorithm Selection

### PELT (Pruned Exact Linear Time)
- **Best for**: General use, balanced speed/accuracy
- **Complexity**: O(n log n)
- **Pros**: Fast, theoretically sound, handles multiple change points
- **Cons**: May miss small changes
- **When to use**: Default algorithm

### Kernel-Based CPD
- **Best for**: Robust to noise and outliers
- **Complexity**: O(n²) or O(n³) depending on kernel
- **Pros**: Handles non-parametric changes, robust
- **Cons**: Slower, requires kernel selection
- **When to use**: Noisy data (use MAD dispersion method)

### Binary Segmentation
- **Best for**: Simple, interpretable
- **Complexity**: O(n²)
- **Pros**: Easy to understand, tunable
- **Cons**: May miss simultaneous changes
- **When to use**: Educational, debugging

**Recommendation**: Start with PELT, leverage existing dispersion method config

---

## Configuration Integration

### Extend `.gitperfconfig`

```toml
[change_point]
enabled = true                     # Enable change point detection
algorithm = "pelt"                 # pelt, kernel, binseg
min_data_points = 10               # Minimum measurements to run
confidence_threshold = 0.8         # Report only >80% confidence

[change_point."build_time"]
enabled = true
algorithm = "kernel"               # Use kernel for this specific measurement
min_magnitude = 5.0                # Only report changes >5%
```

### CLI Integration

```bash
git perf audit -m build_time --detect-changes
git perf audit -m build_time --cpd-algorithm kernel --cpd-confidence 0.9
git perf analyze-change-points -m build_time -n 100  # Full history
```

---

## Data Availability for Implementation

### From `stats.rs`
```rust
// Available statistical functions
pub struct Stats {
    pub mean: f64,
    pub stddev: f64,
    pub mad: f64,
    pub len: usize,
}

// Can compute window statistics
pub fn aggregate_measurements(iter) -> Stats
pub fn calculate_mad(measurements: &[f64]) -> f64
```

### From `measurement_retrieval.rs`
```rust
// Time series access
pub fn walk_commits(num_commits: usize) -> Result<impl Iterator<Item = Result<Commit>>>
pub fn summarize_measurements(...) -> impl Iterator<Item = Result<CommitSummary>>
pub fn take_while_same_epoch(iter) -> impl Iterator
```

### From `config.rs`
```rust
// Configuration access
pub fn audit_dispersion_method(measurement: &str) -> String
pub fn measurement_unit(measurement: &str) -> Option<String>
pub fn get_custom_value(key: &str) -> Option<String>  // For CPD settings
```

---

## Testing Strategy

### Unit Tests (in `change_point.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_change_point_detection() {
        let data = vec![10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0];
        let points = detect_change_points(&data, DispersionMethod::StandardDeviation).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].index, 4);
    }

    #[test]
    fn test_multiple_change_points() {
        let data = vec![5.0, 5.0, 10.0, 10.0, 15.0, 15.0];
        let points = detect_change_points(&data, DispersionMethod::MedianAbsoluteDeviation).unwrap();
        assert!(points.len() >= 2);
    }

    #[test]
    fn test_no_change_points() {
        let data = vec![10.0; 20];  // Constant data
        let points = detect_change_points(&data, DispersionMethod::StandardDeviation).unwrap();
        assert!(points.is_empty());
    }
}
```

### Integration Tests

```bash
# Test with real git data
git perf add build_time 10 && git commit -m "initial"
git perf add build_time 10 && git commit -m "no change"
git perf add build_time 15 && git commit -m "regression"
git perf audit -m build_time --detect-changes
```

### Mutation Testing

Use existing mutation testing framework to verify change point logic:

```rust
#[test]
#[cfg(test)]
fn test_cpd_with_mutation_testing() {
    // Coverage for algorithm correctness
    // Should fail if operators are flipped (>, <, etc.)
}
```

---

## Output Format Examples

### Audit with Change Points

```
✅ 'build_time'
z-score (stddev): ↓ 0.42
Head: μ: 9.87 ms σ: 0.12 MAD: 0.08 n: 1
Tail: μ: 10.23 ms σ: 0.34 MAD: 0.25 n: 20
 [-3.52% – +0.73%] ▄▅▅▆▇█

Analysis: Change points detected in historical data:
  → Commit a1b2c3d (5 commits ago): +15.2% increase
  → Commit x7y8z9w (2 commits ago): -4.1% decrease (within noise)
```

### Standalone Change Point Analysis

```
Analyzing 'test_execution_time' with 40 historical measurements...

Detected 3 change points (PELT algorithm, confidence > 80%):

1. Commit 5c3e8f2 (Jan 12, 2024) - HIGH CONFIDENCE (99%)
   Magnitude: +34.7% regression
   Cause: Suspected infrastructure change or test expansion
   
2. Commit a2f4d9e (Jan 8, 2024) - MEDIUM CONFIDENCE (87%)
   Magnitude: -8.3% improvement
   Cause: Potential optimization
   
3. Commit f1e2d3c (Jan 1, 2024) - LOW CONFIDENCE (72%)
   Magnitude: +2.1% (within noise margin)
   Note: May be statistical artifact
```

### HTML Report with Change Points

- Vertical red dashed lines at change points
- Hover tooltip shows: commit hash, date, magnitude, confidence
- Optional legend showing all detected changes
- Visual gap indicators between segments

---

## Minimal Implementation Checklist

For a working MVP (Minimum Viable Product):

- [ ] Create `git_perf/src/change_point.rs` (~300 lines)
- [ ] Implement `detect_change_points()` with PELT algorithm
- [ ] Create `ChangePoint` struct with basic fields
- [ ] Add result formatting function
- [ ] Integrate into `audit.rs` (10-20 line modification)
- [ ] Add config support (20-30 lines in `config.rs`)
- [ ] Add unit tests (50-100 lines)
- [ ] Test with real git repository
- [ ] Update manpages/docs

**Estimated Effort**: 400-500 lines of new code, 50-100 lines modified

---

## Future Enhancements

1. **Multiple Algorithm Support**
   - Add kernel CPD, binary segmentation
   - User-selectable via config
   - Algorithm benchmarking framework

2. **Advanced Features**
   - Anomaly detection (unusual single points)
   - Trend analysis (regression/improvement slopes)
   - Seasonal pattern detection
   - Auto-correlation analysis

3. **Integration Points**
   - GitHub Actions annotation
   - CI/CD failure triggers
   - Slack/Teams notifications
   - Baseline auto-adjustment

4. **Performance Optimization**
   - Streaming algorithm variants
   - Incremental analysis (only new commits)
   - Caching change point results

5. **Visualization**
   - Multi-color segmentation (different colors per segment)
   - Confidence bands around change points
   - Trend lines with slope indication
   - Anomaly highlighting

---

## References Within Codebase

**Key Files to Study**:
1. `/root/repo/git_perf/src/audit.rs` - Audit system architecture
2. `/root/repo/git_perf/src/stats.rs` - Statistical calculations
3. `/root/repo/git_perf/src/measurement_retrieval.rs` - Data pipeline
4. `/root/repo/git_perf/src/config.rs` - Configuration system
5. `/root/repo/git_perf/src/cli.rs` - Command structure

**Similar Implementation Patterns**:
- `audit_multiple()` - Pattern for handling multiple measurements
- `discover_matching_measurements()` - Measurement filtering
- `resolve_audit_params()` - Configuration resolution
- `aggregate_measurements()` - Statistical aggregation

---

## Approval Checklist

Before implementing, ensure:

- [ ] Module location decided (in audit.rs vs. separate change_point.rs)
- [ ] Algorithm chosen (PELT recommended for MVP)
- [ ] Config integration plan reviewed
- [ ] CLI integration approach agreed
- [ ] Output format finalized
- [ ] Testing strategy approved
- [ ] Documentation plan established

---

## Conclusion

The git-perf codebase is exceptionally well-prepared for change point detection integration:

- **Data Pipeline**: Existing retrieval/filtering/aggregation systems are suitable
- **Statistical Foundation**: Mean, variance, MAD already calculated
- **Configuration System**: Can be extended with CPD parameters
- **Testing Infrastructure**: Unit tests and integration tests established
- **Output Paths**: Audit messages and HTML reports ready
- **Code Quality**: Well-structured, documented, mutation-tested

**Recommended Approach**: Option A (embedded in audit) for initial MVP, then consider Options B & C for enhanced user experience.

