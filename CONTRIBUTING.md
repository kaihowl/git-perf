# Contributing to git-perf

Thank you for your interest in contributing to git-perf! We welcome contributions from everyone. Whether it's a bug report, new feature, documentation improvement, or a simple typo fix - all contributions are valued and appreciated.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Quality Standards](#code-quality-standards)
- [Testing Requirements](#testing-requirements)
- [Submitting Changes](#submitting-changes)
- [Commit Message Guidelines](#commit-message-guidelines)
- [Documentation](#documentation)
- [Project Goals](#project-goals)
- [Getting Help](#getting-help)

## Code of Conduct

We are committed to providing a welcoming and inclusive environment for everyone. We expect all contributors to:
- Be respectful and professional in all interactions
- Provide constructive feedback
- Focus on what is best for the community
- Show empathy towards other community members

> **Note**: The first impression you give to a new contributor never fades. Let's make every interaction positive and encouraging.

## Ways to Contribute

No contribution is too small! Here are ways you can help:

### Non-Code Contributions
- **Report bugs**: Found an issue? Let us know!
- **Suggest features**: Have an idea? Share it with us
- **Improve documentation**: Fix typos, clarify instructions, add examples
- **Answer questions**: Help others in issues and discussions
- **Triage issues**: Help categorize and reproduce reported issues

### Code Contributions
- **Fix bugs**: Pick an issue and submit a fix
- **Add features**: Implement new functionality
- **Improve performance**: Optimize existing code
- **Write tests**: Increase test coverage
- **Refactor code**: Improve code quality and maintainability

## Getting Started

### Best Way to Start

The best way to get started is by asking for help! We're here to support you.

1. **Browse existing issues** to find something interesting
2. **Check the documentation**:
   - [README](README.md) - Project overview and quick start
   - [Integration Tutorial](docs/INTEGRATION_TUTORIAL.md) - End-to-end setup guide
   - [CLAUDE.md](CLAUDE.md) - Detailed development guidelines
3. **Join the conversation** by commenting on issues
4. **Start small** - even fixing a typo is a great first contribution

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

## Development Workflow

### 1. Fork and Clone

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/your-username/git-perf.git
cd git-perf
```

### 2. Create a Branch

Use descriptive branch names that reflect your changes:

```bash
# For new features
git checkout -b feat/your-feature-name

# For bug fixes
git checkout -b fix/issue-description

# For documentation
git checkout -b docs/what-youre-documenting
```

### 3. Set Up Your Environment

```bash
# Ensure Rust toolchain is available
rustc --version
cargo --version

# Verify formatting tools
cargo fmt --version
cargo clippy --version

# Build the project
cargo build

# Run tests
cargo nextest run -- --skip slow
```

### 4. Make Your Changes

- Keep changes focused and atomic (one logical change per commit)
- Write clear, descriptive commit messages (see [Commit Guidelines](#commit-message-guidelines))
- Add tests for new functionality
- Update documentation as needed

### 5. Before Submitting

Run the complete pre-submission checklist:

```bash
# Format code (REQUIRED)
cargo fmt

# Run linting (REQUIRED)
cargo clippy

# Run tests (REQUIRED)
cargo nextest run -- --skip slow

# Ensure no warnings
cargo build --workspace

# If you modified CLI, regenerate documentation
./scripts/generate-manpages.sh
```

## Code Quality Standards

All code contributions must meet these standards:

### Required Checks

✅ **Format your code**:
```bash
cargo fmt
```

✅ **Run linting**:
```bash
cargo clippy
```

✅ **Run tests**:
```bash
cargo nextest run -- --skip slow
```

✅ **Ensure no warnings**:
```bash
cargo build --workspace
```

### Rust Best Practices

- Follow idiomatic Rust code patterns
- Use proper error handling with `Result` and `Option` types
- Use meaningful variable and function names
- Add documentation comments (`///`) for public APIs
- Avoid `unsafe` code unless absolutely necessary and well-justified
- Write code that is:
  - **Memory-conscious**: Avoid unnecessary allocations
  - **Fast**: Performance matters for a measurement tool
  - **Correct**: Handle errors gracefully, validate inputs

## Testing Requirements

### Running Tests

The project uses `cargo-nextest` for testing:

```bash
# Run standard tests (excludes slow tests) - recommended for pre-submission
cargo nextest run -- --skip slow

# Run full test suite including slow tests
cargo nextest run

# Run tests for a specific package
cargo nextest run -p git_perf
cargo nextest run -p cli_types
```

### Writing Tests

When adding new functionality:
- ✅ Add tests before implementing the feature (TDD approach)
- ✅ Test edge cases and error conditions
- ✅ Use descriptive test names that explain what is being tested
- ✅ Keep tests focused and isolated
- ✅ Add integration tests for end-to-end workflows

Example test structure:
```rust
#[test]
fn test_measurement_parsing_handles_empty_input() {
    // Arrange
    let input = "";

    // Act
    let result = parse_measurement(input);

    // Assert
    assert!(result.is_err());
}
```

## Submitting Changes

### Opening Issues

**Before opening an issue**, please:
- Search existing issues to avoid duplicates
- Check if your issue is already fixed in the latest version

**When reporting bugs**, include:
- git-perf version: `git perf --version`
- Operating system and version
- Rust version: `rustc --version`
- Minimal, Complete, and Verifiable example to reproduce
- Expected vs actual behavior
- Relevant error messages or logs

**When requesting features**, describe:
- The problem you're trying to solve
- Why existing features don't address it
- Your proposed solution (if any)
- Alternative approaches you've considered
- Potential drawbacks or trade-offs

### Creating Pull Requests

**Recommended workflow**:
1. **For significant changes**: Open an issue first to discuss the approach
2. **For small changes**: Feel free to submit a PR directly

**Pull Request Guidelines**:

1. **Create focused, atomic PRs**: Each PR should have a single, clear purpose
2. **Write clear descriptions**: Explain what changes you made and why
3. **Follow conventional commits**: Both commit messages AND PR titles (see below)
4. **Link related issues**: Use "Fixes #123" or "Closes #456" in PR description
5. **Be responsive**: Address review feedback promptly

**Creating the PR**:

```bash
# Push your branch
git push -u origin your-branch-name

# Create PR with proper conventional commits title
gh pr create --title "feat(scope): description of change" --body "
## Summary
Brief description of changes

## Changes Made
- Change 1
- Change 2

## Testing
How to test these changes

Fixes #123
"
```

### Pull Request Checklist

Before requesting review, ensure:

- [ ] Code is formatted with `cargo fmt`
- [ ] All tests pass: `cargo nextest run -- --skip slow`
- [ ] Linting passes: `cargo clippy` (no warnings)
- [ ] Commit messages follow Conventional Commits format
- [ ] PR title follows Conventional Commits format
- [ ] Documentation is updated (if needed)
- [ ] Manpages regenerated (if CLI was modified): `./scripts/generate-manpages.sh`
- [ ] Tests are added for new functionality
- [ ] Breaking changes are clearly documented

## Commit Message Guidelines

### Conventional Commits Format

**CRITICAL**: All commit messages AND pull request titles MUST follow [Conventional Commits](https://www.conventionalcommits.org/).

This is enforced by CI and enables:
- Automated changelog generation
- Semantic versioning
- Clear project history
- Better collaboration

**Format**:
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Commit Types

| Type | Description | Use When |
|------|-------------|----------|
| `feat` | New features | Adding new functionality |
| `fix` | Bug fixes | Fixing incorrect behavior |
| `docs` | Documentation only | README, comments, guides |
| `refactor` | Code refactoring | Restructuring without behavior change |
| `test` | Adding or updating tests | Test additions/modifications |
| `chore` | Maintenance tasks | Dependencies, tooling, cleanup |
| `perf` | Performance improvements | Optimizations |
| `build` | Build system changes | Cargo.toml, build scripts |
| `ci` | CI/CD changes | GitHub Actions, workflows |
| `revert` | Reverts previous commits | Undoing previous changes |

### Scopes (Optional but Recommended)

Common scopes in this project:
- `cli_types` - CLI types crate
- `git_perf` - Main git-perf crate
- `config` - Configuration system
- `audit` - Audit functionality
- `docs` - Documentation
- `test` - Tests
- `ci` - Continuous integration

### Examples

✅ **Good Commit Messages**:
```
feat(cli_types): add new measurement command
fix(audit): handle empty measurement data correctly
docs: improve installation instructions in README
test(integration): add git interop edge case tests
chore(deps): update clap to 4.5.0
perf(parser): optimize measurement parsing by 30%
```

❌ **Bad Commit Messages**:
```
Add new feature                     # Missing type prefix
Update README                       # Missing type prefix
fix stuff                          # Too vague, no description
feat: Add new measurement command   # Inconsistent capitalization
WIP                                 # Not descriptive
```

### Atomic Commits

We ask that commits are **atomic**, meaning they:
- Are complete and functional
- Have a single, clear responsibility
- Can be understood in isolation
- Make it easy to review and revert if needed

**Example of good atomic commits**:
```bash
git commit -m "feat(audit): add baseline threshold configuration"
git commit -m "test(audit): add tests for threshold edge cases"
git commit -m "docs(audit): document threshold configuration"
```

## Documentation

### When to Update Documentation

Update documentation when:
- Adding new features or commands
- Changing existing functionality
- Modifying configuration options
- Adding new examples or use cases
- Fixing bugs that were caused by unclear docs

### Documentation Types

1. **Code Comments**:
   - Use `///` for public APIs
   - Explain *why*, not just *what*
   - Include examples for complex functions

2. **README**:
   - Keep user-facing documentation current
   - Add new features to the feature list
   - Update examples if behavior changes

3. **Manpages**:
   - Automatically regenerated from CLI definitions
   - Run `./scripts/generate-manpages.sh` after CLI changes
   - Commit generated docs with code changes

4. **Integration Guides**:
   - Update if setup process changes
   - Add troubleshooting for common issues

### Regenerating Manpages

If you modify the CLI (commands, arguments, descriptions):

```bash
# Regenerate manpages and markdown documentation
./scripts/generate-manpages.sh

# Verify the changes
git diff docs/

# Commit the regenerated documentation
git add docs/manpages/ docs/cli/
git commit -m "docs: regenerate manpages for CLI changes"
```

## Project Goals

Understanding the project goals helps you make contributions that align with the project's direction:

### Core Principles

1. **Performance First**: git-perf is a performance measurement tool - it must be fast
2. **Correctness**: Accurate measurements and reliable regression detection
3. **Git Integration**: Seamless integration with Git workflows and git-notes
4. **Developer Experience**: Easy to set up, use, and integrate
5. **Backwards Compatibility**: Avoid breaking changes when possible

### What We Value

- **Simple, focused changes** over large, complex refactors
- **Well-tested code** over "it works on my machine"
- **Clear documentation** over clever code
- **Incremental improvements** over perfect solutions
- **Community feedback** over individual preferences

## Project Structure

This is a Rust workspace with multiple crates:

```
git-perf/
├── cli_types/          # CLI type definitions and argument parsing
│   ├── src/
│   └── Cargo.toml
├── git_perf/           # Core functionality and main binary
│   ├── src/
│   └── Cargo.toml
├── docs/               # Documentation
│   ├── INTEGRATION_TUTORIAL.md
│   ├── cli/
│   └── manpages/
├── scripts/            # Build and maintenance scripts
│   └── generate-manpages.sh
├── .github/            # GitHub Actions workflows and actions
│   ├── workflows/
│   └── actions/
└── Cargo.toml          # Workspace configuration
```

## Getting Help

Don't be shy about asking for help! We're here to support you.

### Resources

- **Documentation**:
  - [README](README.md) - Project overview
  - [Integration Tutorial](docs/INTEGRATION_TUTORIAL.md) - Setup guide
  - [CLAUDE.md](CLAUDE.md) - Development guidelines

- **Issues**:
  - Search [existing issues](https://github.com/terragonlabs/git-perf/issues) for similar questions
  - Open a new issue for bugs, features, or questions
  - Use labels to help categorize your issue

- **Pull Requests**:
  - Look at [recent PRs](https://github.com/terragonlabs/git-perf/pulls?q=is%3Apr) for examples
  - Review feedback is a learning opportunity, not criticism

### Common Questions

**Q: I'm new to Rust, can I still contribute?**
A: Absolutely! Start with documentation, tests, or small bug fixes. We're happy to help you learn.

**Q: How long does it take to get a PR reviewed?**
A: We try to review PRs within a few days. If it's been longer, feel free to ping us.

**Q: My PR got feedback asking for changes, what do I do?**
A: Make the requested changes and push them to your branch. The PR will update automatically.

**Q: Should I open an issue before submitting a PR?**
A: For significant changes, yes. For small fixes (typos, bugs), you can submit a PR directly.

**Q: Can I work on an issue that's already assigned?**
A: Best to ask first! The assignee might be actively working on it.

## Recognition

We value all contributions and recognize contributors through:
- Git commit history and co-author tags
- Release notes and changelogs
- Project README (for significant contributions)
- Community shout-outs

---

Thank you for contributing to git-perf! Your efforts help make performance measurement better for everyone.
