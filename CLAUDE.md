# Agent Instructions for git-perf Project

Rust workspace with `cli_types` and `git_perf` crates providing Git repository performance measurement tools.

## 🚨 CRITICAL: Pull Request Requirements for AI Agents

**MANDATORY PR Creation**:
```bash
# ALWAYS use --title with Conventional Commits format
gh pr create --title "type(scope): description" --body "..."

# NEVER rely on auto-generated titles
gh pr create --body "..."  # ❌ FORBIDDEN
```

**AI Agent Requirements**:
- MUST use `--title` parameter with `type(scope): description` format
- NEVER create PR without explicit title
- GitHub auto-generates non-compliant titles from branch names/commits

## Pre-Submission Checklist

**Required Commands** (run before every submission):
```bash
cargo fmt                              # Format code
cargo nextest run -- --skip slow       # Run tests (exclude slow)
cargo clippy                           # Lint code
./scripts/generate-manpages.sh         # If cli_types changed
```

**Setup** (install once):
```bash
cargo install cargo-nextest --locked
export PATH="/usr/local/cargo/bin:$PATH"  # Add to shell profile
```

## Conventional Commits (CI-Enforced)

**Format**: `type(scope): lowercase description`

**Types**:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation only
- `refactor:` - Code restructuring (no functional change)
- `chore:` - Maintenance (deps, config)
- `test:` - Test changes
- `perf:` - Performance improvements
- `build:` - Build system changes
- `ci:` - CI/CD changes
- `revert:` - Revert previous commit

**Scopes**: `cli_types`, `git_perf`, `config`, `audit`, `docs`, `test`

**Examples**:
```
✅ feat(cli_types): add measurement export
✅ fix(audit): handle empty data
✅ docs: improve installation steps
✅ chore(deps): update clap to 4.5.0

❌ Add new feature           # Missing type
❌ fix stuff                 # Too vague
❌ feat: Add Feature         # Wrong capitalization
```

**PR Title Validation**:
- Starts with valid type (`feat:`, `fix:`, `docs:`, etc.)
- Lowercase after colon
- Includes scope when relevant
- Descriptive but concise

## Documentation

**Manpages** (required for CLI changes):
- Run `./scripts/generate-manpages.sh` after modifying `cli_types`
- Commit regenerated docs with code changes
- CI validates docs are up-to-date
- Custom version: `GIT_PERF_VERSION=1.0.0 ./scripts/generate-manpages.sh`

## Testing

**Standard**: `cargo nextest run -- --skip slow`
**Full suite**: `cargo nextest run`

## Code Quality Standards

- Follow Rust idioms and best practices
- Use `Result` and `Option` for error handling
- Meaningful variable/function names
- No warnings allowed

## Environment Setup

**PATH Configuration** (required for background agents):
```bash
export PATH="/usr/local/cargo/bin:$PATH"

# Verify
rustc --version && cargo fmt --version && cargo nextest --version
```

## Why These Rules Matter

**Conventional Commits**: Non-compliant titles break automated changelog generation, version management, release automation, and documentation tools.

**Code Quality**: Ensures consistency, catches bugs early, maintains readability, and passes CI/CD checks.

## GitHub Templates

- `.github/ISSUE_TEMPLATE/`: bug_report.md, feature_request.md, documentation.md
- `.github/pull_request_template.md`: Checklist for testing and verification

## Troubleshooting

**Issue**: Background agents not applying rustfmt
**Cause**: Rust toolchain not in PATH (`/usr/local/cargo/bin/`)
**Fix**: Add PATH export to environment
