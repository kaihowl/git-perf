# Quick Start: Change Point Detection Implementation

**TL;DR**: Add statistical change point detection to git-perf to identify WHEN performance changes occurred, not just IF.

---

## What is Change Point Detection?

**Current (Z-Score)**: "Is HEAD significantly different from history?"
- Answer: YES or NO
- Use: Detect if current commit has a regression

**Proposed (Change Points)**: "Where in history did performance shifts occur?"
- Answer: List of commits with magnitude and confidence
- Use: Root cause analysis, understand performance evolution

**Example Output**:
```
Change Points Detected:
  ↑ Commit a1b2c3d (5 commits ago): +44.7% regression (99% confidence)
  ↓ Commit d4e5f6g (12 commits ago): -8.3% improvement (94% confidence)
```

---

## Recommended Algorithm: PELT

**Why PELT (Pruned Exact Linear Time)?**
- ✅ Fast: O(n) complexity
- ✅ Exact: Mathematically optimal
- ✅ Scalable: Handles 1000+ data points
- ✅ Multiple change points in one pass
- ✅ Well-researched: Killick et al. 2012

**Alternatives**:
- **E-Divisive**: More robust, slower (Netflix uses this)
- **Binary Segmentation**: Simpler, less accurate

---

## Implementation Plan

### Phase 1: MVP (2 weeks)
**Goal**: Working PELT integrated into audit

**Files to Create/Modify**:
- ✨ NEW: `git_perf/src/change_point.rs` (~400 lines)
- 📝 MODIFY: `git_perf/src/audit.rs` (+20 lines)
- 📝 MODIFY: `git_perf/src/config.rs` (+30 lines)
- ✅ TESTS: Unit tests (~100 lines)

**Integration Point**:
```rust
// In audit.rs after z-score test
if tail.len() >= 10 {
    let change_points = detect_change_points(&tail, &commits)?;
    println!("{}", format_change_points(&change_points));
}
```

### Phase 2: Multiple Algorithms (2 weeks)
- Add E-Divisive (~500 lines)
- Add Binary Segmentation (~200 lines)
- User-selectable via config

### Phase 3: Enhanced Features (4 weeks)
- HTML report visualization
- CSV export integration
- Confidence scoring refinement
- CI/CD examples

---

## Configuration

### .gitperfconfig
```toml
[change_point]
enabled = true
algorithm = "pelt"              # pelt, edivisive, binseg
min_data_points = 10
min_magnitude_pct = 5.0         # Only report >5% changes
confidence_threshold = 0.8      # Only report >80% confidence
penalty_multiplier = 3.0        # Higher = fewer change points

# Measurement-specific override
[change_point."build_time"]
penalty_multiplier = 2.0        # More sensitive for build times
```

### CLI
```bash
# Embedded in audit (if enabled in config)
git perf audit -m build_time

# Force enable
git perf audit -m build_time --detect-changes

# Override algorithm
git perf audit -m build_time --cpd-algorithm edivisive

# Standalone analysis (Phase 2)
git perf analyze-change-points -m build_time -n 100
```

---

## PELT Algorithm Pseudocode

```
Input: measurements[n], penalty β
Output: List of change point indices

F[0] = -β
R = {0}  // Active set

for t = 1 to n:
    // Find best previous change point
    min_cost = ∞
    for each τ in R:
        cost = F[τ] + C(measurements[τ:t]) + β
        if cost < min_cost:
            min_cost = cost
            best_τ = τ

    F[t] = min_cost
    cp[t] = best_τ

    // Pruning: remove τ from R if future cost can't improve
    R = {τ ∈ R : F[τ] + C(τ:t) ≤ min_cost}
    R.add(t)

// Backtrack to find change points
return backtrack(cp)
```

**Key Insight**: Pruning step eliminates impossible solutions while maintaining exactness.

---

## Data Structures

```rust
pub struct ChangePoint {
    pub index: usize,          // Position in time series
    pub commit_sha: String,    // Git SHA
    pub magnitude_pct: f64,    // Percentage change
    pub confidence: f64,       // [0.0, 1.0]
    pub direction: ChangeDirection,
}

pub enum ChangeDirection {
    Increase,  // Regression (slower)
    Decrease,  // Improvement (faster)
}

pub enum Algorithm {
    PELT,
    EDivisive,
    BinSeg,
}

pub struct ChangePointConfig {
    pub min_data_points: usize,
    pub algorithm: Algorithm,
    pub min_magnitude_pct: f64,
    pub confidence_threshold: f64,
    pub penalty: f64,
    pub include_improvements: bool,
}
```

---

## Testing Checklist

### Unit Tests
- [ ] Single change point detection
- [ ] Multiple change points
- [ ] No change points (stable data)
- [ ] Noisy data handling
- [ ] Penalty affects sensitivity
- [ ] Insufficient data handling

### Integration Tests
```bash
# Create test data with known change points
git perf add build_time 10  # Baseline
git perf add build_time 15  # +50% regression
git perf add build_time 12  # -20% improvement

# Verify detection
git perf audit -m build_time --detect-changes | grep "2 change points"
```

### Performance Tests
- [ ] PELT runs in < 100ms for n=100
- [ ] Linear scaling verified
- [ ] Memory usage < 10MB

---

## Example Output

### Audit (Embedded)
```
✅ 'build_time'
z-score (stddev): ↑ 2.34
Head: μ: 15.12 ms σ: 0.23 MAD: 0.15 n: 1
Tail: μ: 10.45 ms σ: 0.52 MAD: 0.38 n: 25
 [+39.2% – +51.8%] ▃▃▃▄▄▅▅██

⚠️  Performance regression detected at HEAD

Change Points Detected (PELT, n=25):
  ↑ Commit a1b2c3d (commit 5): +44.7% (99% confidence)
  ↓ Commit d4e5f6g (commit 12): -3.2% (87% confidence)
```

### Standalone Analysis (Phase 2)
```
$ git perf analyze-change-points -m test_time -n 100

Detected 3 change points:

1. Commit 5c3e8f2 (Jan 25) - 87 commits ago
   Magnitude: +34.7% (12.5s → 16.8s)
   Confidence: 99%
   Direction: ↑ Regression

2. Commit a2f4d9e (Jan 18) - 64 commits ago
   Magnitude: -8.3% (16.8s → 15.4s)
   Confidence: 94%
   Direction: ↓ Improvement

Summary: Net change +16.8% with 2 regressions, 2 improvements
```

---

## Key Design Decisions

### 1. Integration Approach
**Decision**: Embed in audit system (Option A)
**Rationale**:
- Reuses existing data pipeline
- Complements z-score analysis
- No new CLI commands needed for MVP
- Natural user experience

### 2. Algorithm Choice
**Decision**: PELT for MVP
**Rationale**:
- Best speed/accuracy trade-off
- Proven scalability
- Simpler than E-Divisive
- Exact (not approximate)

### 3. Default Behavior
**Decision**: Disabled by default initially
**Rationale**:
- Allow gradual adoption
- User can enable via config
- Reduces surprise/confusion
- Can change to enabled after user feedback

### 4. Penalty Calculation
**Decision**: BIC-based with multiplier
**Rationale**:
- Theoretically sound
- User-tunable sensitivity
- Automatic adaptation to data variance

---

## Dependencies

**Recommendation**: Start with ZERO external dependencies

Use existing git-perf infrastructure:
- `stats.rs` - Statistical functions
- `measurement_retrieval.rs` - Data pipeline
- `config.rs` - Configuration
- Standard library only

**Optional (Phase 2+)**:
- `statrs` - Advanced statistics
- `ndarray` - Matrix operations (for E-Divisive)

---

## Success Criteria

### Week 2 (MVP)
- [ ] PELT implementation complete
- [ ] Integrated into audit
- [ ] All tests pass
- [ ] Documentation updated
- [ ] Example working on real repository

### Week 4 (Multiple Algorithms)
- [ ] E-Divisive and BinSeg working
- [ ] User can select algorithm
- [ ] Comparative benchmarks available

### Week 8 (Production Ready)
- [ ] HTML report integration
- [ ] CSV export
- [ ] Performance optimized
- [ ] User guide complete
- [ ] CI/CD examples

---

## Pre-Implementation Checklist

Before starting:
- [ ] Read CHANGE_POINT_DETECTION_PROPOSAL.md (full details)
- [ ] Review existing codebase architecture
- [ ] Study PELT algorithm paper (Killick et al. 2012)
- [ ] Set up development environment
- [ ] Create feature branch

---

## Resources

### Documentation
1. **Full Proposal**: `/root/repo/CHANGE_POINT_DETECTION_PROPOSAL.md`
2. **Codebase Architecture**: `/root/repo/CODEBASE_ARCHITECTURE.md`
3. **Integration Guide**: `/root/repo/CHANGE_POINT_INTEGRATION_GUIDE.md`

### Papers
1. Killick et al. (2012): "Optimal Detection of Changepoints" - PELT algorithm
2. Matteson & James (2014): "Multiple Change Point Analysis" - E-Divisive
3. Netflix Tech Blog: "Fixing Performance Regressions Before They Happen"

### Reference Implementations
1. **Python**: ruptures library (https://github.com/deepcharles/ruptures)
2. **R**: changepoint package
3. **Rust**: fastpelt (https://github.com/ritchie46/fastpelt)

---

## FAQ

**Q: Why not just use z-score?**
A: Z-score answers "is HEAD different?" Change points answer "WHEN did things change?" Both are valuable.

**Q: How is this different from Netflix's approach?**
A: Netflix uses E-Divisive. We start with PELT (faster) and add E-Divisive in Phase 2.

**Q: Will this slow down audits?**
A: No. PELT is O(n) and runs in ~10-50ms for typical datasets.

**Q: How do I tune sensitivity?**
A: Adjust `penalty_multiplier` in config: higher = fewer change points, lower = more sensitive.

**Q: What if I get false positives?**
A: Increase penalty_multiplier or min_magnitude_pct threshold.

**Q: Can I disable it?**
A: Yes, set `enabled = false` in config or use `--no-detect-changes` flag.

---

**Next Steps**: Review full proposal and begin Phase 1 implementation.
