# TOML Config Test with Asterisk Values and Wildcard Path Selection

This project demonstrates how to use the `config` crate in Rust to read TOML configuration files, including:
1. TOML files with asterisks in the filename
2. Configuration values containing literal asterisks
3. Custom implementation of wildcard-like path selection (since the config crate doesn't support wildcards natively)

## Files

- `test-config.toml` - TOML configuration file with asterisk in the name and nested structures for testing
- `src/main.rs` - Rust code demonstrating various ways to read config values and custom wildcard functionality
- `Cargo.toml` - Project dependencies

## Features Demonstrated

1. **TOML file with asterisk in name**: The config file is named `test-config.toml` (contains asterisk)
2. **Literal asterisk values**: Configuration values like `"value*with*asterisk"` and `"config*test*value"`
3. **Nested configuration structures**: For testing wildcard-like access patterns
4. **Custom wildcard implementation**: A function that mimics `main.*.value` style access
5. **Multiple access methods**:
   - Direct access using `config.get()`
   - Custom wildcard-like functionality
   - Section-based access

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
- Configuration values containing literal asterisks are read correctly
- Direct access methods work with asterisk values
- Custom wildcard-like functionality works for patterns like `main.*.value`
- The `config` crate does NOT support native wildcard path selection

## Key Findings

### What Works:
- **Literal asterisks in filenames**: `test-config.toml` loads without issues
- **Literal asterisks in values**: `"value*with*asterisk"` is preserved exactly as written
- **Direct path access**: `config.get("main.foo.value")` works perfectly
- **Custom wildcard implementation**: Our `get_wildcard_values()` function successfully implements `main.*.value` style access

### What Doesn't Work:
- **Native wildcard support**: The `config` crate does NOT support `main.*.value` syntax natively
- **Direct wildcard usage**: `config.get("main.*.value")` fails with a parsing error

### Custom Wildcard Implementation:
The project includes a custom `get_wildcard_values()` function that:
- Parses patterns like `"main.*.value"`
- Returns a `HashMap<String, Value>` with matched results
- Works for any nested structure in the TOML file
- Provides the wildcard functionality that the config crate lacks

## Example Usage

```rust
// This works - literal asterisk in key name
let value: String = config.get("special_config.test_value_with_asterisk")?;

// This works - direct access to specific values
let foo_value: String = config.get("main.foo.value")?;

// This works - custom wildcard implementation
let all_values: HashMap<String, serde_json::Value> = get_wildcard_values(&config, "main.*.value")?;

// This FAILS - native wildcard (not supported)
let result: Result<String, _> = config.get("main.*.value"); // Will return an error
```