# Agent Instructions for git-perf

Rust workspace (2 crates: `cli_types` v0.2.0, `git-perf` v0.18.0) providing Git repository performance measurement tools with git-notes storage, statistical analysis, and interactive reporting.

## Quick Reference Commands

```bash
cargo fmt                              # Format code (required before commit)
cargo clippy                           # Lint code (must pass)
cargo nextest run -- --skip slow       # Run tests (standard)
cargo nextest run                      # Run full test suite including slow tests
./scripts/generate-manpages.sh        # Regenerate docs after CLI changes
```

**Setup (one-time)**:
```bash
cargo install cargo-nextest --locked
export PATH="/usr/local/cargo/bin:$PATH"  # Add to shell profile
```

## Pull Request Requirements

**MANDATORY for AI Agents**:
```bash
# ALWAYS use --title with Conventional Commits format
gh pr create --title "type(scope): description" --body "..."

# NEVER rely on auto-generated titles - they break CI
gh pr create --body "..."  # FORBIDDEN
```

**Format**: `type(scope): lowercase description`

**Types**: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`, `perf`, `build`, `ci`, `revert`

**Scopes** (primary crates): `cli_types`, `git_perf`
**Scopes** (modules): `config`, `audit`, `import`, `git`, `stats`, `reporting`, `parsers`

**Examples**:
```
feat(cli_types): add measurement export option
fix(audit): handle empty measurement data correctly
docs: update installation guide
chore(deps): update clap to 4.5.0
refactor(git): simplify notes retrieval logic
```

## Testing

**Test Types**:
- **Bash integration tests**: 46+ scripts in `/test/` (primary test suite)
- **Rust unit tests**: Throughout `git_perf/src/` modules
- **Criterion benchmarks**: `git_perf/benches/` (read.rs, add.rs, sample_ci_bench.rs)
- **Mutation testing**: Configured in `.cargo/mutants.toml`

**Key test commands**:
```bash
cargo nextest run -- --skip slow       # Skip slow tests (default)
cargo nextest run                      # Full suite including slow tests
./test/run_tests.sh                   # Run all bash integration tests
```

## Documentation Generation

**IMPORTANT**: Run `./scripts/generate-manpages.sh` after modifying:
- `cli_types/src/lib.rs` (CLI type definitions)
- `git_perf/src/cli.rs` (CLI implementation)
- Any command-line argument changes

**What it generates**:
- Man pages: `man/man1/git-perf*.1`
- Markdown docs: `docs/manpage.md`

**CI validates** that generated docs match source. Commit regenerated docs with your changes.

## Code Quality Standards

- Follow Rust idioms and best practices
- Use `Result` and `Option` for error handling
- Meaningful variable/function names
- No compiler warnings allowed
- No clippy warnings allowed

## Project Architecture

**Crate Structure**:
- `cli_types/` - Shared CLI types (Commands, ReductionFunc, DispersionMethod, etc.)
- `git_perf/` - Main application (11 commands, git integration, statistics, reporting)

**Key Modules** (git_perf/src/):
- `audit.rs` (56KB) - Performance validation and threshold checking
- `config.rs` (35KB) - Configuration file management (.gitperfconfig)
- `reporting.rs` (31KB) - Plotly-based interactive HTML reports
- `git/git_interop.rs` (39KB) - Git-notes operations, push/pull
- `stats.rs` (23KB) - Statistical analysis (stddev, MAD, z-scores)
- `import.rs` (19KB) - JUnit XML and Criterion JSON import
- `parsers/` - Format parsers (criterion_json.rs, junit_xml.rs)

**Core Features**:
- Git-notes storage (`refs/notes/perf-v3`)
- Statistical validation (configurable dispersion methods: stddev, MAD)
- Multi-format import (JUnit XML from nextest/pytest/Jest, Criterion JSON)
- Interactive Plotly HTML reports with filtering/aggregation
- Sparkline terminal visualization
- Shallow clone detection and warnings

## Configuration

**File**: `.gitperfconfig` (TOML format)
```toml
[measurement]
dispersion_method = "mad"           # or "stddev"
min_relative_deviation = 5.0
min_measurements = 3
aggregate_by = "median"             # min, max, median, mean
sigma = 3.5

[measurement."specific_test"]       # Per-measurement overrides
min_relative_deviation = 10.0

[change_point]
penalty = 0.5                       # PELT sensitivity (0.3=high, 0.5=default, 1.0+=low)
min_data_points = 10
min_magnitude_pct = 5.0

[change_point."specific_test"]      # Per-measurement overrides
penalty = 0.3                       # More sensitive for this measurement
```

**Precedence**: CLI flags > Per-measurement config > Default config > Built-in defaults

### Change Point Detection Tuning

The `penalty` parameter controls how many change points PELT detects:
- **0.3-0.5**: High sensitivity - detects multiple change points
- **0.5-1.0**: Balanced (default 0.5)
- **1.0+**: Conservative - only major shifts

Lower penalty values are better for detecting multiple regime changes in performance data.

## Requirements

- **Git**: 2.43.0+ (version checked automatically)
- **Rust**: Edition 2021
- **nextest**: Required for test execution

## Build & Release

**Build script** (`build.rs`): Auto-generates manpages and markdown docs during `cargo build`

**Distribution** (dist-workspace.toml):
- Tool: cargo-dist 0.29.0
- Targets: macOS ARM64/x86_64, Linux ARM64/x86_64/musl
- Installer: Shell script
- Release automation: release-plz

## Environment Setup

**PATH Configuration** (required for background agents):
```bash
export PATH="/usr/local/cargo/bin:$PATH"

# Verify setup
rustc --version && cargo fmt --version && cargo nextest --version
```

## Troubleshooting

**Issue**: Rust toolchain not found
**Fix**: Add PATH export to environment: `export PATH="/usr/local/cargo/bin:$PATH"`

**Issue**: Tests fail on shallow clone
**Fix**: Git operations require full history. Use `git fetch --unshallow` or clone with full depth.

**Issue**: Manpage validation fails in CI
**Fix**: Run `./scripts/generate-manpages.sh` and commit the regenerated docs.

## GitHub Templates

- `.github/ISSUE_TEMPLATE/`: bug_report.md, feature_request.md, documentation.md
- `.github/pull_request_template.md`: Checklist for testing and verification

## See Also

**For Contributors:**
- **[CONTRIBUTING.md](./CONTRIBUTING.md)** - Complete contribution guidelines with code quality standards, testing requirements, and PR process

**For Users:**
- **[Documentation Index](./docs/README.md)** - All available documentation
- **[Integration Tutorial](./docs/INTEGRATION_TUTORIAL.md)** - GitHub Actions setup guide
