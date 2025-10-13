# Contributing to git-perf

Thank you for considering contributing to git-perf! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Quality Standards](#code-quality-standards)
- [Testing Requirements](#testing-requirements)
- [Pull Request Process](#pull-request-process)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Documentation](#documentation)

## Code of Conduct

We expect all contributors to be respectful and professional. Please maintain a welcoming and inclusive environment for everyone.

## Getting Started

### Prerequisites

- Rust toolchain (stable channel recommended)
- Git
- cargo-nextest for running tests

### Installing Development Tools

```bash
# Install cargo-nextest (required for running tests)
cargo install cargo-nextest --locked

# Verify installation
cargo nextest --version
```

## Development Setup

1. **Fork and clone the repository**:
   ```bash
   git clone https://github.com/your-username/git-perf.git
   cd git-perf
   ```

2. **Verify your environment**:
   ```bash
   # Ensure Rust toolchain is available
   rustc --version
   cargo --version

   # Verify formatting tools
   cargo fmt --version
   cargo clippy --version
   ```

3. **Build the project**:
   ```bash
   cargo build
   ```

4. **Run tests**:
   ```bash
   cargo nextest run -- --skip slow
   ```

## Code Quality Standards

All code contributions must meet the following standards:

### Required Checks Before Submission

1. **Format your code** (MANDATORY):
   ```bash
   cargo fmt
   ```

2. **Run linting** (MANDATORY):
   ```bash
   cargo clippy
   ```

3. **Run tests** (MANDATORY):
   ```bash
   cargo nextest run -- --skip slow
   ```

4. **Ensure no warnings**:
   ```bash
   cargo build --workspace
   ```

### Rust Best Practices

- Follow idiomatic Rust code patterns
- Use proper error handling with `Result` and `Option` types
- Use meaningful variable and function names
- Add documentation comments for public APIs
- Avoid unsafe code unless absolutely necessary and well-justified

## Testing Requirements

### Running Tests

The project uses `cargo-nextest` for testing:

```bash
# Run standard tests (excludes slow tests)
cargo nextest run -- --skip slow

# Run full test suite including slow tests
cargo nextest run

# Run tests for a specific package
cargo nextest run -p git_perf
```

### Writing Tests

- Add tests for all new functionality
- Ensure edge cases are covered
- Use descriptive test names that explain what is being tested
- Keep tests focused and isolated

## Pull Request Process

### Before Creating a Pull Request

Run the complete pre-submission checklist:

```bash
# Format code
cargo fmt

# Run tests
cargo nextest run -- --skip slow

# Run linting
cargo clippy

# If you modified CLI types, regenerate documentation
./scripts/generate-manpages.sh
```

### Creating a Pull Request

**CRITICAL**: Both commit messages AND pull request titles MUST follow [Conventional Commits](https://www.conventionalcommits.org/).

1. **Create a feature branch**:
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes and commit** (see commit guidelines below)

3. **Push your branch**:
   ```bash
   git push -u origin feat/your-feature-name
   ```

4. **Create the pull request with a proper title**:
   ```bash
   gh pr create --title "feat(scope): description of change" --body "Detailed description..."
   ```

### Pull Request Checklist

- [ ] Code is formatted with `cargo fmt`
- [ ] All tests pass with `cargo nextest run -- --skip slow`
- [ ] Code passes `cargo clippy` without warnings
- [ ] Commit messages follow Conventional Commits format
- [ ] PR title follows Conventional Commits format
- [ ] Documentation is updated if needed
- [ ] Manpages regenerated if CLI was modified

## Commit Message Guidelines

### Conventional Commits Format

All commit messages MUST follow the [Conventional Commits specification](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Commit Types

| Type | Description | Examples |
|------|-------------|----------|
| `feat` | New features | `feat(cli): add audit command` |
| `fix` | Bug fixes | `fix(config): handle missing file gracefully` |
| `docs` | Documentation only | `docs: update README installation steps` |
| `refactor` | Code refactoring | `refactor(parser): simplify error handling` |
| `test` | Adding or updating tests | `test: add integration tests for audit` |
| `chore` | Maintenance tasks | `chore(deps): update clap to 4.5.0` |
| `perf` | Performance improvements | `perf: optimize measurement parsing` |
| `build` | Build system changes | `build: update cargo config` |
| `ci` | CI/CD changes | `ci: add release workflow` |
| `revert` | Reverts previous commits | `revert: undo performance optimization` |

### Scopes (Optional but Recommended)

Common scopes in this project:
- `cli_types` - CLI types crate
- `git_perf` - Main git-perf crate
- `config` - Configuration system
- `audit` - Audit functionality
- `docs` - Documentation
- `test` - Tests

### Good Commit Message Examples

✅ **Good**:
```
feat(cli_types): add new measurement command
fix(audit): handle empty measurement data
docs: improve installation instructions
test(integration): add git interop tests
chore(deps): update clap to 4.5.0
```

❌ **Bad**:
```
Add new feature                    # Missing type prefix
Update README                      # Missing type prefix
fix stuff                         # Too vague
feat: Add new measurement command  # Inconsistent capitalization
```

## Documentation

### When to Update Documentation

Update documentation when:
- Adding new features or commands
- Changing existing functionality
- Modifying configuration options
- Adding new examples or use cases

### Documentation Types

1. **Code Comments**: Document complex logic and public APIs
2. **README**: Keep user-facing documentation current
3. **Manpages**: Automatically regenerated from CLI definitions
4. **Integration Guides**: Update if setup process changes

### Regenerating Manpages

If you modify the CLI (commands, arguments, descriptions):

```bash
# Regenerate manpages and markdown documentation
./scripts/generate-manpages.sh

# Commit the regenerated documentation
git add docs/manpages/ docs/cli/
git commit -m "docs: regenerate manpages for CLI changes"
```

## Project Structure

This is a Rust workspace with multiple crates:

```
git-perf/
├── cli_types/          # CLI type definitions and argument parsing
├── git_perf/           # Core functionality and main binary
├── docs/               # Documentation
├── scripts/            # Build and maintenance scripts
└── .github/            # GitHub Actions workflows and actions
```

## Getting Help

- Check existing issues for similar questions or problems
- Review the [README](README.md) and documentation
- Look at the [integration tutorial](docs/INTEGRATION_TUTORIAL.md)
- Open a new issue with your question

## Issue Reporting

When reporting issues, please include:
- git-perf version (`git perf --version`)
- Operating system and version
- Rust version (`rustc --version`)
- Steps to reproduce the issue
- Expected vs actual behavior
- Relevant error messages or logs

## Recognition

Contributors will be recognized in:
- Git commit history
- Release notes and changelogs
- Project README (for significant contributions)

Thank you for contributing to git-perf!
