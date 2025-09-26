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
There are helper scripts to migrate existing performance notes between formats:

- v1 → v2: `./migration/to_v2.sh <path-to-your-repo>`
- v2 → v3: `./migration/to_v3.sh <path-to-your-repo>`

Both scripts clone the target repo into a temporary directory, transform the
notes, commit the change, and push to the appropriate notes ref on `origin`:

- v2 is pushed to `refs/notes/perf-v2`
- v3 is pushed to `refs/notes/perf-v3`

After migration, make sure your consumers fetch the new notes ref, e.g.:

```bash
git fetch origin refs/notes/perf-v3:refs/notes/perf-v3
```

# Setup Different Remote
`git-perf push`/`pull` automatically use a special remote called `git-perf-origin`.
If this remote doesn't exist, git-perf will automatically create it using your
`origin` remote's URL.

To use a different remote for performance measurements:

```bash
# Option 1: Set the git-perf-origin remote to a different URL
git remote set-url git-perf-origin git@github.com:org/repo.git

# Option 2: Add a new remote and set git-perf-origin to use it
git remote add perf-upstream git@github.com:org/repo.git
git remote set-url git-perf-origin git@github.com:org/repo.git

# Now git-perf push/pull will use the new remote
git perf push
git perf pull
```


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
# Default settings for all measurements (parent table)
[measurement]
min_relative_deviation = 5.0
dispersion_method = "mad"  # Use MAD for all measurements by default
epoch = "00000000"  # Default epoch for performance changes

# Measurement-specific settings (override defaults)
[measurement."build_time"]
min_relative_deviation = 10.0
dispersion_method = "mad"  # Build times can have outliers, use MAD
epoch = "12345678"

[measurement."memory_usage"]
min_relative_deviation = 2.0
dispersion_method = "stddev"  # Memory usage is more stable, use stddev
epoch = "abcdef12"

[measurement."test_runtime"]
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
2. **Measurement-specific config** (`[measurement."name"].dispersion_method`)
3. **Default config** (`[measurement].dispersion_method`)
4. **Built-in default** (stddev) - lowest priority

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
# Generate manpages and markdown documentation with normalized version (defaults to 0.0.0)
./scripts/generate-manpages.sh

# Or with a custom version
GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh
```

The script uses the `GIT_PERF_VERSION` environment variable to set a normalized version for documentation generation, avoiding version-based diffs. If not specified, it defaults to `0.0.0`.

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

This project uses [nextest](https://nexte.st/) for faster, more reliable test execution.

### Running Tests with Nextest

**Basic test run:**
```bash
cargo nextest run
```

**Skip slow tests (recommended for development):**
```bash
cargo nextest run --skip slow
```

**Run specific test patterns:**
```bash
cargo nextest run --test-pattern "git_interop"
```

**Run tests with verbose output:**
```bash
cargo nextest run --verbose
```

**Run tests in a specific package:**
```bash
cargo nextest run -p git-perf
```

### Legacy Cargo Test Commands

If you prefer to use the standard cargo test runner:
```bash
cargo test
```

Exclude slow integration tests:
```bash
cargo test -- --skip slow
```
