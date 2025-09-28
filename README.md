# git-perf

A performance measurement tracking tool for Git repositories that stores metrics using git-notes.

## Overview

git-perf provides a comprehensive solution for tracking and analyzing performance metrics in Git repositories. It stores measurements as git-notes, enabling version-controlled performance tracking alongside your codebase.

**🔗 Live Example**: See the [example report for master branch](https://kaihowl.github.io/git-perf/master.html)

## Table of Contents

- [Quick Start](#quick-start)
- [Key Features](#key-features)
- [Audit System](#audit-system)
- [Configuration](#configuration)
- [Migration](#migration)
- [Remote Setup](#remote-setup)
- [Development](#development)
- [Documentation](#documentation)

## Quick Start

```bash
# Add a performance measurement
git perf add build_time 42.5

# Audit for performance regressions
git perf audit -m build_time

# Push measurements to remote
git perf push
```

## Key Features

- **Git-notes Integration**: Store performance data alongside your Git history
- **Statistical Analysis**: Advanced audit system with configurable dispersion methods
- **Regression Detection**: Automated detection of performance changes
- **Centralized Collection**: Designed for centralized metric gathering (e.g., CI/CD)
- **Multiple Formats**: Support for data migration between format versions

## ⚠️ Important Notes

- **Experimental Status**: This tool is experimental and under active development
- **Performance Impact**: Repeated individual measurements are costly; prefer bulk additions when possible
- **Centralized Design**: Unlike Git's decentralized model, git-perf assumes centralized metric collection

## Limitations

Contrary to git itself, git-perf does not support decentralized collection of
performance measurements. Instead, git-perf assumes that there is a single,
central place for the collection of metrics. This should usually be your source
foundry, e.g., GitHub. As performance measurements become less relevant over
time, we allow metrics to be purged. As a delete in git still preserves the
history before that deletion event, we have to rewrite history. To make
rewriting of shared history safe, git-perf deliberately dropped some basic
ideas of decentralized version control and instead focuses on the collection of
metrics in a single central location.

## Migration

git-perf provides helper scripts to migrate existing performance notes between format versions.

### Available Migration Paths

| From | To | Script | Target Ref |
|------|----|---------|-----------|
| v1 | v2 | `./migration/to_v2.sh <path-to-your-repo>` | `refs/notes/perf-v2` |
| v2 | v3 | `./migration/to_v3.sh <path-to-your-repo>` | `refs/notes/perf-v3` |

### Migration Process

The migration scripts:
1. Clone the target repository into a temporary directory
2. Transform the notes to the new format
3. Commit the changes
4. Push to the appropriate notes ref on `origin`

### Post-Migration Steps

After migration, ensure consumers fetch the new notes ref:

```bash
git fetch origin refs/notes/perf-v3:refs/notes/perf-v3
```

## Remote Setup
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


## Audit System

git-perf includes a powerful audit system for detecting performance regressions and improvements. The system uses statistical analysis to identify meaningful performance changes while filtering out noise.

### Statistical Dispersion Methods

Choose between two statistical methods for calculating dispersion:

| Method | Description | Best For |
|--------|-------------|----------|
| **Standard Deviation (stddev)** | Traditional method, sensitive to outliers | Normally distributed data, stable measurements |
| **Median Absolute Deviation (MAD)** | Robust method, less sensitive to outliers | Data with outliers, variable environments |

### When to Use Each Method

**Standard Deviation** is ideal when:
- ✅ Performance data is normally distributed
- ✅ You want to detect all changes, including outlier-caused ones
- ✅ You have consistent, stable measurement environments

**MAD** is recommended when:
- ✅ Performance data has occasional outliers or spikes
- ✅ You want to focus on typical performance changes
- ✅ You're measuring in environments with variable system load
- ✅ You need more robust regression detection

## Configuration

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

### Usage Examples

```bash
# Basic audit (uses configuration or defaults to stddev)
git perf audit -m build_time

# Audit multiple measurements
git perf audit -m build_time -m memory_usage

# Custom deviation threshold
git perf audit -m build_time -d 3.0

# Override dispersion method
git perf audit -m build_time --dispersion-method mad
git perf audit -m build_time -D stddev  # Short form
```

### How It Works

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

## Documentation

### Available Documentation

- **[Manpages](./docs/manpage.md)** - Complete CLI reference
- **[Evaluation Tools](./evaluation/README.md)** - Statistical method comparison tools

### Generating Documentation

Documentation is automatically generated using `clap_mangen` and `clap_markdown`:

```bash
# Generate with default version (0.0.0)
./scripts/generate-manpages.sh

# Generate with custom version
GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh
```

#### Output Locations

- **Manpages**: `target/man/man1/`
- **Markdown**: `docs/manpage.md`

The CI automatically validates that documentation stays current with CLI definitions.

## Development

### Prerequisites

Install required development dependencies:

```bash
# macOS
brew install libfaketime

# Ubuntu/Debian
sudo apt-get install libfaketime
```

### Development Setup

For contributors, run the setup script to install development tools:

```bash
# Install cargo-nextest and other development tools
./scripts/setup-dev-tools.sh
```

This script will install `cargo-nextest` if not already present, enabling faster test execution in your local development environment.

### Testing

This project uses [nextest](https://nexte.st/) for faster, more reliable test execution.

#### Quick Commands

```bash
# Development testing (recommended - skips slow tests)
cargo nextest run -- --skip slow

# Full test suite
cargo nextest run

# Specific test pattern
cargo nextest run --test-pattern "git_interop"

# Verbose output
cargo nextest run --verbose

# Specific package
cargo nextest run -p git-perf
```

#### Alternative: Standard Cargo Tests

```bash
# All tests
cargo test

# Skip slow tests
cargo test -- --skip slow
```

### Code Quality

Before submitting changes, ensure code quality:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy
```
