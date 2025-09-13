# Manpage Generation Guide

This document explains how manpage generation works in the git-perf project and how to avoid CI failures related to manpage inconsistencies.

## Overview

The git-perf project uses automated manpage generation to keep documentation in sync with the CLI interface. The process involves:

1. **Build-time generation**: Manpages are generated during `cargo build` using `clap_mangen`
2. **CI validation**: GitHub Actions validates that `docs/manpage.md` matches the generated manpages
3. **Local tools**: Scripts and Makefile targets to help developers maintain consistency

## The Problem

The CI system expects `docs/manpage.md` to exactly match what would be generated from the current codebase. However, there are several challenges:

1. **Version differences**: CI temporarily sets version to `0.0.0` to avoid version-based diffs
2. **Formatting differences**: Different pandoc versions or environments may produce slightly different markdown
3. **Manual process complexity**: The manual generation process is error-prone and time-consuming

## The Solution

We've created automated tools that replicate the exact CI process locally:

### Quick Commands

```bash
# Generate manpage.md to match CI expectations
make generate-manpage

# Validate that manpage.md matches CI expectations  
make validate-manpage

# Test both generation and validation
make test-manpage

# Show all available targets
make help
```

### Scripts

- `scripts/generate-manpage.sh` - Generates `docs/manpage.md` using the exact CI process
- `scripts/validate-manpage.sh` - Validates that `docs/manpage.md` matches CI expectations (ignores whitespace, markdown formatting, underscore escaping, and line wrapping differences)
- `scripts/pre-commit-hook.sh` - Optional pre-commit hook to catch issues early

## How It Works

### CI Process

The CI system (`.github/workflows/ci.yml`) does the following:

1. Temporarily sets version to `0.0.0` in `git_perf/Cargo.toml`
2. Runs `cargo build` to generate manpages in `target/man/man1/`
3. Uses pandoc to convert all manpages to markdown format
4. Compares the result with `docs/manpage.md`
5. Restores the original version

### Local Tools

Our local tools replicate this exact process:

1. **Backup original version** from `git_perf/Cargo.toml`
2. **Set version to 0.0.0** temporarily
3. **Build the project** to generate manpages
4. **Convert to markdown** using pandoc with the same command as CI
5. **Restore original version**
6. **Update or validate** `docs/manpage.md`

## Usage Examples

### Before Making Changes

```bash
# Validate current state
make validate-manpage
```

### After Adding New CLI Options

```bash
# Generate updated manpage
make generate-manpage

# Verify it's correct
make validate-manpage

# Commit the changes
git add docs/manpage.md
git commit -m "docs: update manpage for new CLI options"
```

### Troubleshooting CI Failures

If CI fails with manpage errors:

```bash
# Generate the correct manpage
make generate-manpage

# Check what changed
git diff docs/manpage.md

# Commit the fix
git add docs/manpage.md
git commit -m "fix: update manpage to match CI expectations"
```

## Expected Manpage Files

The system expects these manpage files to be generated:

- `git-perf.1` - Main command
- `git-perf-add.1` - Add subcommand
- `git-perf-audit.1` - Audit subcommand
- `git-perf-bump-epoch.1` - Bump epoch subcommand
- `git-perf-measure.1` - Measure subcommand
- `git-perf-prune.1` - Prune subcommand
- `git-perf-pull.1` - Pull subcommand
- `git-perf-push.1` - Push subcommand
- `git-perf-remove.1` - Remove subcommand
- `git-perf-report.1` - Report subcommand

## Dependencies

The manpage generation requires:

- **Rust/Cargo**: For building the project
- **pandoc**: For converting manpages to markdown
  - Ubuntu/Debian: `sudo apt-get install pandoc`
  - macOS: `brew install pandoc`

Install dependencies with:
```bash
make install-deps
```

## Pre-commit Hook (Optional)

You can install the pre-commit hook to automatically validate manpages:

```bash
# Install the hook
cp scripts/pre-commit-hook.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

This will automatically run manpage validation before each commit if `docs/manpage.md` is being modified.

## Troubleshooting

### "pandoc not found"
```bash
# Install pandoc
make install-deps
```

### "cargo not found"
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### "Manpage is out of date"
```bash
# Generate the correct manpage
make generate-manpage
```

### Version differences in diff
This is expected! The CI uses version `0.0.0` while your local version shows the actual version. The tools handle this automatically.

### Whitespace and formatting differences
The validation script ignores whitespace differences (spaces, tabs, line endings), markdown formatting differences (list markers, indentation), underscore escaping differences, and line wrapping differences to focus on content differences. This makes the validation more robust against minor formatting variations between environments, including:
- Differences in how pandoc formats markdown lists
- Underscore escaping variations (`SEPARATE_BY` vs `SEPARATE\_BY`)
- Different pandoc versions with varying escaping behavior
- Line wrapping differences (text wrapped at different points)

## Best Practices

1. **Always use the automated tools** instead of manual generation
2. **Run validation before pushing** to catch issues early
3. **Update manpages when adding CLI options** or changing help text
4. **Use the pre-commit hook** for automatic validation
5. **Check CI logs** if validation fails to understand the exact differences

## Integration with Development Workflow

### Adding New CLI Options

1. Modify the CLI definition in `cli_types/src/lib.rs`
2. Run `make generate-manpage` to update documentation
3. Run `make validate-manpage` to verify consistency
4. Commit both code and documentation changes

### Before Creating PRs

```bash
# Run all checks including manpage validation
make check
```

This runs formatting, clippy, tests, and manpage validation to ensure CI will pass.