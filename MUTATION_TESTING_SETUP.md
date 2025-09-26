# Mutation Testing Setup Documentation

This document provides instructions for completing the mutation testing setup for git-perf.

## Phase 1 Implementation Status

### ✅ Completed Tasks

1. **Cargo-mutants Configuration** - Added to `git_perf/Cargo.toml`
   - Skip tests, examples, benches
   - 60-second timeout
   - 4 parallel jobs

2. **GitHub Actions Workflows**
   - `mutation-testing.yml` - Full mutation testing on push/PR
   - `mutation-check.yml` - Critical module checks for PRs

3. **Artifact Storage** - Configured in workflows
   - Mutation reports stored for 30 days
   - Critical module reports stored for 7 days

### 🔄 Remaining Setup Tasks

#### Branch Protection Rules (Manual Setup Required)

To complete the PR checks setup, configure the following branch protection rules in GitHub:

1. Go to **Settings > Branches** in the GitHub repository
2. Edit the `master` branch protection rule (or create one)
3. Add the following **Required status checks**:
   - `Critical Module Mutation Check`
   - `Mutation Testing` (optional, for visibility)

#### Installation of cargo-mutants

The `cargo install cargo-mutants` command was started but may need completion:

```bash
# Complete the installation if needed
cargo install cargo-mutants --version 25.3.1

# Verify installation
cargo mutants --version
```

## Generating Baseline Report

Once cargo-mutants is installed, generate the initial baseline report:

```bash
cd git_perf
cargo mutants --output baseline-report.json --jobs 4 --timeout 60
```

This will provide the current mutation testing baseline for tracking improvements.

## Critical Module Targets

The mutation check workflow enforces these targets:
- **stats.rs**: 90% mutation score
- **audit.rs**: 85% mutation score
- **config.rs**: 80% mutation score

## Workflow Behavior

### Full Mutation Testing (`mutation-testing.yml`)
- Runs on all pushes to master and pull requests
- Generates complete mutation report
- Posts summary comment on PRs
- Stores artifacts for 30 days

### Critical Module Check (`mutation-check.yml`)
- Runs only when critical modules are modified
- Enforces mutation score targets
- Fails PR if targets not met
- Optimized for fast feedback (30-minute timeout)

## Next Steps (Phase 2)

After completing Phase 1 setup:

1. **Fix Error Handling in stats.rs**
   - Replace `unwrap()` calls at lines 88 and 156
   - Add tests for NaN/infinity handling
   - Target: 90% mutation score

2. **Enhance audit.rs Conditional Logic**
   - Fix `unwrap()` at line 28
   - Test all conditional branches
   - Target: 85% mutation score

3. **Strengthen config.rs Error Coverage**
   - Test file operation error scenarios
   - Improve configuration validation
   - Target: 80% mutation score

## Troubleshooting

### Slow Installation
If `cargo install cargo-mutants` is slow:
- Use pre-built binaries from releases
- Cache the installation in CI
- Consider using a different runner

### High Resource Usage
- Adjust `jobs` parameter in configuration
- Increase timeout if needed
- Use selective testing for large codebases

### False Positives
- Review mutation results manually
- Exclude irrelevant mutations if needed
- Focus on high-value mutations

## Monitoring

- Weekly mutation score reviews
- PR-based regression detection
- Quarterly target adjustments
- Build time impact monitoring (target: <20% increase)