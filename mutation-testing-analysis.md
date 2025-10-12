# Mutation Testing Analysis - Week of October 12, 2025

## Summary

**Mutation Score: 95.29%** (304 caught / 319 viable mutants)

- Total mutants tested: 342
- Caught: 304
- Missed: 15
- Unviable: 23
- Timeout: 0

## Files with Missed Mutants

| File | Missed Mutants | Priority |
|------|----------------|----------|
| `git_perf/src/git/git_interop.rs` | 5 | **HIGH** |
| `git_perf/src/reporting.rs` | 3 | Medium |
| `git_perf/src/audit.rs` | 3 | Medium |
| `git_perf/src/git/git_lowlevel.rs` | 2 | Low |
| `git_perf/src/measurement_storage.rs` | 1 | Low |
| `git_perf/src/serialization.rs` | 1 | Low |

---

## Detailed Analysis

### 1. git_perf/src/git/git_interop.rs (5 missed mutants) ⚠️ PRIORITY

This file has the most missed mutants and should be the primary focus for test improvements.

#### Mutant 1 & 2: `new_symbolic_write_ref()` - Function Return Value (Lines 430)
**Issue**: Function can return empty string or arbitrary string without tests detecting it
- **Original**: Returns result of `create_temp_ref_name()` and git operations
- **Missed mutations**:
  - `Ok(String::new())` - returning empty string
  - `Ok("xyzzy".into())` - returning arbitrary string

**Root Cause**: No tests verify that `new_symbolic_write_ref()` returns a valid, non-empty reference name or that the returned reference actually exists in the git repository.

**Test Improvement Suggestions**:
```rust
#[test]
fn test_new_symbolic_write_ref_returns_valid_ref() {
    // Setup test repo
    let result = new_symbolic_write_ref().unwrap();

    // Verify result is not empty
    assert!(!result.is_empty(), "Reference name should not be empty");

    // Verify result has correct prefix
    assert!(result.starts_with(REFS_NOTES_WRITE_TARGET_PREFIX));

    // Verify the reference actually exists in git
    let ref_exists = git_show_ref(&result).is_ok();
    assert!(ref_exists, "Created reference should exist");
}

#[test]
fn test_symbolic_ref_points_to_created_ref() {
    let target = new_symbolic_write_ref().unwrap();

    // Verify symbolic ref points to the target
    let symbolic_ref_target = git_symbolic_ref_read(REFS_NOTES_WRITE_SYMBOLIC_REF).unwrap();
    assert_eq!(symbolic_ref_target, target);
}
```

#### Mutant 3: `default_backoff()` - Configuration Function (Line 59)
**Issue**: Function can return `Default::default()` instead of configured backoff
- **Original**: Builds `ExponentialBackoff` with custom `max_elapsed_time`
- **Missed mutation**: `Default::default()` - using default backoff settings

**Root Cause**: No tests verify that backoff configuration is actually applied. The default backoff settings might coincidentally work, masking the configuration bug.

**Test Improvement Suggestions**:
```rust
#[test]
fn test_default_backoff_uses_configured_max_elapsed() {
    // Set a specific backoff config value
    let expected_max_elapsed = config::backoff_max_elapsed_seconds();

    let backoff = default_backoff();

    // Extract and verify max_elapsed_time is set correctly
    // Note: This may require exposing backoff settings or using a trait
    assert_eq!(backoff.max_elapsed_time, Some(Duration::from_secs(expected_max_elapsed)));
}

#[test]
fn test_backoff_actually_retries_with_config() {
    // Test that operations with backoff actually retry
    // and respect the configured timeout
}
```

#### Mutant 4: `git_push_notes_ref()` - Unary Operator (Line 569)
**Issue**: Removing `!` negation operator doesn't cause test failure
- **Original**: `!l.starts_with('!')`
- **Missed mutation**: `l.starts_with('!')` (removed negation)

**Root Cause**: Tests don't cover the scenario where git push output contains lines starting with '!' (which indicates push failures/rejections).

**Test Improvement Suggestions**:
```rust
#[test]
fn test_git_push_notes_ref_detects_failed_push() {
    // Mock or create scenario where git push returns failure lines starting with '!'
    // Example output: "! refs/notes/commits:refs/notes/commits [rejected]"

    let result = git_push_notes_ref("origin");

    assert!(result.is_err(), "Should fail when push is rejected");
    assert!(matches!(result.unwrap_err(), GitError::RefFailedToPush { .. }));
}

#[test]
fn test_git_push_notes_ref_succeeds_with_ok_output() {
    // Test successful push that contains the ref without '!' prefix
    // Example output: "refs/notes/commits:refs/notes/commits abc123..def456"
}
```

#### Mutant 5: `walk_commits()` - Binary Operator (Line 725)
**Issue**: Changing `|=` to `^=` doesn't cause test failure
- **Original**: `detected_shallow |= info[2..].contains(&"grafted")`
- **Missed mutation**: `detected_shallow ^= info[2..].contains(&"grafted")` (XOR instead of OR)

**Root Cause**: No tests verify shallow/grafted repository detection, or tests only check repos with single grafted commit (where `|=` and `^=` produce same result).

**Test Improvement Suggestions**:
```rust
#[test]
fn test_walk_commits_detects_shallow_repo() {
    // Create a shallow cloned repo
    let (commits, is_shallow) = walk_commits("HEAD", &repo_path).unwrap();

    assert!(is_shallow, "Should detect shallow repository");
}

#[test]
fn test_walk_commits_detects_multiple_grafted_commits() {
    // Create repo with multiple grafted commits
    // This is the key test that would catch the ^= mutation
    // because XOR would toggle the flag instead of OR-ing it
    let (commits, is_shallow) = walk_commits("HEAD", &repo_path).unwrap();

    assert!(is_shallow);
}

#[test]
fn test_walk_commits_normal_repo_not_shallow() {
    // Full clone - should not be detected as shallow
    let (commits, is_shallow) = walk_commits("HEAD", &repo_path).unwrap();

    assert!(!is_shallow, "Full repo should not be shallow");
}
```

---

### 2. git_perf/src/reporting.rs (3 missed mutants)

#### Mutant 1: `report()` - Match Arm Guard (Line 358)
**Issue**: Match guard condition can be replaced with `true`
- **Original**: Complex condition in match guard
- **Missed mutation**: `true` (always matches)

**Root Cause**: Test doesn't verify the specific conditional logic in the match guard.

**Test Improvement**: Add test that verifies the match guard condition is evaluated correctly by testing both branches (when condition is true vs false).

#### Mutant 2: `report()` - Binary Operator (Line 329)
**Issue**: Changing `==` to `!=` doesn't fail tests
- **Original**: `==` comparison
- **Missed mutation**: `!=`

**Root Cause**: Missing test coverage for the specific equality check at line 329.

**Test Improvement**: Add test that exercises both sides of this comparison to ensure correct operator is used.

#### Mutant 3: `add_summarized_trace()` - Function Value (Line 123)
**Issue**: Function body can be replaced with `()` (empty/no-op)
- **Original**: Adds trace data to plot
- **Missed mutation**: `()` (does nothing)

**Root Cause**: No tests verify that `add_summarized_trace()` actually adds trace data to the output.

**Test Improvement**:
```rust
#[test]
fn test_add_summarized_trace_adds_trace_to_output() {
    let mut reporter = PlotlyReporter::new();

    reporter.add_summarized_trace(/* params */);

    // Verify trace was actually added
    assert!(!reporter.traces.is_empty(), "Should add trace");

    // Verify trace contains expected data
    let trace = &reporter.traces[0];
    assert!(trace.x.len() > 0);
    assert!(trace.y.len() > 0);
}
```

---

### 3. git_perf/src/audit.rs (3 missed mutants)

#### Mutant 1: Line 174 - Binary Operator
**Issue**: Changing `*` to `/` in calculation doesn't fail tests
- **Original**: `(head / tail_median - 1.0).abs() * 100.0` (multiply by 100)
- **Missed mutation**: `(head / tail_median - 1.0).abs() / 100.0` (divide by 100)

**Root Cause**: Tests don't verify the exact percentage calculation. A mutation changing multiplication to division by 100 would drastically change the deviation value (off by 10,000x), but tests may not be asserting specific values.

**Test Improvement**:
```rust
#[test]
fn test_relative_deviation_calculation() {
    // Test with known values
    let head = 110.0;
    let tail_median = 100.0;

    // Expected: (110/100 - 1.0).abs() * 100.0 = 10.0%
    let result = audit_with_data(/* setup with these values */);

    // Assert the exact percentage
    assert_eq!(result.relative_deviation, 10.0);
}
```

#### Mutant 2: Line 206 - Binary Operator
**Issue**: Changing `&&` to `||` doesn't fail tests
- **Original**: `!z_score_exceeds_sigma || passed_due_to_threshold`
- **Missed mutation**: `!z_score_exceeds_sigma && passed_due_to_threshold` (changed || to &&)

**Wait, the mutation report says replacement is `||` but original should have `&&`**

Actually looking at the code, line 206 shows:
```rust
let passed = !z_score_exceeds_sigma || passed_due_to_threshold;
```

The mutation changed `&&` to `||` somewhere, but the current code has `||`. This suggests the test doesn't distinguish between these logical operators.

**Root Cause**: Missing test cases for the four combinations of the boolean conditions.

**Test Improvement**:
```rust
#[test]
fn test_audit_pass_conditions() {
    // Case 1: z_score ok, threshold not applicable -> PASS
    // Case 2: z_score bad, threshold saves it -> PASS
    // Case 3: z_score bad, threshold not met -> FAIL
    // Case 4: z_score ok, threshold applicable but not needed -> PASS
}
```

#### Mutant 3: Line 182 - Binary Operator
**Issue**: Changing `<` to `<=` doesn't fail tests
- **Original**: `head_relative_deviation < threshold`
- **Missed mutation**: `head_relative_deviation <= threshold`

**Root Cause**: No test with exact boundary value (deviation exactly equals threshold).

**Test Improvement**:
```rust
#[test]
fn test_threshold_boundary_condition() {
    // Set threshold to exactly 5.0
    // Set deviation to exactly 5.0
    // Should FAIL with < (5 < 5 is false)
    // Would PASS with <= (5 <= 5 is true)

    let result = audit_with_threshold(5.0, 5.0);
    assert!(!result.passed, "Exact threshold match should fail");
}
```

---

### 4. git_perf/src/git/git_lowlevel.rs (2 missed mutants)

Both mutants are in `concat_version()` at line 197:
- Can return `"xyzzy".into()`
- Can return `String::new()`

**Root Cause**: No tests verify that `concat_version()` returns a properly formatted version string.

**Test Improvement**:
```rust
#[test]
fn test_concat_version_format() {
    let version = concat_version(2, 45, 1);

    assert_eq!(version, "2.45.1");
    assert!(!version.is_empty());
    assert!(version.chars().any(|c| c.is_numeric()));
}
```

---

### 5. git_perf/src/measurement_storage.rs (1 missed mutant)

#### `add_multiple()` - Line 22
**Issue**: Function body can be replaced with `Ok(())` (no-op)

**Root Cause**: No tests verify that `add_multiple()` actually persists the measurements.

**Test Improvement**:
```rust
#[test]
fn test_add_multiple_persists_data() {
    let storage = MeasurementStorage::new();

    storage.add_multiple(&measurements).unwrap();

    // Verify data was actually written
    let retrieved = storage.get_all().unwrap();
    assert_eq!(retrieved.len(), measurements.len());
    assert_eq!(retrieved, measurements);
}
```

---

### 6. git_perf/src/serialization.rs (1 missed mutant)

#### `deserialize_single()` - Line 84
**Issue**: Changing `<` to `>=` doesn't fail tests
- **Original**: Some comparison with `<`
- **Missed mutation**: `>=`

**Root Cause**: Missing boundary/edge case test for deserialization.

**Test Improvement**: Add tests with boundary values that would behave differently with `<` vs `>=`.

---

## Recommendations

### Immediate Actions (High Priority)

1. **Focus on `git_perf/src/git/git_interop.rs`** - This file has 5 missed mutants and appears to handle critical git operations
   - Add tests for `new_symbolic_write_ref()` return value validation
   - Add tests for git push failure scenarios (lines starting with '!')
   - Add tests for shallow/grafted repository detection with multiple grafted commits
   - Add tests for backoff configuration

### General Testing Improvements

2. **Add boundary value tests**
   - Test exact equality conditions (audit threshold at line 182)
   - Test edge cases in comparisons

3. **Add integration tests for side effects**
   - Functions that return `Ok(())` or `()` need tests that verify state changes
   - Verify `add_multiple()` actually persists data
   - Verify `add_summarized_trace()` actually adds traces

4. **Test error paths and failure scenarios**
   - Git push rejections
   - Invalid/empty return values

5. **Test logical operator combinations**
   - All four combinations of boolean conditions in `audit.rs` line 206

### Long-term Improvements

6. **Property-based testing** - Consider using `proptest` or `quickcheck` for:
   - Version string formatting
   - Calculation accuracy (relative deviation)

7. **Add coverage-guided mutation testing** - Focus mutation testing on high-risk code paths

8. **Consider test quality metrics** - Track not just code coverage but mutation score over time

## Test Implementation Priority

**Phase 1** (This PR):
- [ ] `git_interop.rs`: Add tests for `new_symbolic_write_ref()`
- [ ] `git_interop.rs`: Add tests for git push failure detection
- [ ] `git_interop.rs`: Add shallow repo detection tests
- [ ] `audit.rs`: Add boundary value test for threshold comparison

**Phase 2** (Follow-up):
- [ ] `measurement_storage.rs`: Add persistence verification test
- [ ] `reporting.rs`: Add trace addition verification test
- [ ] `git_lowlevel.rs`: Add version format validation test
- [ ] `serialization.rs`: Add boundary value tests

**Phase 3** (Future):
- [ ] Comprehensive logical operator testing in `audit.rs`
- [ ] Backoff configuration validation
- [ ] Property-based testing for calculations
