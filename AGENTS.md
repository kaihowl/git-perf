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
cargo nextest run --skip slow
```

## Testing Policy

- **Test command**: `cargo nextest run --skip slow` (excludes slow tests)
- Ensure all tests pass before submitting code
- Use `cargo nextest run` for full test suite when needed
- This is the standard command for CI and local pre-submit

## Documentation & Build Process

- If changes are made to the `cli_types` crate, ensure any documentation regenerated as part of the build process is included in the commit
- Run `./scripts/generate-manpages.sh` and commit regenerated documentation
- Commit the regenerated docs alongside the code changes

### Manpage Documentation

- **Always update manpages** when making CLI changes (commands, arguments, descriptions)
- Run `./scripts/generate-manpages.sh` to regenerate manpages and markdown docs
- Manpages are automatically generated using `clap_mangen` and `clap_markdown`
- CI validates that documentation stays up-to-date with CLI definitions
- Use `GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh` for custom versioning

## Pull Request Standards

### Conventional Commits Requirement

**CRITICAL**: Both commit messages AND pull request titles MUST follow the [Conventional Commits specification](https://www.conventionalcommits.org/). This is enforced by CI and is non-negotiable.

### Conventional Commit Types

| Type | Description | Examples |
|------|-------------|----------|
| `feat:` | New features | `feat(cli): add audit command`, `feat: implement MAD dispersion` |
| `fix:` | Bug fixes | `fix(config): handle missing file gracefully`, `fix: resolve memory leak` |
| `docs:` | Documentation changes | `docs: update README installation steps`, `docs(api): add examples` |
| `refactor:` | Code refactoring (no functional changes) | `refactor(parser): simplify error handling` |
| `chore:` | Maintenance tasks | `chore: update dependencies`, `chore(deps): bump clap to 4.0` |
| `test:` | Test additions/changes | `test: add integration tests for audit`, `test(unit): cover edge cases` |
| `perf:` | Performance improvements | `perf: optimize measurement parsing`, `perf(db): reduce query time` |
| `build:` | Build system changes | `build: update cargo config`, `build(ci): optimize pipeline` |
| `ci:` | CI/CD changes | `ci: add release workflow`, `ci(test): run on multiple OS` |
| `revert:` | Reverts previous commits | `revert: undo performance optimization` |

### Scopes (Optional but Recommended)

Use scopes to specify the area of change:
- `(cli_types)` - changes to the CLI types crate
- `(git_perf)` - changes to the main git-perf crate
- `(config)` - configuration-related changes
- `(audit)` - audit system changes
- `(docs)` - documentation changes
- `(test)` - test-related changes

### Examples of Proper Conventional Commits

✅ **Good Examples:**
```
feat(cli_types): add new measurement command
fix(audit): handle empty measurement data
docs: improve installation instructions
chore(deps): update clap to 4.5.0
test(integration): add git interop tests
```

❌ **Bad Examples:**
```
Add new feature                     # Missing type prefix
Update README                       # Missing type prefix
fix stuff                          # Too vague
feat: Add new measurement command   # Inconsistent capitalization
```

### Creating Pull Requests

**IMPORTANT**: When creating pull requests, you MUST manually ensure the title follows Conventional Commits, regardless of auto-generated titles.

#### Steps for PR Creation

1. **Create commits with proper format:**
   ```bash
   git commit -m "docs(agents): enhance conventional commits guidance"
   ```

2. **Push branch:**
   ```bash
   git push -u origin feature-branch-name
   ```

3. **Create PR with correct title:**
   ```bash
   # GitHub CLI - MANUALLY specify the title
   gh pr create --title "docs(agents): enhance conventional commits guidance" --body "..."

   # OR via GitHub web interface - MANUALLY enter correct title
   ```

#### Common PR Creation Pitfalls

🚨 **WARNING**: GitHub often auto-generates PR titles from:
- Branch names (e.g., `improve-readme` → `"Improve readme"`)
- First commit messages
- Repository patterns

**Always manually verify and correct the PR title before submitting!**

#### PR Title Validation

Before submitting, verify your PR title:
- ✅ Starts with a valid type (`feat:`, `fix:`, `docs:`, etc.)
- ✅ Uses lowercase after the colon
- ✅ Is descriptive but concise
- ✅ Includes scope when relevant
- ✅ Matches the actual changes made

#### Examples of Title Corrections

| Auto-Generated (❌) | Corrected (✅) |
|-------------------|----------------|
| `Improve README` | `docs: improve README readability and organization` |
| `Fix bug in audit` | `fix(audit): handle missing measurement data` |
| `Add new feature` | `feat(cli): add measurement export functionality` |
| `Update dependencies` | `chore(deps): update clap and serde versions` |

## Pre-Submission Checklist

Before submitting any code, ensure:

### Code Quality
1. ✅ Run `cargo fmt` to format code
2. ✅ Run `cargo nextest run --skip slow` to verify tests pass
3. ✅ Run `cargo clippy` for additional code quality checks
4. ✅ Ensure all changes compile without warnings
5. ✅ If `cli_types` changed, run `./scripts/generate-manpages.sh` and commit regenerated documentation

### Conventional Commits Compliance
6. ✅ **Verify commit messages follow Conventional Commits format**
   ```bash
   # Check your commit messages
   git log --oneline -5
   # Each should start with: feat:, fix:, docs:, etc.
   ```

7. ✅ **Verify PR title follows Conventional Commits format**
   - Must start with valid type prefix (`feat:`, `fix:`, `docs:`, etc.)
   - Use lowercase after the colon
   - Include scope when relevant (e.g., `feat(cli_types):`)
   - Be descriptive but concise

### Final Verification Commands
```bash
# Verify formatting and tests
cargo fmt --check && cargo nextest run --skip slow && cargo clippy

# Check commit message format (should show proper conventional format)
git log --oneline -1

# When creating PR, manually verify title format before submitting
```

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

### Code Quality Rules
The `rustfmt` and `cargo clippy` rules are critical for:
- **Consistency**: All code follows the same formatting standards
- **Quality**: Catches potential bugs and enforces best practices
- **Maintainability**: Clean, readable code that's easy to modify
- **CI/CD**: Automated checks ensure code quality in the pipeline

### Conventional Commits Rules
The Conventional Commits standard is essential for:
- **Automated Releases**: Tools can automatically generate changelogs and determine version bumps
- **Clear History**: Anyone can quickly understand what changed by looking at commit/PR titles
- **Tooling Integration**: Various tools expect this format for automation
- **Professional Standards**: Industry-standard practice for open source projects
- **CI/CD Pipeline**: Automated workflows depend on consistent commit formatting

**Real Impact**: A single non-compliant PR title can break:
- Automated changelog generation
- Version management tools
- Release automation
- Project documentation tools

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

The project uses the default rustfmt configuration for consistent formatting across all environments.