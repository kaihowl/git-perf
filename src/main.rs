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

fn main() -> Result<(), ConfigError> {
    // Load configuration from TOML file
    let config = Config::builder()
        .add_source(File::with_name("test-config"))
        .build()?;

    println!("=== Literal Asterisk Path Selection Test ===");
    
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

    // Test 3: Access literal asterisk keys through HashMap approach
    println!("\n3. Testing literal asterisk as key name through HashMap access:");
    
    // Get the main section as a HashMap
    let main_section: HashMap<String, serde_json::Value> = config.get("main")?;
    println!("  main section keys: {:?}", main_section.keys().collect::<Vec<_>>());
    
    // Access the asterisk key from the HashMap
    if let Some(asterisk_entry) = main_section.get("*") {
        if let Some(asterisk_obj) = asterisk_entry.as_object() {
            if let Some(value) = asterisk_obj.get("value") {
                println!("  main[*].value: {}", value);
            }
            if let Some(priority) = asterisk_obj.get("priority") {
                println!("  main[*].priority: {}", priority);
            }
        }
    }
    
    // Get the services section as a HashMap
    let services_section: HashMap<String, serde_json::Value> = config.get("services")?;
    println!("  services section keys: {:?}", services_section.keys().collect::<Vec<_>>());
    
    // Access the asterisk key from the services HashMap
    if let Some(asterisk_entry) = services_section.get("*") {
        if let Some(asterisk_obj) = asterisk_entry.as_object() {
            if let Some(port) = asterisk_obj.get("port") {
                println!("  services[*].port: {}", port);
            }
            if let Some(host) = asterisk_obj.get("host") {
                println!("  services[*].host: {}", host);
            }
        }
    }

    // Test 4: Show what happens when we try to use asterisk without quotes
    println!("\n4. Testing asterisk without quotes (this will fail):");
    match config.get::<String>("main.*.value") {
        Ok(value) => println!("  Unexpected: main.*.value worked and returned: {}", value),
        Err(e) => println!("  Expected error: {}", e),
    }

    // Test 5: Try different quote syntaxes
    println!("\n5. Testing different quote syntaxes:");
    let quote_tests = vec![
        "main.\"*\".value",
        "main.'*'.value", 
        "main.\\*.value",
        "main[*].value",
    ];
    
    for test_path in quote_tests {
        match config.get::<String>(test_path) {
            Ok(value) => println!("  {}: SUCCESS - {}", test_path, value),
            Err(e) => println!("  {}: FAILED - {}", test_path, e),
        }
    }

    // Test 6: Demonstrate the working approach
    println!("\n6. Working approach summary:");
    println!("  - Direct path access works for normal keys: main.foo.value");
    println!("  - HashMap access works for special keys: main[*].value");
    println!("  - The config crate treats asterisk as a literal character in keys");
    println!("  - Special characters in keys require HashMap-based access");

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
    fn test_literal_asterisk_as_key_name_via_hashmap() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test literal asterisk as a key name through HashMap access
        let main_section: HashMap<String, serde_json::Value> = config.get("main")
            .expect("Failed to get main section");
        
        // Verify the asterisk key exists
        assert!(main_section.contains_key("*"), "main section should contain '*' key");
        
        // Access the asterisk key's value
        if let Some(asterisk_entry) = main_section.get("*") {
            if let Some(asterisk_obj) = asterisk_entry.as_object() {
                if let Some(value) = asterisk_obj.get("value") {
                    assert_eq!(value.as_str(), Some("asterisk_value"));
                }
                if let Some(priority) = asterisk_obj.get("priority") {
                    assert_eq!(priority.as_i64(), Some(5));
                }
            }
        }

        // Test services section
        let services_section: HashMap<String, serde_json::Value> = config.get("services")
            .expect("Failed to get services section");
        
        assert!(services_section.contains_key("*"), "services section should contain '*' key");
        
        if let Some(asterisk_entry) = services_section.get("*") {
            if let Some(asterisk_obj) = asterisk_entry.as_object() {
                if let Some(port) = asterisk_obj.get("port") {
                    assert_eq!(port.as_i64(), Some(9999));
                }
                if let Some(host) = asterisk_obj.get("host") {
                    assert_eq!(host.as_str(), Some("asterisk_host"));
                }
            }
        }
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
    fn test_asterisk_without_quotes_fails() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test that asterisk without quotes fails
        let result: Result<String, _> = config.get("main.*.value");
        assert!(result.is_err(), "main.*.value should fail without quotes");
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

    #[test]
    fn test_section_keys_include_asterisk() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test that sections contain the literal asterisk key
        let main_section: HashMap<String, serde_json::Value> = config.get("main")
            .expect("Failed to get main section");
        
        assert!(main_section.contains_key("*"), "main section should contain '*' key");
        
        let services_section: HashMap<String, serde_json::Value> = config.get("services")
            .expect("Failed to get services section");
        
        assert!(services_section.contains_key("*"), "services section should contain '*' key");
    }
}