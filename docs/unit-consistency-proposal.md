# Unit Consistency Proposal for Import System

## Overview

This document describes the approach taken to maintain unit consistency between imported measurements and configured units in git-perf.

## Design Goals

1. **Skip passing tests** - Don't import test measurements that have duration (only track failures/errors)
2. **Preserve benchmark units** - Don't assume all benchmarks are time-based
3. **Unit consistency** - Validate imported units against config and provide helpful warnings
4. **Store unit metadata** - Allow post-hoc validation and future conversion features

## Implementation

### Test Measurements

**Behavior**: Tests WITH duration are **skipped entirely** (not converted).

**Rationale**:
- We don't want to track performance of passing tests via import
- Only failed/error/skipped tests (typically without duration) are useful for tracking
- Performance tracking should use direct measurement or benchmarks

**For tests without duration**:
- Value: `0.0` (no duration available)
- No unit stored (no meaningful performance data)

### Benchmark Measurements

**Behavior**: Benchmarks preserve their original unit from criterion, normalized to nanoseconds.

**Unit Conversion**:
- All time units are converted to **nanoseconds** (ns)
- Conversion table:
  - `ns` → `ns` (no conversion)
  - `us` / `μs` → `ns` (×1,000)
  - `ms` → `ns` (×1,000,000)
  - `s` → `ns` (×1,000,000,000)
- Unknown units are preserved as-is with a warning

**Rationale**:
- Nanoseconds is the natural unit for high-precision timing
- Matches criterion's default unit
- Consistent with existing git-perf measurement practices
- The existing units module can handle display formatting

### Unit Validation

**Config Integration**:
The converter uses `config::measurement_unit()` to check for configured units.

**Warning Levels**:

1. **Unit Mismatch** (WARN):
   ```
   Unit mismatch for 'bench::my_test::mean': importing 'ns' but config specifies 'ms'.
   Consider updating .gitperfconfig to match.
   ```

2. **No Unit Configured** (INFO):
   ```
   No unit configured for 'bench::my_test::mean'. Importing with unit 'ns'.
   Consider adding to .gitperfconfig: [measurement."bench::my_test::mean"]
   unit = "ns"
   ```

**Non-Blocking**:
- Warnings are logged but **never block** the import
- Users can import first, configure later
- Allows gradual adoption of unit configuration

### Metadata Storage

Every benchmark measurement includes:
```rust
key_values: {
    "type": "bench",
    "group": "my_group",
    "bench_name": "my_bench",
    "statistic": "mean",
    "unit": "ns",  // ← Stored unit
    // ... plus user metadata
}
```

**Benefits**:
- Post-hoc validation possible
- Future unit conversion features enabled
- Clear documentation of measurement units
- Audit trail for debugging

## Configuration Example

Users should configure units in `.gitperfconfig`:

```toml
# Parent table default for all benchmarks
[measurement]
unit = "ns"

# Measurement-specific configuration
[measurement."bench::fibonacci::mean"]
unit = "ns"

[measurement."bench::throughput::mean"]
unit = "requests/sec"  # Non-time unit example
```

## Future Enhancements

1. **Automatic config generation** - Suggest config entries after first import
2. **Unit conversion on read** - Convert stored units to configured display units
3. **Unit validation strictness levels** - Make warnings errors in strict mode
4. **Non-time unit support** - Better handling of throughput, counts, sizes

## Edge Cases

### Unknown Units
- Stored as-is
- Warning logged
- No conversion attempted
- Config validation still performed

### Mismatched Units
- Warning logged
- Import proceeds
- User decides whether to update config or re-import

### Missing Config
- Info message logged
- Suggests config entry
- Import proceeds normally

## Testing

The implementation includes comprehensive tests:
- Test skipping (with/without duration)
- Unit conversion (ns, us, ms, s)
- Metadata preservation
- Mixed test/benchmark imports

All 16 converter tests pass.
