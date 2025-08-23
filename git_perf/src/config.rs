use anyhow::Result;
use std::{
    fs::File,
    io::{Read, Write},
};
use toml_edit::{value, Document};

use crate::git::git_interop::get_head_revision;

pub fn write_config(conf: &str) -> Result<()> {
    let mut f = File::create(".gitperfconfig")?;
    f.write_all(conf.as_bytes())?;
    Ok(())
}

pub fn read_config() -> Result<String> {
    read_config_from_file(".gitperfconfig")
}

use std::path::Path;

fn read_config_from_file<P: AsRef<Path>>(file: P) -> Result<String> {
    let mut conf_str = String::new();
    File::open(file)?.read_to_string(&mut conf_str)?;
    Ok(conf_str)
}

pub fn determine_epoch_from_config(measurement: &str) -> Option<u32> {
    // TODO(hoewelmk) configure path, use different working directory than repo root
    // TODO(hoewelmk) proper error handling
    let conf = read_config().ok()?;
    determine_epoch(measurement, &conf)
}

fn determine_epoch(measurement: &str, conf_str: &str) -> Option<u32> {
    let config = conf_str
        .parse::<Document>()
        .expect("Failed to parse config");

    let get_epoch = |section: &str| {
        let s = config
            .get("measurement")?
            .get(section)?
            .get("epoch")?
            .as_str()?;
        u32::from_str_radix(s, 16).ok()
    };

    get_epoch(measurement).or_else(|| get_epoch("*"))
}

pub fn bump_epoch_in_conf(measurement: &str, conf_str: &mut String) -> Result<()> {
    let mut conf = conf_str
        .parse::<Document>()
        .expect("failed to parse config");

    let head_revision = get_head_revision()?;
    // TODO(kaihowl) ensure that always non-inline tables are written in an empty config file
    conf["measurement"][measurement]["epoch"] = value(&head_revision[0..8]);
    *conf_str = conf.to_string();

    Ok(())
}

pub fn bump_epoch(measurement: &str) -> Result<()> {
    let mut conf_str = read_config().unwrap_or_default();
    bump_epoch_in_conf(measurement, &mut conf_str)?;
    write_config(&conf_str)?;
    Ok(())
}

/// Returns the backoff max elapsed seconds from a config string, or 60 if not set.
pub fn backoff_max_elapsed_seconds_from_str(conf: &str) -> u64 {
    let doc = conf.parse::<Document>().ok();
    doc.and_then(|doc| {
        doc.get("backoff")
            .and_then(|b| b.get("max_elapsed_seconds"))
            .and_then(|v| v.as_integer())
            .map(|v| v as u64)
    })
    .unwrap_or(60)
}

/// Returns the backoff max elapsed seconds from config, or 60 if not set.
pub fn backoff_max_elapsed_seconds() -> u64 {
    backoff_max_elapsed_seconds_from_str(read_config().unwrap_or_default().as_str())
}

/// Returns the minimum relative deviation threshold from a config string, or None if not set.
/// Follows precedence: measurement-specific > global > None
pub fn audit_min_relative_deviation_from_str(conf: &str, measurement: &str) -> Option<f64> {
    let doc = conf.parse::<Document>().ok()?;

    // Check for measurement-specific setting first
    if let Some(threshold) = doc
        .get("audit")
        .and_then(|audit| audit.get("measurement"))
        .and_then(|measurement_section| measurement_section.get(measurement))
        .and_then(|config| config.get("min_relative_deviation"))
        .and_then(|threshold| threshold.as_float())
    {
        return Some(threshold);
    }

    // Check for global setting
    if let Some(threshold) = doc
        .get("audit")
        .and_then(|audit| audit.get("global"))
        .and_then(|global| global.get("min_relative_deviation"))
        .and_then(|threshold| threshold.as_float())
    {
        return Some(threshold);
    }

    None
}

/// Returns the minimum relative deviation threshold from config, or None if not set.
pub fn audit_min_relative_deviation(measurement: &str) -> Option<f64> {
    audit_min_relative_deviation_from_str(read_config().unwrap_or_default().as_str(), measurement)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_epochs() {
        // TODO(hoewelmk) order unspecified in serialization...
        let configfile = r#"[measurement."something"]
#My comment
epoch="34567898"

[measurement."somethingelse"]
epoch="a3dead"

[measurement."*"]
# General performance regression
epoch="12344555"
"#;

        let epoch = determine_epoch("something", configfile);
        assert_eq!(epoch, Some(0x34567898));

        let epoch = determine_epoch("somethingelse", configfile);
        assert_eq!(epoch, Some(0xa3dead));

        let epoch = determine_epoch("unspecified", configfile);
        assert_eq!(epoch, Some(0x12344555));
    }

    #[test]
    fn test_bump_epochs() {
        let configfile = r#"[measurement."something"]
#My comment
epoch = "34567898"
"#;

        let mut actual = String::from(configfile);
        bump_epoch_in_conf("something", &mut actual).expect("Failed to bump epoch");

        let expected = format!(
            r#"[measurement."something"]
#My comment
epoch = "{}"
"#,
            &get_head_revision().expect("get_head_revision failed")[0..8],
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_bump_new_epoch_and_read_it() {
        let mut conf = String::new();
        bump_epoch_in_conf("mymeasurement", &mut conf).expect("Failed to bump epoch");
        let epoch = determine_epoch("mymeasurement", &conf);
        assert!(epoch.is_some());
    }

    #[test]
    fn test_parsing() {
        let toml_str = r#"
        measurement = { test2 = { epoch = "834ae670e2ecd5c87020fde23378b890832d6076" } }
    "#;

        let doc = toml_str.parse::<Document>().expect("sfdfdf");

        let measurement = "test";

        if let Some(e) = doc
            .get("measurement")
            .and_then(|m| m.get(measurement))
            .and_then(|m| m.get("epoch"))
        {
            println!("YAY: {}", e);
            panic!("stuff");
        }
    }

    #[test]
    fn test_backoff_max_elapsed_seconds() {
        // Case 1: config string with explicit value
        let configfile = "[backoff]\nmax_elapsed_seconds = 42\n";
        assert_eq!(super::backoff_max_elapsed_seconds_from_str(configfile), 42);

        // Case 2: config string missing value
        let configfile = "";
        assert_eq!(super::backoff_max_elapsed_seconds_from_str(configfile), 60);
    }

    #[test]
    fn test_audit_min_relative_deviation() {
        // Case 1: measurement-specific setting
        let configfile = r#"
[audit.measurement."build_time"]
min_relative_deviation = 10.0

[audit.measurement."memory_usage"]
min_relative_deviation = 2.5
"#;
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "build_time"),
            Some(10.0)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "memory_usage"),
            Some(2.5)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "other_measurement"),
            None
        );

        // Case 2: global setting
        let configfile = r#"
[audit.global]
min_relative_deviation = 5.0
"#;
        println!("Testing Case 2: global setting");
        let result = super::audit_min_relative_deviation_from_str(configfile, "any_measurement");
        println!("Case 2 result: {:?}", result);
        assert_eq!(result, Some(5.0));

        // Case 3: precedence - measurement-specific overrides global
        let configfile = r#"
[audit.global]
min_relative_deviation = 5.0

[audit.measurement."build_time"]
min_relative_deviation = 10.0
"#;
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "build_time"),
            Some(10.0)
        );
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "other_measurement"),
            Some(5.0)
        );

        // Case 5: no audit configuration
        let configfile = "";
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "any_measurement"),
            None
        );

        // Case 6: invalid config (should return None)
        let configfile = "[audit]\nmin_relative_deviation = invalid\n";
        assert_eq!(
            super::audit_min_relative_deviation_from_str(configfile, "any_measurement"),
            None
        );
    }
}
