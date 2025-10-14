# Plan: Support for Measurement Units

**Related Issue:** #330
**Status:** Planning
**Created:** 2025-10-14

## Overview

Add support for specifying, storing, validating, and displaying units for performance measurements in git-perf. This will improve clarity, enable better analysis, prevent unit confusion, and enhance reporting capabilities.

## Motivation

Currently, git-perf stores numeric measurement values without any associated unit information. Users must rely on naming conventions (e.g., `build_time_ms` vs `build_time_sec`) to track units, which is error-prone and makes it difficult to:

- Understand what a measurement value represents without context
- Prevent mixing measurements with different units
- Provide meaningful axis labels and tooltips in reports
- Perform unit conversions or comparisons
- Validate that measurements are consistent over time

## Goals

1. **Store units with measurements** - Persist unit information alongside measurement data
2. **Validate unit consistency** - Prevent mixing different units for the same measurement
3. **Display units in outputs** - Show units in reports, audit results, and CLI output
4. **Configure default units** - Allow per-measurement unit configuration
5. **Backward compatibility** - Support existing measurements without units (optional field)

## Non-Goals (Future Work)

- Automatic unit conversion (e.g., milliseconds to seconds)
- Complex unit arithmetic (e.g., KB/s to MB/s)
- Support for composite units with automatic parsing (e.g., "requests/second")
- Unit-aware statistical analysis (different analysis based on unit type)

## Design

### 1. Data Model Changes

#### MeasurementData Structure
Add an optional `unit` field to the `MeasurementData` struct:

```rust
// git_perf/src/data.rs
#[derive(Debug, PartialEq)]
pub struct MeasurementData {
    pub epoch: u32,
    pub name: String,
    pub timestamp: f64,
    pub val: f64,
    pub key_values: HashMap<String, String>,
    pub unit: Option<String>,  // NEW: optional unit (e.g., "ms", "bytes", "requests/sec")
}
```

**Rationale:**
- `Option<String>` for backward compatibility (existing measurements without units)
- String type for flexibility (any unit name allowed)
- Simple string comparison for validation (no complex unit parsing initially)

#### Serialization Format
Extend the serialization format to include an optional unit field after key-value pairs:

**Current format:**
```
<epoch><name><timestamp><val>[<key1>=<val1>][<key2>=<val2>]...
```

**New format with optional unit:**
```
<epoch><name><timestamp><val>[<key1>=<val1>][<key2>=<val2>]...[unit=<unit>]
```

**Examples:**
```
0build_time1234567890.042.5unit=ms
0memory_usage1234567890.01048576unit=bytes
0test_count1234567890.0150
```

**Rationale:**
- Uses existing key-value serialization mechanism
- Special key `unit=` for the unit value
- Backward compatible (unit is optional)
- No breaking changes to existing data
- Simple to parse with existing deserializer infrastructure

**Alternative considered:** Add unit as a 5th positional field before key-values
- **Rejected:** Would break existing serialization format and require migration
- Current approach leverages key-value mechanism already in place

### 2. Configuration

Add per-measurement unit configuration in `.gitperfconfig`:

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

**Precedence (same as other config options):**
1. CLI option (if we add `--unit` flag)
2. Measurement-specific config (`[measurement."name"].unit`)
3. Parent default config (`[measurement].unit`)
4. None (no unit)

### 3. CLI Changes

#### Add Command
Add optional `--unit` flag:

```rust
Add {
    /// Measured value to be added
    value: f64,

    #[command(flatten)]
    measurement: CliMeasurement,

    /// Unit of measurement (e.g., "ms", "bytes", "requests/sec")
    #[arg(short = 'u', long)]
    unit: Option<String>,
}
```

**Example usage:**
```bash
git perf add 42.5 -m build_time --unit ms
git perf add 1024 -m memory_usage -u bytes
```

#### Measure Command
Add optional `--unit` flag:

```rust
Measure {
    /// Repetitions
    #[arg(short = 'n', long, value_parser=clap::value_parser!(u16).range(1..), default_value = "1")]
    repetitions: u16,

    #[command(flatten)]
    measurement: CliMeasurement,

    /// Unit of measurement (e.g., "ns", "ms", "seconds")
    #[arg(short = 'u', long)]
    unit: Option<String>,

    /// Command to measure
    #[arg(required(true), last(true))]
    command: Vec<String>,
}
```

**Example usage:**
```bash
git perf measure -m build_time --unit ms -- cargo build
```

**Note:** The `measure` command measures in nanoseconds by default. If no unit is specified, it should use the configured unit or default to "ns".

### 4. Validation

Implement unit consistency validation when adding measurements:

#### Validation Strategy
When adding a new measurement:
1. Retrieve the configured unit for the measurement (if any)
2. Use provided `--unit` flag or fall back to configured unit
3. Check if unit is consistent with historical measurements (optional strict mode)

#### Implementation Location
Add validation in `git_perf/src/measurement_storage.rs`:

```rust
fn validate_unit_consistency(measurement: &str, unit: &Option<String>) -> Result<()> {
    // Implementation:
    // 1. Query recent measurements for this name
    // 2. Check if they have units
    // 3. Warn if units don't match (or error in strict mode)
    Ok(())
}
```

**Validation levels:**
- **Permissive (default):** Warn on unit mismatch but allow
- **Strict (future):** Error on unit mismatch, require explicit override

**Example warning:**
```
Warning: Measurement 'build_time' previously recorded with unit 'ms', but 'seconds' was specified.
```

### 5. Display and Reporting

#### Audit Output
Include units in audit results:

**Current output:**
```
✓ build_time: 42.5 (within acceptable range)
```

**New output:**
```
✓ build_time: 42.5 ms (within acceptable range)
```

#### HTML Report
Update report generation to include units:
- Axis labels with units (e.g., "Build Time (ms)")
- Tooltip hover information showing unit
- Legend entries with units

**Implementation location:** `git_perf/src/reporting.rs`

#### Report Command
No CLI changes needed for the `report` command initially. Units will be automatically extracted from measurement data and displayed in the generated HTML.

### 6. Backward Compatibility

**Key principles:**
- Unit field is optional (`Option<String>`)
- Existing measurements without units continue to work
- No migration required for existing data
- New measurements can specify units, old measurements return `None`

**Deserialization behavior:**
- If `unit=<value>` is present → parse and set `unit: Some(value)`
- If `unit=` is not present → set `unit: None`
- Existing measurements → `unit: None`

**Display behavior:**
- If unit is `Some(u)` → display value with unit (e.g., "42.5 ms")
- If unit is `None` → display value only (e.g., "42.5")

### 7. Implementation Phases

#### Phase 1: Core Data Model (Essential)
- [ ] Add `unit: Option<String>` field to `MeasurementData`
- [ ] Update serialization to handle `unit=` key-value pair
- [ ] Update deserialization to parse `unit=` key-value pair
- [ ] Add unit tests for serialization/deserialization roundtrip
- [ ] Update affected tests to handle optional unit field

**Estimated effort:** Medium (1-2 days)

#### Phase 2: Configuration Support (Essential)
- [ ] Add `measurement_unit()` function to config module
- [ ] Add parent fallback support for unit configuration
- [ ] Update example config with unit examples
- [ ] Add config tests for unit retrieval

**Estimated effort:** Small (0.5-1 day)

#### Phase 3: CLI Integration (Essential)
- [ ] Add `--unit` flag to `add` command
- [ ] Add `--unit` flag to `measure` command
- [ ] Update CLI argument parsing to accept unit
- [ ] Pass unit through to measurement storage
- [ ] Update CLI help documentation

**Estimated effort:** Medium (1-2 days)

#### Phase 4: Validation (Important but not blocking)
- [ ] Implement unit consistency checking
- [ ] Add warnings for unit mismatches
- [ ] Make validation configurable (warn vs error)
- [ ] Add validation tests

**Estimated effort:** Medium (1-2 days)

#### Phase 5: Display and Output (Important)
- [ ] Update audit output to show units
- [ ] Update HTML report to show units in axis labels
- [ ] Update tooltips to include units
- [ ] Ensure backward compatibility for measurements without units

**Estimated effort:** Medium (1-2 days)

#### Phase 6: Documentation (Essential)
- [ ] Update README with unit examples
- [ ] Update INTEGRATION_TUTORIAL with unit usage
- [ ] Regenerate manpages with `--unit` flag documentation
- [ ] Add unit examples to example_config.toml
- [ ] Document unit validation behavior

**Estimated effort:** Small (0.5-1 day)

## Testing Strategy

### Unit Tests
- Serialization/deserialization with and without units
- Config loading for unit settings
- Unit validation logic
- Backward compatibility (measurements without units)

### Integration Tests
- Add measurement with unit via CLI
- Measure command with unit
- Report generation with mixed unit/non-unit measurements
- Audit with unit display

### Test Data Compatibility
- Ensure existing test data (without units) still works
- Add new test data with units
- Test migration scenarios (adding units to existing measurements)

## Migration Path

**No migration required** - This is a non-breaking, additive change.

Users can:
1. Continue using git-perf without units (backward compatible)
2. Start adding units to new measurements immediately
3. Gradually add units to configurations for existing measurements
4. Mix measurements with and without units in the same repository

## Example Usage Scenarios

### Scenario 1: Simple measurement with unit
```bash
# Add measurement with explicit unit
git perf add 42.5 -m build_time --unit ms

# Audit will display: "build_time: 42.5 ms"
git perf audit -m build_time
```

### Scenario 2: Measure command with configured unit
```toml
# .gitperfconfig
[measurement."compile_time"]
unit = "seconds"
```

```bash
# Unit from config is automatically used
git perf measure -m compile_time -- cargo build
```

### Scenario 3: Mixed units (validation warning)
```bash
# First measurement
git perf add 1000 -m response_time --unit ms

# Later measurement with different unit (triggers warning)
git perf add 1.5 -m response_time --unit seconds
# Warning: Measurement 'response_time' previously recorded with unit 'ms'
```

### Scenario 4: Report with units
```bash
git perf report -o report.html -m build_time
# Generated report shows "Build Time (ms)" on Y-axis
# Tooltips show "42.5 ms" on hover
```

## Open Questions

1. **Should we support a predefined set of common units (enum)?**
   - **Recommendation:** Start with free-form strings for flexibility
   - Future: Add optional validation against common units list

2. **Should we implement unit conversion in the initial release?**
   - **Recommendation:** No, keep it simple. Add in future if needed.
   - Conversion is complex (ms→s, KB→MB, etc.) and can be error-prone

3. **How strict should unit validation be by default?**
   - **Recommendation:** Permissive (warnings) by default
   - Add strict mode later if users request it

4. **Should the `measure` command override the hardcoded nanosecond measurement?**
   - **Recommendation:** No. Measure always captures in nanoseconds (internal)
   - But allow storing with a different display unit (e.g., "ms")
   - Document that measure's `--unit` is for display/storage, not measurement collection

5. **Should we allow units in key-values vs. top-level field?**
   - **Decision:** Top-level field using special `unit=` key-value
   - **Rationale:** Leverages existing serialization, backward compatible

## Risks and Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Serialization format breaking | High | Use key-value mechanism, make unit optional |
| Unit confusion (mixing units) | Medium | Add validation and warnings |
| Performance impact of validation | Low | Make validation lightweight, cache results |
| Increased data storage size | Low | Units are short strings, minimal impact |
| Backward compatibility issues | High | Extensive testing with existing data |

## Success Criteria

1. ✅ Users can specify units when adding measurements
2. ✅ Units are stored persistently with measurements
3. ✅ Units are displayed in audit output and reports
4. ✅ Configuration supports per-measurement default units
5. ✅ Existing measurements without units continue to work
6. ✅ Documentation includes unit usage examples
7. ✅ All tests pass including new unit-related tests

## Future Enhancements (Out of Scope)

- Unit conversion (e.g., ms ↔ seconds, KB ↔ MB)
- Complex unit types (e.g., dimensional analysis)
- Unit-aware aggregation (group by unit)
- Unit validation against predefined list
- Strict mode for unit consistency enforcement
- Auto-detection of units from measurement names
- Unit normalization (e.g., "ms" vs "milliseconds" vs "msec")

## References

- Issue #330: feat(measurement): allow units in measurements
- Existing configuration system: `git_perf/src/config.rs`
- Serialization format: `git_perf/src/serialization.rs`
- Measurement data model: `git_perf/src/data.rs`
- CLI types: `cli_types/src/lib.rs`

## Appendix: Alternative Designs Considered

### Alternative 1: Add unit as 5th positional field
**Format:** `<epoch><name><timestamp><val><unit>[<key1>=<val1>]...`

**Pros:**
- Clearer separation from key-values
- Slightly more efficient to parse

**Cons:**
- Breaking change to serialization format
- Requires migration of existing data
- More complex backward compatibility

**Decision:** Rejected due to breaking changes

### Alternative 2: Store unit in key-values only (no top-level field)
**Approach:** Use `unit` as a reserved key in key-values

**Pros:**
- No changes to MeasurementData struct
- Already serializable

**Cons:**
- Unit is special, not just another key-value
- More awkward to access (need to query HashMap)
- Harder to type-check and validate

**Decision:** Rejected - unit deserves top-level field, but we use key-value for serialization

### Alternative 3: Separate unit metadata table
**Approach:** Store units separately in config or separate notes

**Pros:**
- No changes to measurement storage
- Centralized unit management

**Cons:**
- Units separated from measurements
- More complex lookups
- Harder to ensure consistency

**Decision:** Rejected - unit should be stored with measurement data
