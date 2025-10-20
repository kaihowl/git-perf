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

1. **Parse JUnit XML** - Support `cargo nextest` JUnit XML output (stable, broadly applicable)
2. **Parse criterion JSON** - Support `cargo criterion --message-format json` output
3. **Flexible input** - Read from stdin or files
4. **Measurement conversion** - Transform parsed data to `MeasurementData` with appropriate naming
5. **Metadata enrichment** - Allow custom metadata (CI info, branch, etc.)
6. **Filtering** - Import subset of tests/benchmarks via regex
7. **CI integration** - Add simple benchmark to CI for sample data collection
8. **Broad applicability** - JUnit XML works with pytest, Jest, JUnit, and many other test frameworks

## Non-Goals

- Running tests or benchmarks (user responsibility)
- Supporting unstable/experimental formats (libtest JSON requires nightly + experimental flag)
- Parsing plain text output (too brittle)
- Custom unit parsing for benchmarks (use existing unit system)

## Architecture

```
┌─────────────────────────────────────────────────┐
│  User Runs Tests/Benchmarks                    │
│  $ cargo nextest run --profile ci              │
│  $   (outputs JUnit XML to file)               │
│  $ cargo criterion --message-format json       │
└─────────────────┬───────────────────────────────┘
                  │ (file or pipe to stdin)
                  │
      ┌───────────▼────────────────────┐
      │  git-perf import <format>      │
      │  - Read from stdin or file     │
      │  - Parse format-specific data  │
      └───────────┬────────────────────┘
                  │
      ┌───────────▼───────────┐
      │  Parser Module        │
      │  - JUnit XML          │
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

### JUnit XML Format (De Facto Standard)

**Why JUnit XML:**
- ✅ **Stable** - No experimental flags required
- ✅ **Universal** - Works with pytest, Jest, JUnit, PHPUnit, RSpec, and countless other frameworks
- ✅ **Well-supported** - Supported by all major CI/CD tools (Jenkins, GitHub Actions, CircleCI, etc.)
- ✅ **Simple** - Well-documented XML structure
- ✅ **nextest native** - cargo-nextest has built-in JUnit support via configuration

**Nextest Configuration** (`.config/nextest.toml`):

```toml
[profile.ci.junit]
path = "junit.xml"
```

**Command:**
```bash
cargo nextest run --profile ci
# Outputs to: target/nextest/ci/junit.xml
```

**JUnit XML Structure:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="3" failures="1" errors="0" skipped="0" time="5.2">
  <testsuite name="test_binary_name" tests="3" failures="1" time="5.2">
    <testcase name="test_one" classname="module::tests" time="1.5"/>
    <testcase name="test_two" classname="module::tests" time="2.1">
      <failure message="assertion failed" type="AssertionError"/>
    </testcase>
    <testcase name="test_three" classname="module::tests" time="1.6">
      <skipped/>
    </testcase>
  </testsuite>
</testsuites>
```

**Key Elements:**
- `<testsuites>` - Root element (optional if single suite)
- `<testsuite>` - Test group/binary (attributes: `name`, `tests`, `failures`, `errors`, `skipped`, `time`)
- `<testcase>` - Individual test (attributes: `name`, `classname`, `time`)
- `<failure>` - Test failure (attributes: `message`, `type`)
- `<error>` - Test error (attributes: `message`, `type`)
- `<skipped>` - Skipped test

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

1. **✅ JUnit XML** - For test runtimes
   - **Stable** - No experimental flags required
   - **Universal** - Works with any test framework (Rust, Python, JavaScript, Java, etc.)
   - **Per-test timing** - Provides individual test duration
   - **Well-supported** - All CI/CD tools understand this format
   - Command: `cargo nextest run --profile ci` (configure JUnit output in `.config/nextest.toml`)

2. **✅ cargo-criterion JSON** - For benchmark runtimes
   - Rich statistical data (mean, median, slope, MAD)
   - Machine-readable, well-documented
   - Command: `cargo criterion --message-format json`

## Design

### CLI Interface

```bash
git-perf import <format> [FILE]

# Formats:
#   junit            - JUnit XML format (nextest, pytest, Jest, etc.)
#   criterion-json   - cargo-criterion JSON format

# Examples:
#   git-perf import junit              # Read from stdin
#   git-perf import junit -            # Read from stdin (explicit)
#   git-perf import junit junit.xml    # Read from file
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
    "classname": "module::tests",  // From JUnit XML classname attribute
    "status": "passed" | "failed" | "error" | "skipped",
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
git-perf import junit --metadata ci=true --metadata branch=main
```

## Implementation Plan

### Phase 1: Parser Infrastructure

**New Files:**
```
git_perf/src/parsers/
├── mod.rs              # Module root, public API
├── types.rs            # Shared types (ParsedMeasurement, TestMeasurement, etc.)
├── junit_xml.rs        # JUnit XML parser
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
    pub duration: Option<std::time::Duration>,
    pub status: TestStatus,
    pub metadata: HashMap<String, String>,
}

pub enum TestStatus {
    Passed,
    Failed,
    Error,
    Skipped,
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
- [ ] Implement `JunitXmlParser`
  - Parse XML using a Rust XML library (quick-xml or serde_xml_rs)
  - Extract `<testcase>` elements
  - Read attributes: `name`, `classname`, `time`
  - Determine status from child elements (`<failure>`, `<error>`, `<skipped>`)
  - Handle both single `<testsuite>` and `<testsuites>` root
- [ ] Implement `CriterionJsonParser`
  - Parse line-delimited JSON
  - Filter benchmark-complete messages
  - Extract statistics (mean, median, slope, MAD)
  - Parse benchmark ID (group/name/input)
- [ ] Add comprehensive unit tests with sample XML/JSON data
- [ ] Test error handling (malformed XML/JSON, missing fields)

### Phase 2: Measurement Conversion

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

### Phase 3: CLI Integration

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
    Junit,
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

### Phase 4: CI Benchmark Integration

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

**Step 1:** Add `.config/nextest.toml` to repository:
```toml
[profile.ci.junit]
path = "junit.xml"
```

**Step 2:** Add to `.github/workflows/` (or existing workflow):
```yaml
- name: Run tests and benchmarks, import results
  run: |
    # Run tests with JUnit output
    cargo nextest run --profile ci
    # Outputs to: target/nextest/ci/junit.xml

    # Import test results
    cargo run -- import junit target/nextest/ci/junit.xml \
      --metadata ci=true \
      --metadata workflow="${GITHUB_WORKFLOW}" \
      --metadata commit="${GITHUB_SHA:0:7}"

    # Run simple benchmark with JSON output
    cargo criterion --bench sample_ci_bench --message-format json > bench-results.json

    # Import benchmark results
    cargo run -- import criterion-json bench-results.json \
      --metadata ci=true \
      --metadata workflow="${GITHUB_WORKFLOW}" \
      --metadata commit="${GITHUB_SHA:0:7}"

    # Show what was imported
    cargo run -- audit --measurement "test::*" --num-commits 5
    cargo run -- audit --measurement "bench::*" --num-commits 5
```

**Tasks:**
- [ ] Create `.config/nextest.toml` with JUnit configuration
- [ ] Create `sample_ci_bench.rs` with simple fibonacci benchmark
- [ ] Add benchmark to `Cargo.toml` [[bench]] section
- [ ] Update CI workflow to run tests with JUnit output
- [ ] Update CI workflow to run benchmark
- [ ] Update CI workflow to import both test and benchmark results
- [ ] Verify measurements stored in git notes
- [ ] Document CI integration in README

### Phase 5: Documentation

**New Files:**
- `docs/importing-measurements.md` - Feature guide
- `examples/import-workflow.sh` - Example workflow script

**Documentation Topics:**
1. Overview and motivation
2. Supported formats (junit, criterion-json)
3. Command reference
4. Usage examples
   - Basic import from file (typical workflow)
   - Import from stdin
   - With metadata
   - With filtering
   - Dry run mode
5. Measurement naming conventions
6. Metadata usage
7. Cross-language support (pytest, Jest, JUnit, etc.)
8. CI/CD integration
9. Troubleshooting

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

### Basic Import (Typical Workflow)

```bash
# Run tests with JUnit output (requires .config/nextest.toml configuration)
cargo nextest run --profile ci
# Outputs to: target/nextest/ci/junit.xml

# Import test results
git-perf import junit target/nextest/ci/junit.xml

# Run benchmarks and import results
cargo criterion --message-format json > bench-results.json
git-perf import criterion-json bench-results.json
```

### Import from stdin

```bash
# Pipe JUnit XML to git-perf
cat target/nextest/ci/junit.xml | git-perf import junit

# Explicit stdin
git-perf import junit -
```

### Import with Metadata

```bash
# Add custom metadata for filtering/tracking
git-perf import junit target/nextest/ci/junit.xml \
  --metadata ci=true \
  --metadata branch=main \
  --metadata pr_number=123
```

### Filtering

```bash
# Only import specific tests matching regex
git-perf import junit junit.xml --filter "^test_integration"

# Only import specific benchmark group
git-perf import criterion-json bench-results.json --filter "add_measurements"
```

### Dry Run

```bash
# Preview what would be imported without storing
git-perf import junit junit.xml --dry-run --verbose
```

### Cross-Language Examples

```bash
# Python pytest
pytest --junit-xml=pytest-results.xml
git-perf import junit pytest-results.xml --metadata language=python

# JavaScript Jest
jest --ci --reporters=jest-junit
git-perf import junit junit.xml --metadata language=javascript

# Java JUnit
mvn test  # Outputs to target/surefire-reports/TEST-*.xml
git-perf import junit target/surefire-reports/TEST-MyTest.xml --metadata language=java
```

### CI/CD Integration

```bash
#!/bin/bash
# .github/workflows/performance.yml

set -e

# Run tests with JUnit output
cargo nextest run --profile ci
# Outputs to: target/nextest/ci/junit.xml

# Import test results
git-perf import junit target/nextest/ci/junit.xml \
  --metadata ci=true \
  --metadata workflow="${GITHUB_WORKFLOW}" \
  --metadata run_id="${GITHUB_RUN_ID}" \
  --metadata commit="${GITHUB_SHA:0:7}"

# Run benchmarks
cargo criterion --bench sample_ci_bench --message-format json > bench-results.json

# Import benchmark results
git-perf import criterion-json bench-results.json \
  --metadata ci=true \
  --metadata workflow="${GITHUB_WORKFLOW}"

# Push measurements
git push origin refs/notes/perf-v3

# Audit for regressions
git-perf audit --measurement "test::*" --num-commits 20 --sigma 4.0
git-perf audit --measurement "bench::*" --num-commits 20 --sigma 3.0
```

## Testing Strategy

### Unit Tests

**Parser Tests:**
- Parse valid JUnit XML (single testsuite and testsuites)
- Parse valid criterion JSON
- Handle malformed XML/JSON gracefully
- Handle missing fields/attributes
- Handle empty input
- Test different test statuses (passed, failed, error, skipped)

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
# Generate test data (requires .config/nextest.toml with JUnit config)
cargo nextest run --profile ci
# Outputs to: target/nextest/ci/junit.xml

# Test import
cargo run -- import junit target/nextest/ci/junit.xml --dry-run --verbose

# Verify stored
cargo run -- import junit target/nextest/ci/junit.xml
cargo run -- audit --measurement "test::*" --num-commits 1
```

## Measurement Examples

### Test Measurements

```
Name: test::test_add_note
Value: 0.0123 seconds
Metadata: {
  "type": "test",
  "classname": "git_perf::git_interop",
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
3. **Universal** - JUnit XML works across all major programming languages
4. **Stable** - No experimental flags or nightly compilers required
5. **Simplicity** - No test execution logic, just parsing
6. **Extensibility** - Easy to add new parsers
7. **CI Integration** - Natural fit for CI/CD pipelines (all tools support JUnit)
8. **Unified Analysis** - Use existing audit/report infrastructure
9. **Broad Applicability** - Track test performance from Rust, Python, Java, JavaScript, etc.

## Limitations

1. **Format Dependencies** - Requires specific output formats (JUnit XML, criterion JSON)
2. **File-based for nextest** - JUnit output goes to file, not stdout (minor inconvenience)
3. **No Validation** - Can't verify units were consistent across measurements

## Future Enhancements

1. **Criterion CSV** - Alternative benchmark format
2. **Generic JSON Parser** - User-configurable JSONPath mapping
3. **Additional benchmark formats** - Support other benchmark tools
4. **TAP (Test Anything Protocol)** - Another universal test format
5. **Subunit** - Binary test streaming protocol

## Success Criteria

- [ ] Successfully parse JUnit XML output (nextest, pytest, etc.)
- [ ] Successfully parse criterion JSON output
- [ ] Store test measurements in git notes
- [ ] Store benchmark measurements in git notes
- [ ] Audit test measurements for regressions
- [ ] Audit benchmark measurements for regressions
- [ ] Generate reports with test/benchmark data
- [ ] Support filtering by regex
- [ ] Support stdin and file input
- [ ] Simple CI benchmark runs and imports successfully
- [ ] CI test runs produce JUnit XML and imports successfully
- [ ] Cross-language example works (e.g., pytest)
- [ ] Documentation complete
- [ ] All tests pass (`cargo nextest run -- --skip slow`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Linting clean (`cargo clippy`)
- [ ] Manpages updated

## Timeline

- **Phase 1**: Parser infrastructure
- **Phase 2**: Measurement conversion
- **Phase 3**: CLI integration
- **Phase 4**: CI benchmark integration
- **Phase 5**: Documentation

## References

- **cargo-nextest JUnit support:** https://nexte.st/docs/machine-readable/junit/
- **JUnit XML format documentation:** https://github.com/testmoapp/junitxml
- **Criterion.rs external tools:** https://bheisler.github.io/criterion.rs/book/cargo_criterion/external_tools.html
- **quick-xml (Rust XML parser):** https://github.com/tafia/quick-xml
- **Jenkins JUnit format:** https://llg.cubic.org/docs/junit/
