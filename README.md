# Performance Measurements in Git

Test tracking performance measurements in git-notes.

Example report for [master](https://kaihowl.github.io/git-perf/master.html).

## Warning
Experimental only.
Repeated additions of measurements (instead of bulk additions) will be costly:
Each time the entire previous measurements are copied and a single line is
appended.

# Limitations

Contrary to git itself, git-perf does not support decentralized collection of
performance measurements. Instead, git-perf assumes that there is a single,
central place for the collection of metrics. This should usually be your source
foundry, e.g., GitHub. As performance measurements become less relevant over
time, we allow metrics to be purged. As a delete in git still preserves the
history before that deletion event, we have to rewrite history. To make
rewriting of shared history safe, git-perf deliberately dropped some basic
ideas of decentralized version control and instead focuses on the collection of
metrics in a single central location.

## Migrate measurements
TODO document this

# Setup Different Remote
TODO document this

# Audit Configuration

git-perf includes a powerful audit system that can detect performance regressions
and improvements. The audit can be configured to filter out noise by setting
minimum relative deviation thresholds and choose between different statistical
dispersion methods.

## Statistical Dispersion Methods

git-perf supports two methods for calculating statistical dispersion:

- **Standard Deviation (stddev)**: Traditional method, sensitive to outliers
- **Median Absolute Deviation (MAD)**: Robust method, less sensitive to outliers

### When to Use Each Method

**Use Standard Deviation when:**
- Your performance data is normally distributed
- You want to detect all performance changes, including those caused by outliers
- You have consistent, stable performance measurements

**Use MAD when:**
- Your performance data has occasional outliers or spikes
- You want to focus on typical performance changes rather than extreme values
- You're measuring in environments with variable system load
- You want more robust regression detection

## Example Configuration

Create a `.gitperfconfig` file in your repository root:

```toml
# Global settings for all measurements
[audit.global]
min_relative_deviation = 5.0
dispersion_method = "mad"  # Use MAD for all measurements by default

# Measurement-specific settings (overrides global)
[audit.measurement."build_time"]
min_relative_deviation = 10.0
dispersion_method = "mad"  # Build times can have outliers, use MAD

[audit.measurement."memory_usage"]
min_relative_deviation = 2.0
dispersion_method = "stddev"  # Memory usage is more stable, use stddev

[audit.measurement."test_runtime"]
min_relative_deviation = 7.5
dispersion_method = "mad"  # Test times can vary significantly
```

## Usage

```bash
# Audit a specific measurement (uses config or defaults to stddev)
git perf audit -m build_time

# Audit multiple measurements
git perf audit -m build_time -m memory_usage

# Use custom sigma threshold
git perf audit -m build_time -d 3.0

# Use MAD dispersion method (overrides config)
git perf audit -m build_time --dispersion-method mad

# Use standard deviation method (overrides config)
git perf audit -m build_time --dispersion-method stddev

# Short form of dispersion method option
git perf audit -m build_time -D mad
```

## How It Works

The audit compares the HEAD measurement against historical measurements:
- **Z-score**: Statistical significance based on chosen dispersion method (stddev or MAD)
- **Relative deviation**: Practical significance as percentage change from median
- **Threshold**: Configurable minimum relative deviation to filter noise
- **Sparkline**: Visualizes measurement range relative to tail median (historical measurements)

### Dispersion Method Precedence

The dispersion method is determined in this order:
1. **CLI option** (`--dispersion-method` or `-D`) - highest priority
2. **Measurement-specific config** (`[audit.measurement."name"].dispersion_method`)
3. **Global config** (`[audit.global].dispersion_method`)
4. **Default** (stddev) - lowest priority

### Example Output

```bash
$ git perf audit -m build_time --dispersion-method mad
✅ 'build_time'
z-score (mad): ↓ 2.15
Head: μ: 110.0 σ: 0.0 MAD: 0.0 n: 1
Tail: μ: 101.7 σ: 45.8 MAD: 2.5 n: 3
 [-1.0% – +96.0%] ▁▃▇▁
```

When the relative deviation is below the threshold, the audit passes even if
the z-score indicates statistical significance. This helps focus on meaningful
performance changes while ignoring noise.

# Docs

See [manpages](./docs/manpage.md).

## Evaluation Tools

For evaluating the statistical robustness of different dispersion methods (stddev vs MAD), see the [evaluation tools](./evaluation/README.md).

## Manpage Generation

Both manpages and markdown documentation are automatically generated during the build process using `clap_mangen` and `clap_markdown`. To regenerate the documentation:

```bash
# Build the project to generate both manpages and markdown documentation
cargo build
```

The documentation is automatically generated during the build process:
- Manpages are written to `target/man/man1/` directory
- Markdown documentation is written to `docs/manpage.md`

No additional steps are required. The CI automatically validates that the markdown documentation is up-to-date with the current CLI definition.

# Development

## Development dependencies

- libfaketime

Install with 
```
if [[ $(uname -s) = Darwin ]]; then
    brew install libfaketime
else # ubuntu
    sudo apt-get install libfaketime
fi
```

## Rust tests
```
cargo test
```

Exclude slow integration tests with:
```
cargo test -- --skip slow
```
