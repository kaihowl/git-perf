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

The manpages are automatically generated during the build process using `clap_mangen`. The CI system validates that `docs/manpage.md` matches the generated manpages.

### Quick Commands

```bash
# Generate manpage.md to match CI expectations
make generate-manpage
# or
./scripts/generate-manpage.sh

# Validate that manpage.md matches CI expectations
make validate-manpage
# or
./scripts/validate-manpage.sh

# Test both generation and validation
make test-manpage
```

### Manual Process

If you need to regenerate manually (not recommended for CI consistency):

```bash
# Build the project to generate manpages
cargo build

# Convert main manpage to markdown
pandoc -f man -t gfm target/man/man1/git-perf.1 > docs/manpage.md

# Or convert the main and all subcommand manpages to markdown
for file in target/man/man1/git-perf.1 target/man/man1/git-perf-add.1 target/man/man1/git-perf-audit.1 target/man/man1/git-perf-bump-epoch.1 target/man/man1/git-perf-measure.1 target/man/man1/git-perf-prune.1 target/man/man1/git-perf-pull.1 target/man/man1/git-perf-push.1 target/man/man1/git-perf-remove.1 target/man/man1/git-perf-report.1; do
    echo "$(basename "$file" .1)";
    echo "================";
    pandoc -f man -t gfm "$file";
    echo -e "\n\n";
done > docs/manpage.md
```

**Note**: The automated scripts ensure consistency with CI by temporarily setting the version to `0.0.0` during generation, which matches the CI behavior.

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
