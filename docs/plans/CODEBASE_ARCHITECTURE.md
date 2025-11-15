# Git-Perf Codebase Architecture & Change Point Detection Integration

## Executive Summary

Git-perf is a sophisticated performance measurement tracking system that stores metrics as git-notes alongside repository history. It provides statistical analysis, visual reporting (HTML/Plotly), and an audit system for regression detection. The system is well-architected with clear separation between storage, retrieval, analysis, and presentation layers.

## 1. Workspace Structure

### Two-Crate Architecture

```
/root/repo/
├── cli_types/              # Shared CLI type definitions
│   └── src/lib.rs         # Enums and types (ReductionFunc, DispersionMethod)
└── git_perf/              # Main application crate
    ├── src/
    │   ├── lib.rs         # Module exports
    │   ├── main.rs        # CLI entrypoint
    │   ├── cli.rs         # Command definitions (~500 lines)
    │   └── [modules]      # See below
    ├── tests/             # Integration tests
    └── benches/           # Performance benchmarks
```

### Core Module Organization (6,468 lines total)

| Module | Lines | Purpose |
|--------|-------|---------|
| **audit.rs** | 1,501 | Regression detection, statistical testing, z-score analysis |
| **config.rs** | 1,118 | Configuration management, hierarchical settings, epoch tracking |
| **reporting.rs** | 976 | HTML report generation using Plotly, CSV export |
| **stats.rs** | 757 | Statistical aggregation, Mean/Variance, MAD, z-scores |
| **import.rs** | 607 | Parse JUnit XML and Criterion JSON, import external test data |
| **serialization.rs** | 343 | Encode/decode measurements in custom format stored in git-notes |
| **units.rs** | 263 | Time/size unit parsing and formatting |
| **test_helpers.rs** | 164 | Test utilities (feature-gated) |
| **filter.rs** | 162 | Regex filtering for measurement names |
| **data.rs** | ~100 | Core data structures (MeasurementData, Commit, CommitSummary) |
| **measurement_storage.rs** | ~75 | Add measurements to git-notes |
| **measurement_retrieval.rs** | ~93 | Walk commits, deserialize, summarize measurements |
| **git/** | ~400 | Git operations, notes management, remote sync |

---

## 2. Performance Measurement Storage Architecture

### Data Model

```rust
// Core measurement data structure (data.rs)
pub struct MeasurementData {
    pub epoch: u32,              // Reset point for new baseline
    pub name: String,            // Measurement identifier (e.g., "build_time")
    pub timestamp: f64,          // Unix timestamp (seconds)
    pub val: f64,                // Numeric measurement value
    pub key_values: HashMap<String, String>,  // Metadata (os, arch, etc.)
}

// Hierarchy on disk
pub struct Commit {
    pub commit: String,          // Git commit SHA
    pub measurements: Vec<MeasurementData>,  // All measurements for this commit
}
```

### Storage Layer (Git Notes)

**Location**: `refs/notes/perf-v3` in git repository

**Format** (serialization.rs):
```
0measurement_name1234567890.0100.5os=linux
0another_metric1234567891.0200.3arch=x64
```

- Delimiter: null byte (`\0`)
- One line per measurement
- Fields: epoch, name, timestamp, value, key=value pairs

**Storage Flow**:
1. `measurement_storage.rs::add()` - Create MeasurementData with timestamp & config epoch
2. `serialization.rs::serialize_single()` - Encode to string
3. `git_interop.rs::add_note_line_to_head()` - Append to git notes with backoff retry

**Retrieval Flow**:
1. `measurement_retrieval.rs::walk_commits()` - Iterate HEAD backwards N commits
2. `git_interop.rs::walk_commits()` - Execute `git log` and read notes
3. `serialization.rs::deserialize()` - Parse lines back to MeasurementData
4. Result: Iterator over Commits with measurements

### Configuration System (config.rs - 1,118 lines)

**Hierarchy** (highest priority first):
1. CLI flags
2. Measurement-specific config (`.gitperfconfig`)
3. Default config (`.gitperfconfig`)
4. System-wide config (`~/.config/git-perf/config.toml`)
5. Built-in defaults

**Configuration Categories**:
- **Audit settings**: `sigma`, `min_measurements`, `dispersion_method`, `aggregate_by`, `min_relative_deviation`
- **Measurement metadata**: `unit` (e.g., "ns", "ms", "bytes")
- **Epochs**: Version number for each measurement (reset baseline)
- **Git settings**: Remote configuration, backoff timing

**Example Config**:
```toml
[measurement]
min_relative_deviation = 5.0
sigma = 3.5
dispersion_method = "mad"

[measurement."build_time"]
unit = "ms"
sigma = 4.5
min_measurements = 5
```

---

## 3. Current Statistical Analysis Capabilities

### Audit System (audit.rs - 1,501 lines)

The audit system is the **primary statistical feature** and works in these phases:

#### Phase 1: Data Aggregation
- Filter measurements by name/pattern, selector key-values
- Summarize multiple measurements per commit using reduction function:
  - `Min`, `Max`, `Median`, `Mean`
  - Default: Min (most conservative for timing metrics)

#### Phase 2: Statistical Analysis

**Z-Score Calculation**:
```rust
z_score = |head_value - tail_mean| / dispersion

// Where dispersion is either:
- Standard Deviation (stddev): Good for normally distributed data
- Median Absolute Deviation (mad): Robust to outliers
```

**Decision Logic**:
```
if tail_measurements < min_measurements:
  SKIP (insufficient data)
else if z_score > sigma:
  FAIL (potential regression)
else if min_relative_deviation is set:
  if relative_deviation < threshold:
    PASS (below noise threshold)
  else:
    FAIL
else:
  PASS
```

#### Phase 3: Visualization
- Sparkline graph showing all measurements
- Range calculation: min/max as percentages relative to tail median
- Direction arrows: ↑ (increase), ↓ (decrease), → (same)

**Supported Dispersion Methods**:
1. **Standard Deviation** (stddev):
   - Sensitive to outliers
   - Use when: normally distributed data, want to detect ALL changes
   - Default in older versions

2. **Median Absolute Deviation** (mad):
   - Robust to outliers (outlier-resistant)
   - Use when: occasional spikes/noise, want typical changes
   - Recommended default (configurable)

### Aggregation Functions (stats.rs - 757 lines)

```rust
pub enum ReductionFunc {
    Min,     // Conservative: smallest value (best for timing)
    Max,     // Largest value
    Median,  // Middle value (robust)
    Mean,    // Average (sensitive to outliers)
}
```

**VecAggregation Trait**:
- Median calculation with proper handling of even/odd length arrays
- MAD (Median Absolute Deviation) calculation
- Mean and variance using `average` crate

**Statistical Structures**:
```rust
pub struct Stats {
    pub mean: f64,
    pub stddev: f64,
    pub mad: f64,
    pub len: usize,
}

// Z-score methods
impl Stats {
    pub fn z_score(&self, other: &Stats) -> f64;
    pub fn z_score_with_method(&self, other: &Stats, method: DispersionMethod) -> f64;
    pub fn is_significant(&self, other: &Stats, sigma: f64, method: DispersionMethod) -> bool;
}
```

---

## 4. Data Access & Filtering

### Measurement Retrieval Pipeline (measurement_retrieval.rs)

```rust
// Iterator-based pipeline
walk_commits(num_commits)
  ↓ (for each commit result)
summarize_measurements(commits, reduction_func, filter_fn)
  ↓ (per-commit aggregation)
take_while_same_epoch(iter)  // Stop at epoch boundary
  ↓ (results in CommitSummary)
```

### Filtering System (filter.rs - 162 lines)

**Capabilities**:
- Regex pattern matching for measurement names
- OR logic (if any filter matches, include)
- Exact match support with anchored patterns
- Selector key-value matching (AND logic across criteria)

**Usage**:
```rust
// Compile regex patterns
let filters = compile_filters(&["bench_.*", "test_.*"])?;

// Check if measurement matches
matches_any_filter("bench_foo", &filters)  // true
matches_any_filter("other", &filters)      // false

// Convert to exact-match regex
measurements_to_anchored_regex(&["build_time"])  // ^build_time$
```

---

## 5. Data Analysis & Reporting

### Report Generation (reporting.rs - 976 lines)

**Output Formats**:
1. **HTML/Plotly**: Interactive graphs with:
   - Reversed X-axis (newest commits on right)
   - Multiple traces per measurement (grouped by metadata)
   - Automatic unit labeling on Y-axis
   - Responsive design

2. **CSV Export**: Tab-delimited format with columns:
   - commit, epoch, measurement, timestamp, value, unit, [metadata key=value pairs]

**CSV Row Structure**:
```rust
struct CsvMeasurementRow {
    commit: String,
    epoch: u32,
    measurement: String,
    timestamp: f64,
    value: f64,
    unit: String,
    metadata: HashMap<String, String>,  // os=linux, arch=x64
}
```

**Trace Grouping** (Recent feature #461):
- Support for multiple split keys
- Creates combined group labels (e.g., "ubuntu/x64")
- Groups traces by metadata values

### Units & Formatting (units.rs - 263 lines)

**Supported Units**:
- **Time**: ns, us, ms, s
- **Size**: B, KB, MB, GB
- **Count**: #

**Auto-Scaling**:
- Display values in human-readable units
- e.g., 1234567890 ns → "1.23 s"
- Preserves full precision in data

**Unit Integration**:
- Configured per measurement in `.gitperfconfig`
- Used in audit output: "μ: 1.23 ms"
- Applied to graph Y-axis labels

---

## 6. Data Import Capabilities (import.rs - 607 lines)

### Supported Formats

#### 1. **JUnit XML** (Test Results)
- Compatible with:
  - cargo-nextest
  - pytest
  - Jest
  - JUnit-based frameworks
- Parses:
  - Test names → measurement names (prefix "test::")
  - Execution times → values
  - Test properties → metadata

#### 2. **Criterion JSON** (Benchmark Results)
- cargo-criterion format
- Extracts statistics:
  - mean (bench::name::mean)
  - median (bench::name::median)
  - slope (bench::name::slope)
  - mad (bench::name::mad)

**Import Processing**:
1. Parse input (XML or JSON)
2. Apply prefix (optional)
3. Apply metadata key-value pairs (CLI or file)
4. Filter by regex pattern
5. Validate/preview (dry-run)
6. Store as measurements

---

## 7. Command Interface (cli.rs - ~500 lines)

### Available Commands

```
git-perf measure      - Run and time a command (n repetitions)
git-perf add          - Add single measurement with metadata
git-perf import       - Import JUnit XML or Criterion JSON
git-perf push         - Push measurements to remote
git-perf pull         - Pull measurements from remote
git-perf report       - Generate HTML report with graphs
git-perf audit        - Check for regressions (main analysis command)
git-perf bump-epoch   - Accept expected regression, start new baseline
git-perf remove       - Delete old measurements (before date)
git-perf prune        - Remove orphaned measurements
git-perf list-commits - List commits with measurements
git-perf size         - Show storage size of measurements
```

### Key Flags
- `-m, --measurement` - Measurement names (anchor matching)
- `-f, --filter` - Regex patterns (OR logic)
- `-s, --separate-by` - Split graph by metadata key
- `-a, --aggregate-by` - Reduction function (min/max/median/mean)
- `-n, --max-count` - Historical depth (default 40)
- `-d, --sigma` - Z-score threshold
- `-D, --dispersion-method` - stddev or mad
- `-l, --min-measurements` - Minimum historical data points

---

## 8. Architecture Integration Points

### Data Flow Diagram

```
Manual/Import → add() → serialization.rs → git_interop.rs → git notes (refs/notes/perf-v3)
                                                              ↓
walk_commits() ← ← ← ← ← ← ← ← ← ← ← ← git_interop.rs ← ← ←

measurement_retrieval.rs → deserialize → filter → aggregate → audit.rs
                                                               ↓
                                                          stats.rs (z-score)
                                                               ↓
                                                        audit output + reporting.rs → HTML/CSV
```

### Key Integration Boundaries

1. **Storage Layer** (measurement_storage.rs)
   - Input: MeasurementData struct
   - Output: Git notes entries
   - Config dependency: epoch determination

2. **Retrieval Layer** (measurement_retrieval.rs)
   - Input: Commit count, filter function
   - Output: Iterator of CommitSummary (aggregated)
   - Uses: measurement_storage format, filtering

3. **Analysis Layer** (audit.rs, stats.rs)
   - Input: Historical measurements + HEAD value
   - Output: Pass/fail decision + statistics
   - Uses: Config (sigma, dispersion method), filtering

4. **Presentation Layer** (reporting.rs)
   - Input: Commits + measurements + metadata
   - Output: HTML or CSV
   - Uses: Units, config (unit definitions)

### Configuration Dependency Map

```
audit.rs
  ├── config::audit_min_measurements(measurement)
  ├── config::audit_sigma(measurement)
  ├── config::audit_dispersion_method(measurement)
  ├── config::audit_aggregate_by(measurement)
  ├── config::audit_min_relative_deviation(measurement)
  └── config::measurement_unit(measurement)

measurement_storage.rs
  └── config::determine_epoch_from_config(measurement)

reporting.rs
  └── config::measurement_unit(measurement_name)
```

---

## 9. Where Change Point Detection Fits

### Integration Points for Change Point Detection

#### A. **Natural Integration with Audit System**
- **Location**: `audit.rs` after z-score calculation
- **Trigger**: When historical data is abundant (10+ measurements)
- **Purpose**: Supplement z-score with "when did the change happen?"
- **Output**: Additional information to audit report

#### B. **As an Alternative Analysis Mode**
- **New Command**: `git perf analyze-change-points` or flag to audit
- **Workflow**: `audit -m measurement --detect-changes`
- **Benefits**: Complements, doesn't replace z-score testing

#### C. **Preprocessing for Better Baselines**
- **Usage**: Identify where baseline should reset (auto-epoch detection)
- **Integration**: Enhancement to `bump-epoch` command
- **Benefit**: Automatic epoch management

#### D. **Report Enhancement**
- **Location**: HTML reporting layer
- **Feature**: Annotate change points on graphs
- **Visual**: Markers showing where performance changed significantly

### Data Availability for Analysis

**Input**: Iterator of measurements ordered chronologically (newest first)
```rust
// Available in audit_with_commits:
let commits: &[Result<Commit>] = ...;  // Ordered from HEAD backwards
let measurements: Vec<f64> = ...;      // After filtering and aggregation

// Properties:
// - Up to 40 measurements (configurable max_count)
// - Pre-aggregated by reduction function (Min/Max/Median/Mean)
// - Filtered by measurement name and metadata selectors
// - Grouped by epoch (stops at epoch boundary)
```

### Key Advantages of This Integration

1. **Seamless Data Access**
   - Uses existing retrieval pipeline
   - Same filtering/aggregation mechanisms
   - Consistent with current workflow

2. **Hierarchical Analysis**
   - Z-score: Quick "did something change?"
   - Change point detection: "Where and why?"
   - Both use same statistical foundations

3. **Configuration Leverage**
   - Reuse sigma threshold for algorithm parameters
   - Leverage existing unit system
   - Use measurement-specific config overrides

4. **Output Compatibility**
   - Extend audit message format
   - Add sparkline annotations
   - Include in CSV export

---

## 10. Statistical Foundation (Ready for Enhancement)

### Current Statistical Capabilities

**Available in stats.rs**:
- Mean, Variance, Standard Deviation (using `average` crate)
- Median calculation with proper sorting
- Median Absolute Deviation (MAD)
- Z-score calculation with two dispersion methods
- `VecAggregation` trait for reduction functions

**Pattern Analysis Readiness**:
- Iterator-based design allows streaming algorithms
- Time series accessible chronologically
- Multiple dispersion methods already supported
- Extensible `Stats` struct

### For Change Point Detection Implementation

**Available building blocks**:
1. Statistical calculations (mean, variance, MAD)
2. Measurement time series with metadata
3. Epoch-aware data (handles resets)
4. Filtering system for measurement selection
5. Configuration system for algorithm parameters
6. Unit system for result formatting

**What needs to be added**:
1. Change point detection algorithms
2. Trend analysis functions
3. Data preparation (reverse order handling)
4. Result structures (detected changes + metadata)
5. Output formatting for audit/reports

---

## 11. Code Quality & Testing

### Testing Infrastructure

**Test Helpers** (test_helpers.rs - 164 lines):
- Test repository setup
- Measurement storage/retrieval testing
- Git operation mocking

**Test Coverage**:
- Unit tests in each module (filter, stats, data)
- Mutation testing for audit logic (marked with `// MUTATION POINT`)
- Integration tests (bash_tests.rs)

**Benchmarks** (benches/):
- `read.rs` - Measure commit walk performance
- `add.rs` - Measure note addition performance
- `sample_ci_bench.rs` - Full CI workflow

### Code Characteristics

**Strengths**:
- Well-documented module structure
- Clear separation of concerns
- Iterator-based design for efficiency
- Comprehensive error handling with `anyhow`
- Configuration hierarchy with fallbacks

**Patterns Used**:
- Trait-based abstraction (`Reporter`, `VecAggregation`)
- Result<T> for error handling
- Option for optional values
- Iterator combinators
- Closure-based filtering

**Dependencies**:
- `itertools` - Iterator utilities
- `regex` - Pattern matching
- `chrono` - Date/time
- `plotly` - Graph rendering
- `average` - Statistical calculations
- `sparklines` - ASCII sparklines

---

## Summary: Change Point Detection Opportunity

The git-perf codebase is exceptionally well-structured for integrating change point detection:

### Current State
- **Strong**: Statistical analysis, time series data, configuration system
- **Mature**: Audit system, filtering, aggregation
- **Extensible**: Clear module boundaries, trait-based design

### For Change Point Detection
- **Access Point**: `audit.rs` after z-score analysis or as alternative mode
- **Data Available**: Ordered time series with 10-40 measurements per threshold
- **Tools Ready**: Statistical functions, filtering, aggregation
- **Output Path**: Audit messages, HTML reports, CSV exports

### Implementation Strategy
1. Add change point detection module (`change_point.rs` ~300-500 lines)
2. Extend audit system with detection trigger (when n > threshold)
3. Integrate results into audit messages and reports
4. Add CLI flags for algorithm selection and parameters
5. Leverage existing configuration and output systems

### Key Files to Reference
- **Core Logic**: `/root/repo/git_perf/src/audit.rs` (1,501 lines)
- **Statistics**: `/root/repo/git_perf/src/stats.rs` (757 lines)
- **Retrieval**: `/root/repo/git_perf/src/measurement_retrieval.rs` (93 lines)
- **Reporting**: `/root/repo/git_perf/src/reporting.rs` (976 lines)
- **Configuration**: `/root/repo/git_perf/src/config.rs` (1,118 lines)

