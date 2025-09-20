# Testing with Nextest

This project has migrated from serial test execution to [nextest](https://nexte.st/) for faster, parallel test execution.

## Quick Commands for Cursor

### Basic Testing
```bash
# Run all tests with nextest (fast, parallel)
cargo nextest run

# Skip slow tests (recommended for development)
cargo nextest run --skip slow

# Run tests with verbose output for debugging
cargo nextest run --verbose
```

### Specific Test Patterns
```bash
# Run only git integration tests
cargo nextest run --test-pattern "git_interop"

# Run only low-level git tests
cargo nextest run --test-pattern "git_lowlevel"

# Run tests in specific package
cargo nextest run -p git-perf
```

### Legacy Commands (if needed)
```bash
# Standard cargo test (slower, serial execution)
cargo test

# Skip slow tests with cargo
cargo test -- --skip slow
```

## Migration Notes

- **Serial annotations removed**: All `#[serial]` annotations have been removed from tests
- **serial_test dependency removed**: No longer needed since tests run in parallel by default
- **Configuration**: Nextest settings are in `.config/nextest.toml`
- **Performance**: Tests now run in parallel, significantly faster than before
- **Important**: Some tests may fail when run with `cargo test` due to parallel execution conflicts. Use `cargo nextest run` for reliable parallel test execution.

## Troubleshooting

If you encounter test failures:
1. Try running with `--verbose` to see detailed output
2. Check if tests are interfering with each other (should not happen with proper cleanup)
3. Use `cargo test` as fallback if needed

## Environment Setup

Make sure Rust toolchain is in PATH:
```bash
export PATH="/usr/local/cargo/bin:$PATH"
```