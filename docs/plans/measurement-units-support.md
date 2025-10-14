# Plan: Support for Measurement Units

**Related Issue:** #330
**Status:** Planning
**Created:** 2025-10-14
**Updated:** 2025-10-14 - Simplified to config-only approach

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
4. **No data model changes** - Keep measurements storage simple and unchanged
5. **Backward compatibility** - Existing measurements and configs work without modification

## Non-Goals (Future Work)

- Storing units with measurement data
- Runtime unit validation or consistency checking
- Automatic unit conversion (e.g., milliseconds to seconds)
- CLI flags for specifying units (config-only)
- Unit display in audit command output

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

#### HTML Report
Update report generation to include units in `git_perf/src/reporting.rs`:

**Changes needed:**
- **Legend entries:** Append unit to measurement name (e.g., "build_time (ms)")
- **Axis labels:** Include unit in Y-axis label (e.g., "Performance (ms)")
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
If git-perf has CSV export functionality, include units in column headers:

**Example CSV output:**
```csv
commit,build_time (ms),memory_usage (bytes),timestamp
abc123,42.5,1048576,1234567890
def456,43.2,1048577,1234567891
```

**Without unit configured:**
```csv
commit,build_time,memory_usage,timestamp
abc123,42.5,1048576,1234567890
```

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

#### Phase 1: Configuration Support (Essential)
- [ ] Add `measurement_unit()` function to `config.rs`
- [ ] Add parent fallback support for unit configuration
- [ ] Add config tests for unit retrieval
- [ ] Update `example_config.toml` with unit examples

**Estimated effort:** Small (0.5 day)

#### Phase 2: HTML Report Integration (Essential)
- [ ] Update report generation to query units from config
- [ ] Add units to legend entries (e.g., "build_time (ms)")
- [ ] Add units to axis labels if all measurements share same unit
- [ ] Add units to hover tooltips
- [ ] Handle mixed measurements (some with units, some without)
- [ ] Test report generation with various unit configurations

**Estimated effort:** Medium (1-2 days)

#### Phase 3: CSV Export Integration (If applicable)
- [ ] Identify if CSV export exists in codebase
- [ ] Add units to CSV column headers
- [ ] Test CSV export with unit configurations

**Estimated effort:** Small (0.5 day)

#### Phase 4: Documentation (Essential)
- [ ] Update README with unit configuration examples
- [ ] Update INTEGRATION_TUTORIAL with unit usage
- [ ] Update `example_config.toml` with comprehensive unit examples
- [ ] Document unit display behavior in reports
- [ ] Add FAQ about why units aren't stored with measurements

**Estimated effort:** Small (0.5-1 day)

## Testing Strategy

### Unit Tests
- Config loading for unit settings with parent fallback
- Unit retrieval for configured vs unconfigured measurements
- Report display name generation with and without units

### Integration Tests
- Report generation with measurements that have units configured
- Report generation with measurements without units
- Mixed scenario: some measurements with units, some without
- CSV export with units (if applicable)

### Manual Testing
- Configure units for different measurements
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

### Scenario 2: Change unit without re-recording
```toml
# Initially configured as milliseconds
[measurement."response_time"]
unit = "ms"
```

```bash
# Generate report - shows "ms"
git perf report -o report1.html -m response_time
```

```toml
# Later, change to seconds for better readability
[measurement."response_time"]
unit = "seconds"
```

```bash
# Generate report - now shows "seconds"
# No need to re-record measurements!
git perf report -o report2.html -m response_time
```

### Scenario 3: Mixed measurements (some with units, some without)
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

### Scenario 4: Default unit for all measurements
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
2. **Flexibility:** Can change units without re-recording measurements
3. **Centralized:** Single source of truth in configuration
4. **Zero risk:** No backward compatibility concerns with data
5. **Fast implementation:** Minimal code changes required
6. **Clean separation:** Configuration vs. data storage concerns

## Limitations of Config-Only Approach

1. **No per-measurement validation:** Can't detect if measurements were recorded in different units
2. **Manual consistency:** Users must ensure measurement names reflect units (e.g., don't call it `time_ms` if config says `seconds`)
3. **No historical tracking:** Can't see if units changed over time
4. **Display only:** Units don't affect audit calculations or comparisons

**Mitigation:**
- Document best practices for measurement naming
- Recommend stable unit choices in documentation
- Users should pick appropriate units and stick with them

## Open Questions

1. **Should audit command output include units?**
   - **Recommendation:** No, not in initial implementation (keep scope small)
   - Future enhancement if users request it

2. **Should we validate that unit config doesn't conflict with measurement name?**
   - **Recommendation:** No, trust users to be consistent
   - Document best practices instead

3. **How should we handle axis labels when measurements have different units?**
   - **Recommendation:** Generic label like "Value" or no unit in axis label
   - Units still appear in legend and tooltips

## Success Criteria

1. ✅ Configuration supports per-measurement unit settings
2. ✅ HTML reports display units in legends
3. ✅ HTML reports display units in tooltips
4. ✅ CSV exports include units in column headers (if CSV export exists)
5. ✅ Existing measurements without unit config continue to work
6. ✅ Documentation includes unit configuration examples
7. ✅ All tests pass including new unit-related tests
8. ✅ Zero changes to data serialization or MeasurementData struct

## Future Enhancements (Out of Scope)

- Store units with measurement data (for validation)
- CLI flags for specifying units
- Unit display in audit command output
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
| Change units retroactively | Yes | No |
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
