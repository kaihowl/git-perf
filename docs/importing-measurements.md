# Importing Test and Benchmark Measurements

This guide explains how to use git-perf's import functionality to track test execution times and benchmark results over time.

## Overview

The `git perf import` command allows you to parse and store runtime measurements from:
- **Test runners** via JUnit XML format (cargo-nextest, pytest, Jest, JUnit, etc.)
- **Benchmark tools** via Criterion JSON format (cargo-criterion)

This enables you to leverage git-perf's statistical audit system to detect performance regressions in your tests and benchmarks.

## Motivation

While `git perf measure` can track arbitrary command execution times, it doesn't provide per-test or per-benchmark granularity. The import feature solves this by:

- **Automatic performance tracking** - Capture individual test/benchmark runtimes without manual instrumentation
- **Regression detection** - Use git-perf's statistical audit to detect performance changes
- **CI integration** - Track performance metrics alongside code changes in your CI pipeline
- **Unified reporting** - Visualize test and benchmark trends using existing report infrastructure

## Supported Formats

### JUnit XML (Tests)

JUnit XML is a de facto standard supported by test frameworks across many languages:

- **Rust**: cargo-nextest
- **Python**: pytest
- **JavaScript**: Jest, Mocha
- **Java**: JUnit, TestNG
- **Go**: go-junit-report
- **And many more...**

**Why JUnit XML:**
- Stable format (no experimental flags required)
- Universal support across languages and CI/CD tools
- Well-documented XML structure
- Provides per-test timing information

### Criterion JSON (Benchmarks)

Criterion is Rust's standard benchmarking library. The JSON output provides rich statistical data including mean, median, slope, and median absolute deviation.

**Command:** `cargo criterion --message-format json`

## Quick Start

### Importing Test Results

```bash
# Configure nextest to output JUnit XML (one-time setup)
# Add to .config/nextest.toml:
# [profile.ci.junit]
# path = "junit.xml"

# Run tests with JUnit output
cargo nextest run --profile ci

# Import test results
git perf import junit target/nextest/ci/junit.xml
```

### Importing Benchmark Results

```bash
# Run benchmarks with JSON output
cargo criterion --message-format json > bench-results.json

# Import benchmark results
git perf import criterion-json bench-results.json
```

## Command Reference

### Basic Syntax

```bash
git perf import <format> [FILE]
```

**Formats:**
- `junit` - JUnit XML format (tests)
- `criterion-json` - Criterion JSON format (benchmarks)

**Input Sources:**
- Omit `[FILE]` or use `-` to read from stdin
- Provide a file path to read from file

### Options

```bash
git perf import <format> [FILE] [OPTIONS]

Options:
  --metadata <KEY>=<VALUE>    Add custom metadata (can be used multiple times)
  --prefix <PREFIX>           Add prefix to measurement names
  --filter <REGEX>            Only import measurements matching regex
  --dry-run                   Preview what would be imported without storing
  --verbose                   Show detailed information about imported measurements
```

## Usage Examples

### Import from File (Typical Workflow)

```bash
# Run tests and import
cargo nextest run --profile ci
git perf import junit target/nextest/ci/junit.xml

# Run benchmarks and import
cargo criterion --message-format json > bench-results.json
git perf import criterion-json bench-results.json
```

### Add Custom Metadata

Metadata helps filter and track measurements in different contexts:

```bash
# Add CI context
git perf import junit junit.xml \
  --metadata ci=true \
  --metadata branch=main \
  --metadata pr_number=123

# Add environment info
git perf import criterion-json bench.json \
  --metadata os=ubuntu \
  --metadata rust_version=1.75.0 \
  --metadata cpu=intel_i7
```

### Filter Imports

Only import specific tests or benchmarks:

```bash
# Only import integration tests
git perf import junit junit.xml --filter "^integration::"

# Only import specific benchmark group
git perf import criterion-json bench.json --filter "add_measurements"
```

### Preview Before Importing (Dry Run)

```bash
# See what would be imported without storing
git perf import junit junit.xml --dry-run --verbose

# Example output:
# Would import 3 measurements:
#   test::git_perf::tests::test_add_note (0.0123s)
#   test::git_perf::tests::test_audit (0.0456s)
#   test::git_perf::tests::test_measure (0.0789s)
```

## Measurement Naming Conventions

### Test Measurements

Tests are stored with the prefix `test::` followed by the test name:

```
test::<test_name>
```

**Examples:**
- `test::my_module::tests::test_basic`
- `test::integration::api::test_endpoint`
- `test::unit::parser::test_parse_xml`

### Benchmark Measurements

Benchmarks create separate measurements for each statistic with the prefix `bench::`:

```
bench::<benchmark_id>::<statistic>
```

**Statistics tracked:**
- `mean` - Average runtime
- `median` - Median runtime
- `slope` - Linear regression slope (for parameterized benchmarks)

**Examples:**
- `bench::fibonacci_10::mean`
- `bench::fibonacci_20::median`
- `bench::add_measurements/add_measurement/50::mean`

## Metadata Schema

### Test Metadata

```json
{
  "type": "test",
  "classname": "module::tests",
  "status": "passed"
}
```

**Fields:**
- `type`: Always "test"
- `classname`: Test module/class from JUnit XML
- `status`: "passed", "failed", "error", or "skipped"

### Benchmark Metadata

```json
{
  "type": "bench",
  "group": "fibonacci",
  "bench_name": "fibonacci_10",
  "input": "10",
  "statistic": "mean"
}
```

**Fields:**
- `type`: Always "bench"
- `group`: Benchmark group name
- `bench_name`: Specific benchmark name
- `input`: Input parameter (if applicable)
- `statistic`: "mean", "median", or "slope"

### Custom Metadata

Add your own metadata using `--metadata` flag:

```bash
git perf import junit junit.xml \
  --metadata ci=true \
  --metadata workflow="Test Suite" \
  --metadata commit=$(git rev-parse --short HEAD)
```

## Cross-Language Examples

JUnit XML works with test frameworks from many languages:

### Python (pytest)

```bash
# Run tests with JUnit output
pytest --junit-xml=pytest-results.xml

# Import results
git perf import junit pytest-results.xml --metadata language=python
```

### JavaScript (Jest)

```bash
# Configure Jest to output JUnit (in package.json or jest.config.js)
# "reporters": ["default", "jest-junit"]

# Run tests
npm test

# Import results
git perf import junit junit.xml --metadata language=javascript
```

### Java (JUnit)

```bash
# Run tests (Maven)
mvn test
# Outputs to: target/surefire-reports/TEST-*.xml

# Import results
git perf import junit target/surefire-reports/TEST-MyTest.xml \
  --metadata language=java
```

### Go

```bash
# Run tests with go-junit-report
go test -v 2>&1 | go-junit-report > report.xml

# Import results
git perf import junit report.xml --metadata language=go
```

## Nextest Configuration

To enable JUnit XML output with cargo-nextest, add this configuration:

**File:** `.config/nextest.toml`

```toml
[profile.ci.junit]
path = "junit.xml"
```

This tells nextest to write JUnit XML output to `target/nextest/ci/junit.xml` when you run:

```bash
cargo nextest run --profile ci
```

## Querying and Analyzing Imported Data

Once you've imported measurements, use git-perf's existing commands:

### View Recent Measurements

```bash
# List all measurements
git perf audit --num-commits 5

# List specific benchmark
git perf audit --measurement "bench::fibonacci_10::mean" --num-commits 10
```

### Check for Regressions

```bash
# Audit tests for performance changes (4-sigma threshold)
git perf audit --sigma 4.0

# Audit specific benchmark for performance changes (3-sigma threshold)
git perf audit --measurement "bench::fibonacci_10::mean" --sigma 3.0
```

### Generate Reports

```bash
# Generate report for specific test
git perf report --measurement "test::my_module::test_slow_operation"

# Generate report for specific benchmark
git perf report --measurement "bench::fibonacci_10::mean"
```

## Best Practices

### 1. Use Consistent Metadata

Add metadata consistently across runs:

```bash
git perf import junit junit.xml \
  --metadata ci=true \
  --metadata os="$(uname -s)" \
  --metadata rust="$(rustc --version | cut -d' ' -f2)"
```

### 2. Higher Sigma Thresholds in CI

CI environments are variable. Use higher thresholds:

- **Local development:** 2-3 sigma
- **CI environments:** 5-10 sigma

### 3. Filter Noisy Tests

Some tests are inherently variable. Filter them:

```bash
# Only import stable integration tests
git perf import junit junit.xml --filter "^integration::"
```

### 4. Track Over Time

Import measurements regularly to build historical data:

```bash
# In CI, on every commit to main
git perf import junit junit.xml
git perf push
```

### 5. Use Appropriate Sigma Thresholds

Use different thresholds based on measurement variability:

```bash
# Tests: higher threshold (more variable)
git perf audit --sigma 5.0

# Benchmarks: lower threshold (more stable)
git perf audit --sigma 3.0
```

## Limitations

1. **Format Dependencies** - Requires specific output formats (JUnit XML, Criterion JSON)
2. **Nextest File Output** - JUnit XML goes to file, not stdout (minor inconvenience)
3. **No Unit Validation** - Can't verify units were consistent across measurements
4. **Timing Precision** - Limited by test framework's timing precision

## Summary

The import feature enables you to:

- Track test execution times from any JUnit-compatible test framework
- Monitor benchmark performance using Criterion
- Detect performance regressions automatically
- Use a consistent workflow across multiple programming languages

**Next Steps:**
1. Configure nextest or your test framework to output JUnit XML
2. Import your first measurements
3. Set up CI integration
4. Start auditing for regressions

## See Also

**Related Documentation:**
- **[Integration Tutorial](./INTEGRATION_TUTORIAL.md)** - Set up GitHub Actions to automatically import measurements in CI
- **[Configuration Guide](../README.md#configuration)** - Configure audit thresholds for imported measurements
- **[Audit System](../README.md#audit-system)** - Understanding how regression detection works with imported data
- **[CLI Reference](./manpage.md)** - Complete import command documentation
- **[FAQ](../README.md#frequently-asked-questions)** - Common questions about importing and auditing

**GitHub Actions:**
- **[Report Action](../.github/actions/report/README.md)** - Generate reports from imported measurements
- **[Cleanup Action](../.github/actions/cleanup/README.md)** - Manage retention of imported measurements
