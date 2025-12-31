# Git-Perf Plans Status Overview

**Last Updated:** 2025-12-31

This document provides an overview of all planned features for git-perf, their current implementation status, and what remains to be done.

## Summary

| Plan | Status | Priority | Notes |
|------|--------|----------|-------|
| Change Point Detection | **Ready for Implementation** | Medium | Algorithm designed, awaiting implementation |
| Config Command | **✅ Complete** | - | Implemented and functional |
| Filter Argument (Audit/Report) | **✅ Complete** | - | Implemented and deployed |
| GitHub Pages Integration & Templating | **Phase 2b Complete** | High | Multi-section dashboards working, subdirectory support pending |
| Import Test/Benchmark Runtimes | **✅ Complete** | - | JUnit XML and Criterion JSON support implemented |
| Measurement Units Support | **✅ Complete** | - | Config-based units with auto-scaling fully deployed |
| Parse/Convert Units in Audit | **✅ Complete** | - | Auto-scaling implemented (9000ms → 9s) |
| Size Subcommand | **Phase 1 Complete** | Medium | Basic implementation done, testing deferred |
| Status and Reset Commands | **Planning** | Medium | Design complete, no implementation yet |

**Total: 9 plans (6 complete, 1 in progress, 1 ready for implementation, 1 in planning)**

## Completed Plans (6)

### 1. Filter Argument for Audit and Report ✅
**Status:** Completed 2025-10-27
**Files:**
- `git_perf/src/filter.rs` - Regex compilation and matching
- Added `--filter` (`-f`) argument to both audit and report commands
- Supports multiple filters with OR semantics

**Usage:**
```bash
git perf report -f "bench.*" -f "test_.*"
git perf audit -m benchmark -f ".*_production"
```

### 2. Import Test and Benchmark Runtimes ✅
**Status:** Completed 2025-10-26
**Files:**
- `git_perf/src/import.rs` - Import command handler
- `git_perf/src/parsers/` - JUnit XML and Criterion JSON parsers
- `git_perf/src/converters/` - Measurement conversion logic

**Supported Formats:**
- **JUnit XML** - For test runtimes (pytest, Jest, nextest, etc.)
- **Criterion JSON** - For Rust benchmark results

**Usage:**
```bash
cargo nextest run --profile ci  # Outputs JUnit XML
git perf import junit target/nextest/ci/junit.xml

cargo criterion --message-format json > bench.json
git perf import criterion-json bench.json
```

### 3. Measurement Units Support ✅
**Status:** Completed 2025-10-17 (all 5 phases)
**Files:**
- `git_perf/src/config.rs` - `measurement_unit()` function
- Config-only approach (no data model changes)

**Configuration:**
```toml
[measurement."build_time"]
unit = "ms"

[measurement."memory_usage"]
unit = "bytes"
```

**Features:**
- Units displayed in audit output, HTML reports, and CSV exports
- No changes to measurement storage format
- Perfect backward compatibility

### 4. Parse and Auto-Scale Units in Audit Output ✅
**Status:** Completed 2025-10-18
**Files:**
- `git_perf/src/units.rs` - Parsing and formatting with auto-scaling
- Dependencies: `fundu`, `bytesize`, `human-repr`

**Auto-Scaling Examples:**
- 9000ms → 9s
- 15000KB → 15MB
- 9000KB/s → 9MB/s

**Supported Unit Types:**
- Duration (ns, μs, ms, s, min, h, d, w)
- Data size (B, KB, MB, GB, KiB, MiB, GiB)
- Data rate (B/s, KB/s, MB/s, etc.)

### 5. Size Subcommand ✅ (Phase 1)
**Status:** Phase 1 Complete 2025-10-27
**Files:**
- `git_perf/src/size.rs` - Size calculation and reporting

**Features Implemented:**
- Calculate total measurement storage size
- Show commit count with measurements
- `--detailed` - Breakdown by measurement name
- `--disk-size` - Show compressed on-disk size
- `--include-objects` - Show repository statistics
- `--format bytes|human` - Output format selection

**Usage:**
```bash
git perf size                           # Basic summary
git perf size --detailed                # Breakdown by measurement
git perf size --disk-size               # Show compressed size
git perf size --include-objects         # Include repo stats
```

**Pending:** Integration tests (deferred to future PR)

### 6. Config Command with --list Flag ✅
**Status:** Completed
**Plan File:** `docs/plans/config-command.md`
**Files:**
- `git_perf/src/config_cmd.rs` - Config information gathering and display
- Supports `--list`, `--detailed`, `--json`, `--validate` flags
- Shows git context, config sources, and measurement settings

**Features Implemented:**
- Display git context (branch, repository root)
- Show configuration sources (system vs local)
- List all measurement configurations
- JSON output format for scripting
- Configuration validation with actionable errors
- Per-measurement filtering

**Usage:**
```bash
git perf config --list                    # Summary
git perf config --list --detailed         # All settings
git perf config --list --json             # JSON output
git perf config --list --validate         # Check config
git perf config --list --measurement M    # Specific measurement
```

## Plans Ready for Implementation (1)

### 7. Change Point Detection
**Status:** Ready for Implementation
**Plan File:** `docs/plans/change-point-detection.md`

**Overview:**
Add PELT (Pruned Exact Linear Time) algorithm to detect when performance shifts occurred in historical data. Complements existing z-score regression testing.

**Key Features:**
- Visualize epoch boundaries and change points in HTML reports (hidden by default)
- Warn in audit if change points exist in current epoch
- O(n) complexity, mathematically optimal segmentation

**Implementation Estimate:** 3-4 weeks (~660 lines of code)

**Files to Create:**
- `git_perf/src/change_point.rs` (~400 lines)

**Files to Modify:**
- `git_perf/src/reporting.rs` (~150 lines)
- `git_perf/src/audit.rs` (~50 lines)
- `git_perf/src/cli.rs` (~30 lines)
- `git_perf/src/config.rs` (~30 lines)

**Configuration:**
```toml
[change_point]
enabled = true
min_data_points = 10
min_magnitude_pct = 5.0
penalty = 0.5  # Lower = more sensitive (0.3-0.5 high, 0.5-1.0 balanced, 1.0+ conservative)

[change_point."build_time"]
penalty = 1.0  # Less sensitive for this measurement
```

**Priority:** Medium - Valuable for understanding performance shifts, but not blocking other work

## Plans In Progress (1)

### 8. GitHub Pages Integration and Templating
**Status:** Phase 2b Complete (Multi-Configuration Templates)
**Plan File:** `docs/plans/github-pages-integration-and-templating.md`

**Completed (Phase 2b):**
- ✅ Multi-section dashboard templates with `{{SECTION[...]}}` syntax
- ✅ Per-section parameters: filter, aggregate-by, separate-by, depth, title, show-epochs, show-changes
- ✅ 20+ unit tests, comprehensive documentation
- ✅ Example templates: performance-overview.html, simple-dashboard.html, dashboard-example.html
- ✅ Integration with change point detection and epoch visualization

**Dashboard Template Example:**
```html
<section>
    <h2>Test Duration Trends (Raw Data)</h2>
    {{SECTION[test-durations-raw]
        measurement-filter: ^test::
        aggregate-by: none
    }}
</section>

<section>
    <h2>Benchmark Performance (Median)</h2>
    {{SECTION[bench-median]
        measurement-filter: ^bench::
        aggregate-by: median
    }}
</section>
```

**Pending Phases:**
- **Phase 1:** Subdirectory support for reports (weeks 1-2)
  - Add `reports-subdirectory` input to report action
  - Update cleanup script to respect subdirectory
  - Concurrency documentation

- **Phase 2 (Basic Templating):** Template placeholder syntax (weeks 3-5)
  - Template loading from `.git-perf/report-template.html`
  - Placeholder substitution (TITLE, PLOTLY_HEAD, PLOTLY_BODY, etc.)
  - CLI flags: `--template`, `--custom-css`, `--title`

- **Phase 3:** Index generation (weeks 5-6)
  - `generate-index` subcommand
  - Scan gh-pages for reports
  - Categorize by branch/commit/custom

- **Phase 4:** Documentation & examples (week 7)
  - Integration guides for MkDocs, Jekyll, static sites
  - Migration guide for existing users

**Priority:** High - Many users want reports integrated with documentation sites

## Plans In Planning Stage (1)

### 9. Status and Reset Commands
**Status:** Planning
**Plan File:** `docs/plans/status-and-reset-commands.md`
**Issue:** #485

**Overview:**
Add commands to manage locally pending measurements (not yet pushed):
- `git perf status` - View pending measurements
- `git perf reset` - Discard pending measurements

**Proposed Features:**

**Status Command:**
- Count of commits with pending measurements
- List unique measurement names
- Optional detailed per-commit breakdown
- Similar to `git status` UX

**Reset Command:**
- Drop all locally pending measurements
- Only affects unpushed data (safe)
- Confirmation prompt with `--force` override
- `--dry-run` preview mode

**Files to Create:**
- `git_perf/src/status.rs` (~300 lines)
- `git_perf/src/reset.rs` (~250 lines)

**Files to Modify:**
- `cli_types/src/lib.rs` - Add Status and Reset commands
- `git_perf/src/cli.rs` - Wire up handlers

**Usage Examples:**
```bash
# Status
git perf status                # Summary of pending
git perf status --detailed     # Per-commit breakdown

# Reset
git perf reset                 # With confirmation
git perf reset --dry-run       # Preview
git perf reset --force         # Skip confirmation
```

**Priority:** Medium - Useful workflow improvement, prevents accidental pushes of test data

**Complexity:** Medium - Requires understanding of write refs architecture

## Implementation Priority Recommendations

Based on impact, complexity, and user demand:

### High Priority
1. **GitHub Pages Integration (Phase 1)** - Many users blocked by this
   - Weeks: 1-2
   - Complexity: Medium
   - Blocking: Users with existing docs sites

### Medium Priority
2. **Change Point Detection** - High value for understanding performance
   - Weeks: 3-4
   - Complexity: Medium-High
   - Impact: Better insights into performance trends

3. **Status and Reset Commands** - Workflow improvement
   - Weeks: 1-2
   - Complexity: Medium
   - Impact: Prevents accidental data pollution

4. **Size Subcommand (Testing)** - Complete existing work
   - Weeks: 0.5
   - Complexity: Low
   - Impact: Finish what's started

### Low Priority
5. **GitHub Pages (Phases 2-4)** - After Phase 1 proves value
   - Weeks: 3-4
   - Complexity: Medium
   - Impact: Enhanced customization

## Implementation Notes

### Completed Work Quality
All completed features have:
- ✅ Full test coverage
- ✅ Comprehensive documentation
- ✅ Clean clippy/fmt
- ✅ Regenerated manpages
- ✅ Backward compatibility maintained

### Pending Work Considerations
For plans marked "Ready" or "Planning":
- All have detailed technical designs
- Success criteria defined
- Testing strategies outlined
- Risk mitigations documented
- No blockers identified

### Resource Estimates
- **Change Point Detection:** 3-4 weeks full-time
- **GitHub Pages (all phases):** 6-7 weeks full-time
- **Status/Reset Commands:** 1-2 weeks full-time
- **Size Testing:** 0.5 weeks full-time

**Total for all pending work:** ~11-14 weeks full-time

## Files Status

### Existing Implementation Files
These files exist in the codebase and contain implemented features:
- ✅ `git_perf/src/filter.rs` - Regex filtering (completed)
- ✅ `git_perf/src/import.rs` - Import command (completed)
- ✅ `git_perf/src/parsers/` - JUnit/Criterion parsers (completed)
- ✅ `git_perf/src/units.rs` - Unit parsing and auto-scaling (completed)
- ✅ `git_perf/src/size.rs` - Size calculation (Phase 1 complete)
- ✅ `git_perf/src/config_cmd.rs` - Config command (completed)

### Files Requiring Creation
These files need to be created for pending plans:
- ⏳ `git_perf/src/change_point.rs` - Change point detection algorithm
- ⏳ `git_perf/src/status.rs` - Status command implementation
- ⏳ `git_perf/src/reset.rs` - Reset command implementation

### Test Files Requiring Creation
- ⏳ `test/test_size.sh` - Size command integration tests
- ⏳ `test/test_status.sh` - Status command integration tests
- ⏳ `test/test_reset.sh` - Reset command integration tests

## Recent Activity
Based on git log, recent work has focused on:
- Audit reporting improvements with `--separate-by` option
- Test infrastructure refactoring
- Git performance features (commit title/author for hover)

## Next Steps

### Immediate (Next Sprint)
1. Complete size subcommand testing
2. Begin GitHub Pages subdirectory support (Phase 1)

### Short-term (Next Quarter)
1. Implement change point detection
2. Add status/reset commands for workflow improvement

### Long-term (Future Quarters)
1. Complete GitHub Pages templating (Phases 2-4)
2. Consider community-requested features

## Notes on Plan Organization

All plans follow a consistent structure:
- **Status** - Current implementation stage
- **Overview** - What the feature does
- **Motivation** - Why it's needed
- **Goals/Non-Goals** - Scope definition
- **Design** - Technical approach
- **Implementation Phases** - Step-by-step breakdown
- **Testing Strategy** - How to verify
- **Success Criteria** - Definition of done

Plans marked "Complete" are kept for historical reference and to document the implemented architecture.
