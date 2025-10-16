# Plan: Support for Measurement Units

**Related Issue:** #330
**Status:** Complete
**Created:** 2025-10-14
**Updated:** 2025-10-15 - All phases implemented and deployed

## Overview

Add support for specifying and displaying units for performance measurements in git-perf through configuration. Units will not be stored with measurement data but will be defined in configuration and applied during reporting and display. This provides a simple, centralized way to manage unit information without changing the data model or serialization format.

## Motivation

Currently, git-perf stores numeric measurement values without any associated unit information. Users must rely on naming conventions (e.g., `build_time_ms` vs `build_time_sec`) to track units, which makes it difficult to:

- Provide meaningful axis labels and tooltips in reports
- Understand what a measurement value represents in documentation
- Display clear, professional reports with proper units

## Goals

1. **Configure units per measurement** - Define units in `.gitperfconfig`
2. **Display units in reports** - Show units in HTML report legends and axis labels
3. **Display units in CSV exports** - Include units in CSV column headers
4. **Display units in audit output** - Show units in audit command results
5. **No data model changes** - Keep measurements storage simple and unchanged
6. **Backward compatibility** - Existing measurements and configs work without modification

## Non-Goals (Future Work)

- Storing units with measurement data
- Runtime unit validation or consistency checking
- Automatic unit conversion (e.g., milliseconds to seconds)
- CLI flags for specifying units (config-only)

## Design

### 1. Configuration Only

Units are defined **only** in `.gitperfconfig` and applied at display time:

```toml
[measurement]
# Default unit for all measurements (if not specified)
# unit = "ms"

[measurement."build_time"]
unit = "ms"

[measurement."memory_usage"]
unit = "bytes"

[measurement."throughput"]
unit = "requests/sec"

[measurement."test_runtime"]
unit = "seconds"
```

#### Configuration Helper Function
Add a new function to `git_perf/src/config.rs`:

```rust
/// Returns the configured unit for a measurement, or None if not set.
pub fn measurement_unit(measurement: &str) -> Option<String> {
    let config = read_hierarchical_config().ok()?;
    config.get_with_parent_fallback("measurement", measurement, "unit")
}
```

**Precedence (follows existing pattern):**
1. Measurement-specific config (`[measurement."name"].unit`)
2. Parent default config (`[measurement].unit`)
3. None (no unit)

### 2. No Data Model Changes

**No changes to:**
- `MeasurementData` struct - remains unchanged
- Serialization format - no unit field added
- Deserialization logic - no unit parsing needed
- Storage mechanism - measurements stored as before

**Rationale:**
- Simpler implementation with fewer moving parts
- No serialization format changes or backward compatibility concerns
- Centralized unit management in configuration
- Units can be changed without re-recording measurements
- No increase in data storage size

### 3. Display and Reporting

Units are applied **only during display**, retrieved from configuration at report/output time.

#### Audit Output
Include units in audit command results in `git_perf/src/audit.rs`:

**Current output:**
```
✓ build_time: 42.5 (within acceptable range)
```

**New output with unit:**
```
✓ build_time: 42.5 ms (within acceptable range)
```

**Implementation approach:**
```rust
// When displaying audit results
let unit = config::measurement_unit(&measurement_name);
let value_display = match unit {
    Some(u) => format!("{} {}", value, u),
    None => format!("{}", value),
};
```

#### HTML Report
Update report generation to include units in `git_perf/src/reporting.rs`:

**Changes needed:**
- **Legend entries:** Append unit to measurement name (e.g., "build_time (ms)")
- **Axis labels:** When all measurements have the same unit, include in Y-axis label (e.g., "Performance (ms)")
- **Hover tooltips:** Show value with unit (e.g., "42.5 ms")

**Implementation approach:**
```rust
// When generating report
let unit = config::measurement_unit(&measurement_name);
let display_name = match unit {
    Some(u) => format!("{} ({})", measurement_name, u),
    None => measurement_name.to_string(),
};
```

#### CSV Export
Git-perf supports CSV export using **long format** where each row represents a single measurement. Units are displayed in a dedicated **unit column** populated from configuration.

**Actual CSV output (long format with unit column):**
```csv
commit	epoch	measurement	timestamp	value	unit	metadata
abc123	0	build_time	1234567890	42.5	ms
abc123	0	memory_usage	1234567890	1048576	bytes
def456	0	build_time	1234567891	43.2	ms
def456	0	memory_usage	1234567891	1048577	bytes
```

**Without units configured:**
```csv
commit	epoch	measurement	timestamp	value	unit	metadata
abc123	0	custom_metric	1234567890	42.5
def456	0	custom_metric	1234567891	43.2
```

**CSV structure:**
- commit: Git commit hash
- epoch: Performance epoch for accepting changes
- measurement: Name of the measurement (e.g., "build_time")
- timestamp: Unix timestamp when measurement was recorded
- value: The measured value
- unit: Configured unit from `.gitperfconfig` (empty if not configured)
- metadata: Key-value pairs (e.g., "group=test")

### 4. No CLI Changes

**No CLI flags added** - units are purely configuration-based.

Users configure units in `.gitperfconfig` and they automatically appear in reports.

**Example workflow:**
```bash
# 1. Configure unit in .gitperfconfig
cat >> .gitperfconfig << EOF
[measurement."build_time"]
unit = "ms"
EOF

# 2. Add measurements as usual (no --unit flag)
git perf add 42.5 -m build_time

# 3. Generate report - unit appears automatically
git perf report -o report.html -m build_time
# Report shows "build_time (ms)" in legend and axis labels
```

### 5. Implementation Phases

#### Phase 1: Configuration Support ✅ COMPLETE (PR #419)
- [x] Add `measurement_unit()` function to `config.rs`
- [x] Add parent fallback support for unit configuration
- [x] Add config tests for unit retrieval
- [x] Update `example_config.toml` with unit examples

**Status:** Merged and deployed

#### Phase 2: Audit Output Integration ✅ COMPLETE (PR #420, #422)
- [x] Update audit output to query units from config
- [x] Format audit results to include units (e.g., "42.5 ms")
- [x] Handle measurements without units (backward compatibility)
- [x] Test audit with various unit configurations
- [x] Display head/tail mean with unit and thousands separators

**Status:** Merged and deployed

#### Phase 3: HTML Report Integration ✅ COMPLETE (PR #423)
- [x] Update report generation to query units from config
- [x] Add units to legend entries (e.g., "build_time (ms)")
- [x] Add units to axis labels when appropriate (when all measurements share same unit)
- [x] Handle mixed measurements (some with units, some without)
- [x] Test report generation with various unit configurations

**Status:** Merged and deployed

#### Phase 4: CSV Export Integration ✅ COMPLETE (PR #425)
- [x] Identify CSV export format (long format: one measurement per row)
- [x] Add CSV header row with unit column
- [x] Implement unit column populated from config
- [x] Refactor CSV serialization to build rows with unit data
- [x] Test CSV export with header and unit column
- [x] Fix slow concurrency test for header line
- [x] Update CSV validation tests for new format
- [x] Document CSV format with unit column support

**Note:** CSV uses long format where each row is a single measurement. Units are displayed in a dedicated unit column populated from configuration. Measurements without configured units will have an empty unit column.

**Status:** Merged and deployed

#### Phase 5: Documentation ⚠️ PARTIALLY COMPLETE
- [x] Update `example_config.toml` with comprehensive unit examples (PR #419)
- [ ] Update README with unit configuration examples
- [ ] Update INTEGRATION_TUTORIAL with unit usage
- [ ] Document unit display behavior in audit and reports
- [ ] Add FAQ about why units aren't stored with measurements

**Status:** Configuration examples complete, user-facing documentation pending

## Testing Strategy

### Unit Tests
- Config loading for unit settings with parent fallback
- Unit retrieval for configured vs unconfigured measurements
- Report display name generation with and without units

### Integration Tests
- Audit command with measurements that have units configured
- Audit command with measurements without units (backward compatibility)
- Report generation with measurements that have units configured
- Report generation with measurements without units
- Mixed scenario: some measurements with units, some without
- CSV export with units (if applicable)

### Manual Testing
- Configure units for different measurements
- Run audit command and verify:
  - Output shows values with units (e.g., "42.5 ms")
  - Measurements without units display correctly
- Generate HTML reports and verify:
  - Legend shows measurement names with units
  - Axis labels include units appropriately
  - Tooltips display values with units
- Verify backward compatibility (no config → no units displayed)

## Example Usage Scenarios

### Scenario 1: Configure and report with units
```toml
# .gitperfconfig
[measurement."build_time"]
unit = "ms"

[measurement."memory_usage"]
unit = "MB"
```

```bash
# Add measurements (no CLI change)
git perf add 42.5 -m build_time
git perf add 256 -m memory_usage

# Generate report
git perf report -o report.html -m build_time -m memory_usage
# Report shows:
# - Legend: "build_time (ms)", "memory_usage (MB)"
# - Tooltips: "42.5 ms", "256 MB"
```

### Scenario 2: Mixed measurements (some with units, some without)
```toml
[measurement."build_time"]
unit = "ms"

# memory_usage has no unit configured
```

```bash
git perf report -o report.html -m build_time -m memory_usage
# Report shows:
# - "build_time (ms)" - with unit
# - "memory_usage" - without unit (as before)
```

### Scenario 3: Default unit for all measurements
```toml
[measurement]
unit = "ms"  # Default for all measurements

[measurement."memory_usage"]
unit = "bytes"  # Override for specific measurement
```

```bash
git perf report -o report.html -m build_time -m memory_usage
# Report shows:
# - "build_time (ms)" - from parent default
# - "memory_usage (bytes)" - from specific override
```

## Backward Compatibility

**Perfect backward compatibility:**
- No changes to measurement data or serialization
- Existing measurements work exactly as before
- Configs without unit settings behave identically to current behavior
- No migration needed
- No new required configuration

If `.gitperfconfig` has no unit settings:
- Reports display measurement names without units (current behavior)
- CSV exports have column headers without units (current behavior)

## Advantages of Config-Only Approach

1. **Simplicity:** No serialization changes, no data model changes
2. **Centralized:** Single source of truth in configuration
3. **Zero risk:** No backward compatibility concerns with data
4. **Fast implementation:** Minimal code changes required
5. **Clean separation:** Configuration vs. data storage concerns

## Limitations of Config-Only Approach

1. **No per-measurement validation:** Can't detect if measurements were recorded in different units
2. **Manual consistency:** Users must ensure measurement names and config units match the actual data
3. **Display only:** Units don't affect audit calculations or comparisons
4. **User responsibility:** Changing unit config doesn't change the actual measurement values - users must ensure config accurately reflects how measurements were recorded

**Mitigation:**
- Document best practices for measurement naming
- Recommend stable unit choices in documentation
- Users should pick appropriate units and stick with them

## Resolved Questions

1. **Should audit command output include units?**
   - **Decision:** Yes, include units in audit output
   - Provides consistency across all output formats
   - Example: `✓ build_time: 42.5 ms (within acceptable range)`

2. **Should we validate that unit config doesn't conflict with measurement name?**
   - **Decision:** No validation
   - Trust users to be consistent
   - Document best practices instead

3. **How should we handle axis labels when measurements have different units?**
   - **Decision:** Follow recommendation
   - Use generic label like "Value" or no unit in axis label when measurements have different units
   - When all measurements share the same unit, include it in axis label
   - Units always appear in legend and tooltips

## Success Criteria

1. ✅ Configuration supports per-measurement unit settings (PR #419)
2. ✅ Audit output displays units with values (PR #420, #422)
3. ✅ HTML reports display units in legends (PR #423)
4. ✅ HTML reports display units in axis labels when appropriate (PR #423)
5. ✅ CSV exports include header row with dedicated unit column (PR #425)
6. ✅ CSV unit column populated from config for each measurement (PR #425)
7. ✅ Existing measurements without unit config continue to work (all PRs)
8. ✅ Configuration documentation includes unit examples (PR #419)
9. ✅ All tests pass including new unit-related tests (all PRs)
10. ✅ Zero changes to data serialization or MeasurementData struct (all PRs)
11. ⚠️ User-facing documentation pending (README, INTEGRATION_TUTORIAL)

**Note:** CSV export uses long format with a dedicated unit column. Units are retrieved from configuration at export time and displayed in the unit column, with empty values for measurements without configured units.

## Future Enhancements (Out of Scope)

- Store units with measurement data (for validation)
- CLI flags for specifying units
- Unit validation (warn if measurements have inconsistent units based on naming)
- Unit conversion (e.g., auto-convert ms to seconds for display)
- Automatic unit inference from measurement names
- Unit normalization (e.g., "ms" vs "milliseconds" vs "msec")

## References

- Issue #330: feat(measurement): allow units in measurements
- Existing configuration system: `git_perf/src/config.rs`
- Report generation: `git_perf/src/reporting.rs`
- Example config: `docs/example_config.toml`

## Appendix: Why Config-Only?

### Comparison: Config-Only vs. Stored Units

| Aspect | Config-Only | Stored with Data |
|--------|-------------|------------------|
| Implementation complexity | Low | Medium-High |
| Serialization changes | None | Required |
| Data model changes | None | Required |
| Backward compatibility | Perfect | Requires careful handling |
| Unit validation | No | Possible |
| Storage overhead | Zero | ~10-20 bytes per measurement |
| Risk level | Minimal | Medium |

### Decision Rationale

**Config-only approach chosen because:**
1. Matches the project's configuration philosophy (other display settings like `dispersion_method` are config-based)
2. Minimal implementation risk and effort
3. Perfect backward compatibility
4. Sufficient for primary use case (clear report display)
5. Can be extended later to store units if validation becomes important

The simplified approach provides 80% of the value with 20% of the complexity.
