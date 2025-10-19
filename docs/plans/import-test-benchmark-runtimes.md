# Plan: Import Test and Benchmark Runtime Measurements

**Status:** Planned
**Created:** 2025-10-19
**Related:** New feature to support data import from test runners and benchmarks

## Overview

Implement an import system to parse and store runtime measurements from test runners (cargo-nextest) and benchmark tools (cargo-criterion). This allows git-perf to track test and benchmark performance over time using the existing measurement infrastructure.

## Motivation

Currently, git-perf can measure arbitrary command execution times via `git-perf measure`, but users cannot easily track:
- Per-test execution times from their test suites
- Benchmark results from criterion benchmarks
- Historical performance trends of tests and benchmarks

This feature enables:
- **Automatic performance tracking** - Capture test/bench runtimes without manual instrumentation
- **Regression detection** - Use git-perf's statistical audit to detect performance regressions
- **CI integration** - Track performance metrics alongside code changes
- **Unified reporting** - Visualize test and benchmark trends using existing report infrastructure

**Design Philosophy:** git-perf doesn't run tests/benchmarks - it only **parses their output**. This follows the Unix philosophy of composability and keeps git-perf focused on measurement storage and analysis.

## Goals

1. **Parse nextest JSON** - Support `cargo nextest --message-format libtest-json` output
2. **Parse criterion JSON** - Support `cargo criterion --message-format json` output
3. **Flexible input** - Read from stdin or files
4. **Measurement conversion** - Transform parsed data to `MeasurementData` with appropriate naming
5. **Metadata enrichment** - Allow custom metadata (CI info, branch, etc.)
6. **Filtering** - Import subset of tests/benchmarks via regex
7. **CI integration** - Add simple benchmark to CI for sample data collection

## Non-Goals

- Running tests or benchmarks (user responsibility)
- Supporting unstable libtest JSON format (requires nightly Rust)
- Parsing plain text output (too brittle)
- JUnit XML support (future enhancement)
- Custom unit parsing for benchmarks (use existing unit system)

## Architecture

```
┌─────────────────────────────────────────────────┐
│  User Runs Tests/Benchmarks                    │
│  $ cargo nextest run --message-format json     │
│  $ cargo criterion --message-format json       │
└─────────────────┬───────────────────────────────┘
                  │ (stdout or save to file)
                  │
      ┌───────────▼────────────────────┐
      │  git-perf import <format>      │
      │  - Read from stdin or file     │
      │  - Parse format-specific data  │
      └───────────┬────────────────────┘
                  │
      ┌───────────▼───────────┐
      │  Parser Module        │
      │  - nextest JSON       │
      │  - criterion JSON     │
      └───────────┬───────────┘
                  │
      ┌───────────▼────────────────────┐
      │  Converter Module              │
      │  - Test/bench → Measurement    │
      │  - Add metadata                │
      └───────────┬────────────────────┘
                  │
      ┌───────────▼───────────┐
      │  measurement_storage  │
      │  ::add_multiple()     │
      └───────────┬───────────┘
                  │
      ┌───────────▼───────────┐
      │  Git Notes Storage    │
      └───────────────────────┘
```

## Research Summary

### Cargo Nextest Output Formats

**Available Formats:**

| Format | Command | Status | Recommendation |
|--------|---------|--------|----------------|
| `libtest-json` | `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --message-format libtest-json` | ⚠️ Experimental | **✅ Recommended** |
| `libtest-json-plus` | Same with extra nextest field | ⚠️ Experimental | Alternative |
| JUnit XML | `cargo nextest run --junit output.xml` | ✅ Stable | Future option |

**libtest-json Format** (Line-delimited JSON):

```json
{ "type": "suite", "event": "started", "test_count": 3 }
{ "type": "test", "event": "started", "name": "tests::test_one" }
{ "type": "test", "name": "tests::test_one", "event": "ok", "exec_time": 0.001234 }
{ "type": "test", "event": "started", "name": "tests::test_two" }
{ "type": "test", "name": "tests::test_two", "event": "ok", "exec_time": 0.015678 }
{ "type": "suite", "event": "ok", "passed": 2, "failed": 1, "exec_time": 0.025432 }
```

**Key Fields:**
- `type`: "suite" | "test"
- `event`: "started" | "ok" | "failed" | "ignored"
- `name`: Full test path (e.g., "module::test_name")
- `exec_time`: Duration in seconds (only on completion events)

### Cargo Criterion Output Formats

**cargo-criterion JSON Format** (Line-delimited JSON):

```json
{
  "reason": "benchmark-complete",
  "id": "add_measurements/add_measurement/50",
  "unit": "ns",
  "mean": { "estimate": 15456.78, "lower_bound": 15234.0, "upper_bound": 15678.5 },
  "median": { "estimate": 15400.0, "lower_bound": 15350.0, "upper_bound": 15450.0 },
  "slope": { "estimate": 15420.5, "lower_bound": 15380.0, "upper_bound": 15460.0 },
  "median_abs_dev": { "estimate": 123.45 }
}
```

**Key Fields:**
- `reason`: "benchmark-complete" | "group-complete"
- `id`: Benchmark identifier (group/name/input format)
- `unit`: "ns" | "us" | "ms" | "s"
- `mean`, `median`, `slope`: Statistical estimates with confidence intervals

### Recommended Formats

1. **✅ nextest libtest-json** - For test runtimes
   - Provides per-test timing
   - Already experimental in nextest, will stabilize
   - Command: `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --message-format libtest-json`

2. **✅ cargo-criterion JSON** - For benchmark runtimes
   - Rich statistical data
   - Machine-readable, well-documented
   - Command: `cargo criterion --message-format json`

## Design

### CLI Interface

```bash
git-perf import <format> [FILE]

# Formats:
#   nextest-json     - nextest libtest-json format
#   criterion-json   - cargo-criterion JSON format

# Examples:
#   git-perf import nextest-json              # Read from stdin
#   git-perf import nextest-json -            # Read from stdin (explicit)
#   git-perf import nextest-json results.json # Read from file
```

### Measurement Naming Convention

**Test Measurements:**
```
test::<test_name>
test::tests::test_one
test::module::submodule::test_name
```

**Benchmark Measurements:**
```
bench::<benchmark_id>::<statistic>
bench::add_measurements/add_measurement/50::mean
bench::add_measurements/add_measurement/50::median
bench::add_measurements/add_measurement/50::slope
```

### Metadata Schema

**For Tests:**
```rust
{
    "type": "test",
    "suite": "tests",           // Extracted from test name first component
    "status": "passed" | "failed" | "ignored",
}
```

**For Benchmarks:**
```rust
{
    "type": "bench",
    "group": "add_measurements",
    "bench_name": "add_measurement",
    "input": "50",              // Optional
    "statistic": "mean" | "median" | "slope",
}
```

Users can add custom metadata via `--metadata` flag:
```bash
git-perf import nextest-json --metadata ci=true --metadata branch=main
```

## Implementation Plan

### Phase 1: Parser Infrastructure (2-3 days)

**New Files:**
```
git_perf/src/parsers/
├── mod.rs              # Module root, public API
├── types.rs            # Shared types (ParsedMeasurement, TestMeasurement, etc.)
├── nextest_json.rs     # Nextest libtest-json parser
└── criterion_json.rs   # Criterion JSON parser
```

**Core Types:**
```rust
pub enum ParsedMeasurement {
    Test(TestMeasurement),
    Benchmark(BenchmarkMeasurement),
}

pub struct TestMeasurement {
    pub name: String,
    pub duration_secs: Option<f64>,
    pub status: TestStatus,
    pub metadata: HashMap<String, String>,
}

pub enum TestStatus {
    Passed,
    Failed,
    Ignored,
}

pub struct BenchmarkMeasurement {
    pub id: String,
    pub statistics: BenchStatistics,
    pub metadata: HashMap<String, String>,
}

pub struct BenchStatistics {
    pub mean_ns: Option<f64>,
    pub median_ns: Option<f64>,
    pub slope_ns: Option<f64>,
    pub mad_ns: Option<f64>,
    pub unit: String,
}

pub trait Parser {
    fn parse(&self, input: &str) -> anyhow::Result<Vec<ParsedMeasurement>>;
}
```

**Tasks:**
- [ ] Create `parsers/` module structure
- [ ] Implement `NextestJsonParser`
  - Parse line-delimited JSON
  - Extract test events (ok, failed, ignored)
  - Capture exec_time field
  - Extract suite from test name
- [ ] Implement `CriterionJsonParser`
  - Parse line-delimited JSON
  - Filter benchmark-complete messages
  - Extract statistics (mean, median, slope, MAD)
  - Parse benchmark ID (group/name/input)
- [ ] Add comprehensive unit tests with sample JSON data
- [ ] Test error handling (malformed JSON, missing fields)

### Phase 2: Measurement Conversion (1-2 days)

**New Files:**
```
git_perf/src/converters/
└── mod.rs              # Conversion logic
```

**Core Functions:**
```rust
pub struct ConversionOptions {
    pub prefix: Option<String>,
    pub extra_metadata: HashMap<String, String>,
    pub epoch: u32,
    pub timestamp: f64,
}

pub fn convert_to_measurements(
    parsed: Vec<ParsedMeasurement>,
    options: &ConversionOptions,
) -> Vec<MeasurementData>
```

**Conversion Logic:**
- Test: `test::<name>` with metadata (type, suite, status)
- Benchmark: `bench::<id>::<stat>` with metadata (type, group, statistic)
- Apply user-provided prefix if specified
- Merge user-provided metadata
- Convert units: nanoseconds → seconds for consistency

**Tasks:**
- [ ] Create `converters/` module
- [ ] Implement test conversion
  - Format measurement name
  - Populate metadata hashmap
  - Handle missing duration (use 0.0)
- [ ] Implement benchmark conversion
  - Create separate measurement for each statistic
  - Convert nanoseconds to seconds
  - Handle input parameters in ID
- [ ] Add unit tests for conversion logic
- [ ] Test edge cases (missing fields, zero values)

### Phase 3: CLI Integration (1-2 days)

**Modified Files:**
- `cli_types/src/lib.rs` - Add `ImportCommand`
- `git_perf/src/cli.rs` - Route import command
- `git_perf/src/commands/import.rs` - Command handler (new)
- `git_perf/src/lib.rs` - Export new modules

**CLI Command:**
```rust
pub struct ImportCommand {
    pub format: ImportFormat,
    pub file: Option<String>,       // None or "-" = stdin
    pub prefix: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub filter: Option<String>,     // Regex filter
    pub dry_run: bool,
    pub verbose: bool,
}

pub enum ImportFormat {
    NextestJson,
    CriterionJson,
}
```

**Command Handler:**
1. Read input (stdin or file)
2. Select parser based on format
3. Parse input → Vec<ParsedMeasurement>
4. Apply regex filter if specified
5. Convert to MeasurementData
6. Display if verbose/dry-run
7. Store via `measurement_storage::add_multiple()`

**File Parameter Behavior:**
- `None` → Read from stdin
- `Some("-")` → Read from stdin (explicit)
- `Some(path)` → Read from file

**Tasks:**
- [ ] Add `ImportCommand` to `cli_types`
- [ ] Implement `handle_import()` function
- [ ] Add stdin/file reading logic
- [ ] Integrate parsers and converters
- [ ] Add filtering support (regex)
- [ ] Add dry-run and verbose modes
- [ ] Wire up command router in `cli.rs`
- [ ] Update `lib.rs` to export modules
- [ ] Integration tests

### Phase 4: CI Benchmark Integration (1 day)

**Goal:** Add a simple benchmark to CI that generates sample data for testing the import feature.

**New Files:**
```
git_perf/benches/
└── sample_ci_bench.rs  # Simple benchmark for CI testing
```

**Simple Benchmark:**
```rust
use criterion::{criterion_group, criterion_main, Criterion, black_box};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_fibonacci(c: &mut Criterion) {
    c.bench_function("fibonacci_10", |b| b.iter(|| fibonacci(black_box(10))));
    c.bench_function("fibonacci_20", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, bench_fibonacci);
criterion_main!(benches);
```

**CI Workflow Changes:**

Add to `.github/workflows/` (or existing workflow):
```yaml
- name: Run benchmarks and import results
  run: |
    # Run simple benchmark with JSON output
    cargo criterion --bench sample_ci_bench --message-format json > bench-results.json

    # Import benchmark results (after implementing import command)
    cargo run -- import criterion-json bench-results.json \
      --metadata ci=true \
      --metadata workflow="${GITHUB_WORKFLOW}" \
      --metadata commit="${GITHUB_SHA:0:7}"

    # Show what was imported
    cargo run -- audit --measurement-filter "bench::" --num-commits 5
```

**Tasks:**
- [ ] Create `sample_ci_bench.rs` with simple fibonacci benchmark
- [ ] Add benchmark to `Cargo.toml` [[bench]] section
- [ ] Update CI workflow to run benchmark
- [ ] Update CI workflow to import results
- [ ] Verify measurements stored in git notes
- [ ] Document CI integration in README

### Phase 5: Documentation (1 day)

**New Files:**
- `docs/importing-measurements.md` - Feature guide
- `examples/import-workflow.sh` - Example workflow script

**Documentation Topics:**
1. Overview and motivation
2. Supported formats (nextest-json, criterion-json)
3. Command reference
4. Usage examples
   - Basic import from stdin
   - Import from file
   - With metadata
   - With filtering
   - Dry run mode
5. Measurement naming conventions
6. Metadata usage
7. CI/CD integration
8. Troubleshooting

**Modified Files:**
- `README.md` - Add import examples to main README
- `CLAUDE.md` - Update agent instructions if needed

**Tasks:**
- [ ] Write comprehensive feature documentation
- [ ] Create example workflow scripts
- [ ] Add quick examples to README
- [ ] Document measurement naming scheme
- [ ] Document metadata best practices
- [ ] Add CI integration examples
- [ ] Run `./scripts/generate-manpages.sh` to update manpages
- [ ] Add troubleshooting section

## Usage Examples

### Basic Import

```bash
# Run nextest and import results
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --message-format libtest-json | \
  git-perf import nextest-json

# Run benchmarks and import results
cargo criterion --message-format json | \
  git-perf import criterion-json
```

### Import from Files

```bash
# Save results to file
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --message-format libtest-json > test-results.json

# Import later
git-perf import nextest-json test-results.json

# With metadata
git-perf import nextest-json test-results.json \
  --metadata ci=true \
  --metadata branch=main
```

### Filtering

```bash
# Only import specific tests
cargo nextest run --message-format libtest-json | \
  git-perf import nextest-json --filter "^tests::unit::"

# Only import specific benchmark group
cargo criterion --message-format json | \
  git-perf import criterion-json --filter "add_measurements"
```

### Dry Run

```bash
# Preview what would be imported
git-perf import nextest-json test-results.json --dry-run --verbose
```

### CI/CD Integration

```bash
#!/bin/bash
# .github/workflows/performance.yml

set -e

# Run tests
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --message-format libtest-json > test-results.json

# Import test results
git-perf import nextest-json test-results.json \
  --metadata ci=true \
  --metadata workflow="${GITHUB_WORKFLOW}" \
  --metadata run_id="${GITHUB_RUN_ID}"

# Run benchmarks
cargo criterion --bench sample_ci_bench --message-format json > bench-results.json

# Import benchmark results
git-perf import criterion-json bench-results.json \
  --metadata ci=true

# Push measurements
git push origin refs/notes/perf-v3

# Audit for regressions
git-perf audit --measurement-filter "test::" --num-commits 20 --sigma 4.0
git-perf audit --measurement-filter "bench::" --num-commits 20 --sigma 3.0
```

## Testing Strategy

### Unit Tests

**Parser Tests:**
- Parse valid nextest JSON
- Parse valid criterion JSON
- Handle malformed JSON gracefully
- Handle missing fields
- Handle empty input

**Converter Tests:**
- Convert test measurements correctly
- Convert benchmark measurements correctly
- Apply prefix correctly
- Merge metadata correctly
- Handle edge cases (missing duration, zero values)

**Integration Tests:**
- Full workflow: parse → convert → store
- File reading
- Stdin reading
- Filtering
- Dry-run mode

### Manual Testing

```bash
# Generate test data
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --message-format libtest-json > /tmp/test-data.json

# Test import
cargo run -- import nextest-json /tmp/test-data.json --dry-run --verbose

# Verify stored
cargo run -- audit --measurement-filter "test::" --num-commits 1
```

## Measurement Examples

### Test Measurements

```
Name: test::git_interop::test_add_note
Value: 0.0123 seconds
Metadata: {
  "type": "test",
  "suite": "git_interop",
  "status": "passed"
}
```

### Benchmark Measurements

```
Name: bench::add_measurements/add_measurement/50::mean
Value: 0.000015456 seconds (15456 ns)
Metadata: {
  "type": "bench",
  "group": "add_measurements",
  "bench_name": "add_measurement",
  "input": "50",
  "statistic": "mean"
}

Name: bench::add_measurements/add_measurement/50::median
Value: 0.0000154 seconds (15400 ns)
Metadata: {
  "type": "bench",
  "group": "add_measurements",
  "bench_name": "add_measurement",
  "input": "50",
  "statistic": "median"
}
```

## Benefits

1. **Composability** - Unix philosophy: git-perf parses, doesn't run
2. **Flexibility** - User controls test execution, git-perf stores results
3. **Tool Agnostic** - Works with any tool that outputs supported formats
4. **Simplicity** - No test execution logic, just parsing
5. **Extensibility** - Easy to add new parsers
6. **CI Integration** - Natural fit for CI/CD pipelines
7. **Unified Analysis** - Use existing audit/report infrastructure

## Limitations

1. **Format Dependencies** - Requires specific output formats
2. **Experimental Status** - nextest libtest-json is experimental
3. **No Validation** - Can't verify units were consistent
4. **Display Only** - Units for display, not audit calculations

## Future Enhancements

1. **JUnit XML Support** - Cross-language test import
2. **Criterion CSV** - Alternative benchmark format
3. **libtest JSON** - If/when stabilized in Rust
4. **Generic JSON Parser** - User-configurable JSONPath mapping
5. **Additional Formats** - pytest, Jest, other test frameworks

## Success Criteria

- [ ] Successfully parse nextest libtest-json output
- [ ] Successfully parse criterion JSON output
- [ ] Store test measurements in git notes
- [ ] Store benchmark measurements in git notes
- [ ] Audit test measurements for regressions
- [ ] Audit benchmark measurements for regressions
- [ ] Generate reports with test/benchmark data
- [ ] Support filtering by regex
- [ ] Support stdin and file input
- [ ] Simple CI benchmark runs and imports successfully
- [ ] Documentation complete
- [ ] All tests pass (`cargo nextest run -- --skip slow`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Linting clean (`cargo clippy`)
- [ ] Manpages updated

## Timeline

- **Phase 1**: Parser infrastructure (2-3 days)
- **Phase 2**: Measurement conversion (1-2 days)
- **Phase 3**: CLI integration (1-2 days)
- **Phase 4**: CI benchmark integration (1 day)
- **Phase 5**: Documentation (1 day)

**Total Estimated Effort:** 6-9 days

## References

- **cargo-nextest docs:** https://nexte.st/docs/machine-readable/
- **Criterion.rs external tools:** https://bheisler.github.io/criterion.rs/book/cargo_criterion/external_tools.html
- **libtest JSON RFC:** https://rust-lang.github.io/rfcs/3558-libtest-json.html
- **Rust libtest tracking issue:** https://github.com/rust-lang/rust/issues/49359
- **nextest libtest-json tracking issue:** https://github.com/nextest-rs/nextest/issues/1152
