use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ApiConfig {
    base_url: String,
    timeout: u32,
}

#[derive(Debug, Deserialize)]
struct SpecialConfig {
    test_value_with_asterisk: String,
    another_asterisk_value: String,
    normal_value: String,
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    database: DatabaseConfig,
    api: ApiConfig,
    special_config: SpecialConfig,
}

// Custom function to implement wildcard-like behavior
fn get_wildcard_values(config: &Config, pattern: &str) -> Result<HashMap<String, serde_json::Value>, ConfigError> {
    // Get the entire config as a HashMap
    let config_map: HashMap<String, serde_json::Value> = config.clone().try_deserialize()?;
    
    // Parse the pattern (e.g., "main.*.value")
    let parts: Vec<&str> = pattern.split('.').collect();
    if parts.len() != 3 || parts[1] != "*" {
        return Err(ConfigError::Message(format!("Invalid wildcard pattern: {}", pattern)));
    }
    
    let section = parts[0];
    let key = parts[2];
    
    // Get the section
    if let Some(section_value) = config_map.get(section) {
        if let Some(section_map) = section_value.as_object() {
            let mut result = HashMap::new();
            
            // Iterate through all keys in the section
            for (sub_key, sub_value) in section_map {
                if let Some(sub_map) = sub_value.as_object() {
                    if let Some(target_value) = sub_map.get(key) {
                        result.insert(sub_key.clone(), target_value.clone());
                    }
                }
            }
            
            return Ok(result);
        }
    }
    
    Ok(HashMap::new())
}

fn main() -> Result<(), ConfigError> {
    // Load configuration from TOML file
    let config = Config::builder()
        .add_source(File::with_name("test-config"))
        .build()?;

    println!("=== Config Crate Asterisk Behavior Demo ===");
    
    // Test 1: Literal asterisk in key names (this works)
    println!("\n1. Testing literal asterisk in key names:");
    let asterisk_value: String = config.get("special_config.test_value_with_asterisk")?;
    let another_asterisk: String = config.get("special_config.another_asterisk_value")?;
    println!("  special_config.test_value_with_asterisk: '{}'", asterisk_value);
    println!("  special_config.another_asterisk_value: '{}'", another_asterisk);

    // Test 2: Direct access to specific values
    println!("\n2. Testing direct access to specific values:");
    let foo_value: String = config.get("main.foo.value")?;
    let bar_value: String = config.get("main.bar.value")?;
    let web_port: i32 = config.get("services.web.port")?;
    println!("  main.foo.value: {}", foo_value);
    println!("  main.bar.value: {}", bar_value);
    println!("  services.web.port: {}", web_port);

    // Test 3: Custom wildcard-like functionality
    println!("\n3. Testing custom wildcard-like functionality:");
    let main_values = get_wildcard_values(&config, "main.*.value")?;
    println!("  main.*.value (custom implementation):");
    for (key, value) in &main_values {
        println!("    {}: {}", key, value);
    }

    let main_priorities = get_wildcard_values(&config, "main.*.priority")?;
    println!("  main.*.priority (custom implementation):");
    for (key, value) in &main_priorities {
        println!("    {}: {}", key, value);
    }

    let service_ports = get_wildcard_values(&config, "services.*.port")?;
    println!("  services.*.port (custom implementation):");
    for (key, value) in &service_ports {
        println!("    {}: {}", key, value);
    }

    // Test 4: Show what happens when we try to use asterisk as wildcard directly
    println!("\n4. Testing direct wildcard usage (this will fail):");
    match config.get::<String>("main.*.value") {
        Ok(_) => println!("  Unexpected: Direct wildcard worked!"),
        Err(e) => println!("  Expected error: {}", e),
    }

    // Test 5: Get entire sections
    println!("\n5. Testing section access:");
    let main_section: HashMap<String, serde_json::Value> = config.get("main")?;
    println!("  main section keys: {:?}", main_section.keys().collect::<Vec<_>>());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_asterisk_in_keys() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test literal asterisk values (not wildcards)
        let asterisk_value: String = config.get("special_config.test_value_with_asterisk")
            .expect("Failed to get asterisk value");
        assert_eq!(asterisk_value, "value*with*asterisk");

        let another_asterisk: String = config.get("special_config.another_asterisk_value")
            .expect("Failed to get another asterisk value");
        assert_eq!(another_asterisk, "config*test*value");
    }

    #[test]
    fn test_direct_value_access() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test direct access to specific values
        let foo_value: String = config.get("main.foo.value")
            .expect("Failed to get main.foo.value");
        assert_eq!(foo_value, "foo_value");

        let bar_priority: i32 = config.get("main.bar.priority")
            .expect("Failed to get main.bar.priority");
        assert_eq!(bar_priority, 2);

        let web_port: i32 = config.get("services.web.port")
            .expect("Failed to get services.web.port");
        assert_eq!(web_port, 8080);
    }

    #[test]
    fn test_custom_wildcard_functionality() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test custom wildcard-like functionality
        let main_values = get_wildcard_values(&config, "main.*.value")
            .expect("Failed to get main.*.value");
        
        assert_eq!(main_values.len(), 4);
        assert_eq!(main_values.get("foo").and_then(|v| v.as_str()), Some("foo_value"));
        assert_eq!(main_values.get("bar").and_then(|v| v.as_str()), Some("bar_value"));
        assert_eq!(main_values.get("baz").and_then(|v| v.as_str()), Some("baz_value"));
        assert_eq!(main_values.get("qux").and_then(|v| v.as_str()), Some("qux_value"));

        let main_priorities = get_wildcard_values(&config, "main.*.priority")
            .expect("Failed to get main.*.priority");
        
        assert_eq!(main_priorities.len(), 4);
        assert_eq!(main_priorities.get("foo").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(main_priorities.get("bar").and_then(|v| v.as_i64()), Some(2));
        assert_eq!(main_priorities.get("baz").and_then(|v| v.as_i64()), Some(3));
        assert_eq!(main_priorities.get("qux").and_then(|v| v.as_i64()), Some(4));

        let service_ports = get_wildcard_values(&config, "services.*.port")
            .expect("Failed to get services.*.port");
        
        assert_eq!(service_ports.len(), 3);
        assert_eq!(service_ports.get("web").and_then(|v| v.as_i64()), Some(8080));
        assert_eq!(service_ports.get("api").and_then(|v| v.as_i64()), Some(3000));
        assert_eq!(service_ports.get("db").and_then(|v| v.as_i64()), Some(5432));
    }

    #[test]
    fn test_direct_wildcard_fails() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test that direct wildcard usage fails
        let result: Result<String, _> = config.get("main.*.value");
        assert!(result.is_err(), "Direct wildcard should fail");
    }

    #[test]
    fn test_config_file_with_asterisk_in_name() {
        // Test that we can load a config file with asterisk in the name
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config file with asterisk in name");

        // Verify we can access values
        let host: String = config.get("database.host")
            .expect("Failed to get database host");
        assert_eq!(host, "localhost");

        // Test literal asterisk access
        let asterisk_value: String = config.get("special_config.test_value_with_asterisk")
            .expect("Failed to get asterisk value");
        assert_eq!(asterisk_value, "value*with*asterisk");
    }
}