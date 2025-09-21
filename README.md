# TOML Config Test with Asterisk Values

This project demonstrates how to use the `config` crate in Rust to read TOML configuration files that contain asterisks (`*`) in both the filename and configuration values.

## Files

- `test-config.toml` - TOML configuration file with asterisk in the name and config values containing asterisks
- `src/main.rs` - Rust code demonstrating various ways to read config values with asterisks
- `Cargo.toml` - Project dependencies

## Features Demonstrated

1. **TOML file with asterisk in name**: The config file is named `test-config.toml` (contains asterisk)
2. **Config values with asterisks**: Configuration values like `"value*with*asterisk"` and `"config*test*value"`
3. **Multiple access methods**:
   - Deserializing to structs
   - Direct access using `config.get()`
   - HashMap-based dynamic access

## Running the Tests

```bash
cargo test
```

## Running the Example

```bash
cargo run
```

## Test Results

The tests verify that:
- TOML files with asterisks in the filename can be loaded successfully
- Configuration values containing asterisks are read correctly
- Both struct deserialization and direct access methods work with asterisk values
- The `config` crate handles asterisks in configuration keys and values without issues

## Key Findings

- The `config` crate handles asterisks in TOML filenames without problems
- Configuration values with asterisks are preserved exactly as written
- All access methods (struct deserialization, direct access, HashMap) work correctly with asterisk values
- No special escaping or handling is required for asterisks in TOML values