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

    // Deserialize the entire config
    let app_config: AppConfig = config.try_deserialize()?;

    println!("=== Configuration loaded successfully ===");
    println!("Database config: {:?}", app_config.database);
    println!("API config: {:?}", app_config.api);
    println!("Special config: {:?}", app_config.special_config);

    // Test accessing specific values with asterisks
    println!("\n=== Testing asterisk values ===");
    println!("Value with asterisk: '{}'", app_config.special_config.test_value_with_asterisk);
    println!("Another asterisk value: '{}'", app_config.special_config.another_asterisk_value);
    println!("Normal value: '{}'", app_config.special_config.normal_value);

    // Test direct access using config.get()
    let config2 = Config::builder()
        .add_source(File::with_name("test-config"))
        .build()?;

    let asterisk_value: String = config2.get("special_config.test_value_with_asterisk")?;
    let another_asterisk: String = config2.get("special_config.another_asterisk_value")?;

    println!("\n=== Direct access test ===");
    println!("Direct access to 'special_config.test_value_with_asterisk': '{}'", asterisk_value);
    println!("Direct access to 'special_config.another_asterisk_value': '{}'", another_asterisk);

    // Test with HashMap approach for dynamic access
    let config_map: HashMap<String, serde_json::Value> = config2.try_deserialize()?;
    println!("\n=== HashMap access test ===");
    if let Some(special_config) = config_map.get("special_config") {
        if let Some(asterisk_val) = special_config.get("test_value_with_asterisk") {
            println!("HashMap access to asterisk value: '{}'", asterisk_val);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_with_asterisk_values() {
        let config = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        // Test deserializing the full config
        let app_config: AppConfig = config.try_deserialize()
            .expect("Failed to deserialize config");

        // Verify asterisk values are correctly loaded
        assert_eq!(app_config.special_config.test_value_with_asterisk, "value*with*asterisk");
        assert_eq!(app_config.special_config.another_asterisk_value, "config*test*value");
        assert_eq!(app_config.special_config.normal_value, "regular_value");

        // Test direct access to asterisk values with a new config instance
        let config2 = Config::builder()
            .add_source(File::with_name("test-config"))
            .build()
            .expect("Failed to load config");

        let asterisk_value: String = config2.get("special_config.test_value_with_asterisk")
            .expect("Failed to get asterisk value");
        assert_eq!(asterisk_value, "value*with*asterisk");

        let another_asterisk: String = config2.get("special_config.another_asterisk_value")
            .expect("Failed to get another asterisk value");
        assert_eq!(another_asterisk, "config*test*value");
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

        let asterisk_value: String = config.get("special_config.test_value_with_asterisk")
            .expect("Failed to get asterisk value");
        assert_eq!(asterisk_value, "value*with*asterisk");
    }
}