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
minimum relative deviation thresholds.

## Example Configuration

Create a `.gitperfconfig` file in your repository root:

```toml
# Global threshold for all measurements
[audit.global]
min_relative_deviation = 5.0

# Measurement-specific thresholds (overrides global)
[audit.measurement."build_time"]
min_relative_deviation = 10.0

[audit.measurement."memory_usage"]
min_relative_deviation = 2.0
```

## Usage

```bash
# Audit a specific measurement
git perf audit -m build_time

# Audit multiple measurements
git perf audit -m build_time -m memory_usage

# Use custom sigma threshold
git perf audit -m build_time -d 3.0
```

## How It Works

The audit compares the HEAD measurement against historical measurements:
- **Z-score**: Statistical significance based on standard deviation
- **Relative deviation**: Practical significance as percentage change from median
- **Threshold**: Configurable minimum relative deviation to filter noise
- **Sparkline**: Visualizes measurement range relative to tail median (historical measurements)

When the relative deviation is below the threshold, the audit passes even if
the z-score indicates statistical significance. This helps focus on meaningful
performance changes while ignoring noise.

# Docs

See [manpages](./docs/manpage.md).

## Manpage Generation

The manpages are automatically generated during the build process using `clap_markdown`. To regenerate the documentation:

```bash
# Build the project to generate markdown documentation
cargo build
```

The markdown documentation is automatically generated and written to `docs/manpage.md` during the build process. No additional steps are required.

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
