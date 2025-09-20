# Agent Instructions for git-perf Project

This document provides clear instructions for AI agents working on this Rust workspace project.

## Project Overview

This is a Rust workspace with multiple crates (`cli_types`, `git_perf`) that provides performance measurement tools for Git repositories.

## Code Quality & Formatting

### Always Required
- **Run `cargo fmt`** before creating any submissions or commits
- **Run `cargo clippy`** for additional linting and suggestions
- Follow Rust best practices and idiomatic code patterns
- Use proper error handling with `Result` and `Option` types
- Use meaningful variable and function names

### Commands
```bash
# Format code (REQUIRED before submission)
cargo fmt

# Run linting (REQUIRED before submission)
cargo clippy

# Run tests (excluding slow ones)
cargo test -- --skip slow
```

## Testing Policy

- **Primary test command**: `cargo test -- --skip slow` (excludes slow tests)
- Ensure all tests pass before submitting code
- Use `cargo test` for full test suite when needed
- This is the standard command for CI and local pre-submit

## Documentation & Build Process

- If changes are made to the `cli_types` crate, ensure any documentation regenerated as part of the build process is included in the commit
- Run `./scripts/generate-manpages.sh` and commit regenerated documentation
- Commit the regenerated docs alongside the code changes

## Pull Request Standards

- Pull Request titles must follow the Conventional Commits specification:
  - `feat:` - new features
  - `fix:` - bug fixes
  - `docs:` - documentation changes
  - `refactor:` - code refactoring
  - `chore:` - maintenance tasks
  - `test:` - test additions/changes
  - `perf:` - performance improvements
  - `build:` - build system changes
  - `ci:` - CI/CD changes
  - `revert:` - reverts
- Use scope when helpful (e.g., `feat(cli_types): add new command`)

## Pre-Submission Checklist

Before submitting any code, ensure:

1. ✅ Run `cargo fmt` to format code
2. ✅ Run `cargo test -- --skip slow` to verify tests pass
3. ✅ Run `cargo clippy` for additional code quality checks
4. ✅ Ensure all changes compile without warnings
5. ✅ If `cli_types` changed, run `./scripts/generate-manpages.sh` and commit regenerated documentation

## Workspace Structure

- Follow workspace conventions for shared dependencies
- Maintain proper module organization
- This is a multi-crate workspace with `cli_types` and `git_perf` crates

## Environment Setup

**IMPORTANT**: Rust toolchain must be in PATH for formatting to work:

```bash
# Add Rust to PATH (required for background agents)
export PATH="/usr/local/cargo/bin:$PATH"

# Verify tools are available
rustc --version
cargo --version
cargo fmt --version
cargo clippy --version
```

## Why These Rules Matter

The `rustfmt` and `cargo clippy` rules are critical for:
- **Consistency**: All code follows the same formatting standards
- **Quality**: Catches potential bugs and enforces best practices
- **Maintainability**: Clean, readable code that's easy to modify
- **CI/CD**: Automated checks ensure code quality in the pipeline

## Troubleshooting Background Agent Issues

**Common Issue**: Background agents not applying `rustfmt` consistently

**Root Cause**: Rust toolchain not in PATH
- Rust is installed at `/usr/local/cargo/bin/` but not in default PATH
- Background agents may not have access to the full environment

**Solutions**:
1. **For Background Agents**: Ensure `export PATH="/usr/local/cargo/bin:$PATH"` is set
2. **For CI/CD**: Add PATH export to build scripts
3. **For Development**: Add to shell profile (`.bashrc`, `.zshrc`)

**Verification**:
```bash
# Test that formatting works
export PATH="/usr/local/cargo/bin:$PATH"
cargo fmt --check
cargo clippy --version
```

The project includes a `rustfmt.toml` configuration file to ensure consistent formatting across all environments.