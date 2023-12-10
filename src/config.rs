use anyhow::Result;
use std::{
    fs::File,
    io::{Read, Write},
};
use toml_edit::{value, Document};

use crate::git_interop::get_head_revision;

// TODO(kaihowl) proper error handling
pub fn write_config(conf: &str) {
    let mut f = File::create(".gitperfconfig").expect("open file for writing failed");
    f.write_all(conf.as_bytes()).expect("failed to write");
}

pub fn read_config() -> Option<String> {
    read_config_from_file(".gitperfconfig")
}

// TODO(kaihowl) proper error handling
// TODO(kaihowl) proper file type
fn read_config_from_file(file: &str) -> Option<String> {
    let mut conf_str = String::new();
    File::open(file).ok()?.read_to_string(&mut conf_str).ok()?;
    Some(conf_str)
}

pub fn determine_epoch_from_config(measurement: &str) -> Option<u32> {
    // TODO(hoewelmk) configure path, use different working directory than repo root
    let conf = read_config()?;
    determine_epoch(measurement, &conf)
}

fn determine_epoch(measurement: &str, conf_str: &str) -> Option<u32> {
    // TODO(kaihowl) buffered reading?
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

// TODO(kaihowl) proper error handling
pub fn bump_epoch(measurement: &str) -> Result<()> {
    let mut conf_str = read_config().unwrap_or_default();
    bump_epoch_in_conf(measurement, &mut conf_str)?;
    write_config(&conf_str);
    Ok(())
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
        dbg!(&conf);
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
}
