# Git-Perf Codebase Exploration - Executive Summary

**Date**: November 12, 2024  
**Project**: git-perf - Performance Measurement Tracking with Git Notes  
**Objective**: Understand codebase architecture and identify change point detection integration opportunities

---

## Key Findings

### 1. Exceptional Codebase Quality
- Well-organized module structure (6,500+ lines core code)
- Clear separation of concerns (storage, retrieval, analysis, reporting)
- Mature configuration system with hierarchical overrides
- Comprehensive statistical capabilities (mean, stddev, MAD, z-scores)
- Established testing infrastructure with mutation testing

### 2. Perfect Foundation for Change Point Detection
- Time series data already accessible and properly formatted
- Statistical aggregation pipeline ready to use
- Iterator-based design allows composable enhancements
- Configuration system can be extended for algorithm parameters
- Output infrastructure (audit messages, HTML reports, CSV) ready

### 3. Current Capabilities

**Storage Layer**:
- Git-notes based (`refs/notes/perf-v3`)
- Custom serialization format (epoch|name|timestamp|value|metadata)
- Backoff-retry logic for concurrent safety

**Analysis Layer** (Primary Feature):
- Z-score based regression detection
- Two dispersion methods: Standard Deviation & Median Absolute Deviation (MAD)
- Four aggregation functions: Min, Max, Median, Mean
- Configurable thresholds and minimum data requirements

**Reporting Layer**:
- HTML/Plotly interactive graphs
- CSV export with full metadata
- Sparkline visualization
- Unit auto-scaling and formatting

**Data Access**:
- Walk commits (HEAD backwards)
- Regex-based measurement filtering
- Key-value selector matching
- Epoch boundary handling

---

## Codebase Architecture at a Glance

```
┌─────────────────────────────────────────────┐
│  CLI Layer (cli.rs, ~500 lines)            │
│  Commands: measure, add, import, audit,     │
│  report, push, pull, bump-epoch, etc.      │
└──────────────────────┬──────────────────────┘
                       │
      ┌────────────────┼────────────────┐
      │                │                │
┌─────▼──────┐  ┌──────▼────────┐  ┌───▼──────────┐
│ Storage    │  │ Import/Parse  │  │ Config       │
│ (75L)      │  │ (607L)        │  │ (1,118L)     │
└─────┬──────┘  └────────────────┘  └──────────────┘
      │
      ▼
┌─────────────────────────────────────────────┐
│  Serialization (343L)                       │
│  Format: epoch|name|timestamp|value|key=val │
└─────────────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────────────┐
│  Git Interop (~400L)                        │
│  git notes append, backoff retry, refs ops  │
└─────────────────────────────────────────────┘
      │
      ▼
   GIT DB (refs/notes/perf-v3)

RETRIEVAL PIPELINE:
git notes ──► walk_commits() ──► summarize_measurements() ──► take_while_same_epoch()
                                         │
                                         ▼
                            ┌───────────────────────┐
                            │ AUDIT SYSTEM (1,501L) │ ◄── Main analysis
                            │ • Data aggregation    │
                            │ • Z-score testing     │
                            │ • Output formatting   │
                            └───────────────────────┘

ANALYSIS FOUNDATION (stats.rs, 757L):
├─ Stats struct (mean, stddev, mad, len)
├─ Z-score calculation with 2 dispersion methods
├─ Median/MAD computation
└─ VecAggregation trait for reduction functions

OUTPUT LAYER:
├─ reporting.rs (976L) - Plotly HTML + CSV export
├─ units.rs (263L) - Unit formatting & auto-scaling
└─ filter.rs (162L) - Regex-based measurement selection
```

---

## Integration Points for Change Point Detection

### Recommended: Option A - Embedded in Audit
**Location**: `git_perf/src/audit.rs` after z-score calculation (line ~400)

**How It Works**:
1. Existing audit flow retrieves and aggregates measurements
2. Z-score test runs (current functionality)
3. If historical data > 10 points, run change point detection
4. Output includes both z-score result AND detected change points

**Implementation**:
- New module: `change_point.rs` (~300-500 lines)
- Minimal modifications to `audit.rs` (10-20 lines)
- Config extension for algorithm parameters

**Advantages**:
- Seamless integration with existing pipeline
- No new CLI commands needed
- Complements rather than replaces z-score analysis
- Leverages configuration system
- Natural user experience

### Alternative: Option B - Standalone Analysis Command
**New Command**: `git perf analyze-change-points`

- More explicit, discoverable feature
- Can be separately documented
- Easier to iterate on algorithms
- Optional for users who don't need it

### Complementary: Option C - Report Enhancement
**Location**: `git_perf/src/reporting.rs`

- Add visual markers on Plotly graphs
- Show change points with confidence indicators
- Add to CSV export metadata
- Non-invasive enhancement

---

## Data Availability for Change Point Detection

### Time Series Input
```rust
// What's available:
Vec<f64> measurements = [10.0, 10.2, 10.1, 10.3, 15.2, 15.1, 15.0, ...]
                         └─── Period 1 ──┘ └──── Period 2 ────────┘
                         (stable)         (post-change)
```

**Properties**:
- 10-40 measurements per analysis (configurable)
- Chronologically ordered (HEAD backwards)
- Pre-aggregated by reduction function (Min/Max/Median/Mean)
- Filtered by measurement name + metadata selectors
- Respects epoch boundaries

### Available Statistical Functions
From `stats.rs`:
- Mean, variance, standard deviation
- Median Absolute Deviation (MAD)
- Z-score calculation (already used for audit)
- VecAggregation trait for reductions

### Configuration Leverage
- Reuse sigma threshold for algorithm parameters
- Measurement-specific config overrides
- Unit system for result formatting
- Built-in defaults with CLI override

---

## Implementation Roadmap

### Phase 1: MVP (Minimal Viable Product)
**Effort**: ~400-500 lines new code + 50-100 lines modifications

Checklist:
- [ ] Create `change_point.rs` with PELT algorithm (300 lines)
- [ ] Implement `ChangePoint` struct (50 lines)
- [ ] Integrate into `audit.rs` (20 lines)
- [ ] Add config support (30 lines)
- [ ] Write unit tests (100 lines)
- [ ] Test with real git repository
- [ ] Update documentation

### Phase 2: Enhancement
- Support multiple algorithms (Kernel CPD, Binary Segmentation)
- Add confidence scoring
- Trend analysis
- Automatic epoch detection

### Phase 3: Advanced Features
- Anomaly detection
- Seasonal pattern detection
- GitHub Actions integration
- Slack/Teams notifications

---

## Key Module Sizes & Responsibilities

| Module | Lines | Key Responsibility |
|--------|-------|-------------------|
| audit.rs | 1,501 | Z-score regression detection, main analysis engine |
| config.rs | 1,118 | Hierarchical configuration with measurement overrides |
| reporting.rs | 976 | HTML/Plotly graphs and CSV export |
| stats.rs | 757 | Statistical calculations (mean, stddev, MAD, z-scores) |
| import.rs | 607 | Parse JUnit XML and Criterion JSON |
| serialization.rs | 343 | Encode/decode git notes format |
| units.rs | 263 | Unit handling and auto-scaling |
| filter.rs | 162 | Regex-based measurement filtering |
| measurement_retrieval.rs | 93 | Iterator-based data pipeline |
| measurement_storage.rs | 75 | Add measurements to git notes |
| git/ | ~400 | Git operations and notes management |

---

## Critical Files to Reference

For implementation:
1. `/root/repo/git_perf/src/audit.rs` - Where to integrate change point detection
2. `/root/repo/git_perf/src/stats.rs` - Statistical foundations
3. `/root/repo/git_perf/src/measurement_retrieval.rs` - Data pipeline
4. `/root/repo/git_perf/src/config.rs` - Configuration system
5. `/root/repo/git_perf/src/cli.rs` - Command structure

For patterns:
- `audit_multiple()` - Multiple measurement handling
- `discover_matching_measurements()` - Filtering logic
- `resolve_audit_params()` - Config resolution pattern
- `aggregate_measurements()` - Statistical aggregation

---

## Supporting Documentation Created

Three comprehensive documents have been created to guide implementation:

1. **CODEBASE_ARCHITECTURE.md** (19 KB)
   - Detailed architecture breakdown
   - Data flow diagrams
   - Module dependencies
   - Current capabilities analysis

2. **ARCHITECTURE_FLOW.txt** (20 KB)
   - ASCII diagrams and visual flows
   - Audit system logic breakdown
   - Configuration hierarchy
   - Integration opportunities

3. **CHANGE_POINT_INTEGRATION_GUIDE.md**
   - Implementation options (A, B, C)
   - Algorithm selection guidance
   - Configuration strategy
   - Testing approach
   - Output format examples
   - MVP checklist

---

## Why This Codebase is Ideal for Change Point Detection

1. **Mature Analysis Layer**
   - Z-score infrastructure already in place
   - Statistical calculations well-tested
   - Dispersion methods (stddev/MAD) established

2. **Robust Data Pipeline**
   - Reliable measurement retrieval
   - Proper filtering and aggregation
   - Epoch awareness and boundary handling

3. **Extensible Architecture**
   - Trait-based abstractions
   - Clear module boundaries
   - Configuration system ready to extend

4. **Well-Tested Codebase**
   - Unit tests in every module
   - Mutation testing for audit logic
   - Integration tests available

5. **Ready Output Paths**
   - Audit message formatting
   - CSV export structure
   - HTML report generation
   - Sparkline support

---

## Next Steps for Implementation

1. **Decide Integration Approach**
   - Option A (embedded) recommended for MVP
   - Review with team

2. **Choose Algorithm**
   - PELT recommended (good speed/accuracy balance)
   - Kernel CPD for robustness (Phase 2)
   - Binary Segmentation for simplicity (educational)

3. **Plan Configuration**
   - Extend `.gitperfconfig` with CPD settings
   - Measurement-specific overrides
   - CLI flag integration

4. **Design Output Format**
   - Commit hash, magnitude, confidence
   - Integration with sparkline visualization
   - CSV export columns

5. **Implement & Test**
   - Create `change_point.rs` module
   - Unit tests with synthetic data
   - Integration tests with real git repo
   - Mutation testing for correctness

6. **Document & Release**
   - User-facing documentation
   - Manpage generation
   - Example configurations
   - Tutorial/guide

---

## Conclusion

The git-perf codebase is exceptionally well-prepared for change point detection implementation. The existing:
- Statistical infrastructure
- Data pipeline
- Configuration system
- Output paths
- Testing framework

...make it an ideal foundation for adding temporal analysis capabilities. Change point detection would complement the existing z-score testing, providing users with "WHERE" answers to go with the "IF" answers they currently get.

**Recommended Action**: Proceed with Option A (embedded audit enhancement) for MVP, planning for 400-500 lines of new code with minimal modifications to existing systems.

---

## Documents Reference

All exploration documents are located in `/root/repo/`:

```
CODEBASE_ARCHITECTURE.md          - Detailed architecture guide
ARCHITECTURE_FLOW.txt              - Visual diagrams and flows
CHANGE_POINT_INTEGRATION_GUIDE.md  - Implementation roadmap
EXPLORATION_SUMMARY.md             - This document
```

