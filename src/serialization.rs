use std::{
    borrow::Borrow,
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        HashMap,
    },
};

use itertools::Itertools;

use crate::data::MeasurementData;

// TODO(kaihowl) serialization with flatten and custom function does not work
#[derive(Debug, PartialEq)]
struct SerializeMeasurementData<'a> {
    epoch: u32,
    name: &'a str,
    timestamp: f64,
    val: f64,
    key_values: &'a HashMap<String, String>,
}

pub const DELIMITER: &str = "";

pub fn serialize_single<M>(measurement_data: &M) -> String
where
    M: Borrow<MeasurementData>,
{
    let md: &MeasurementData = measurement_data.borrow();

    // TODO(kaihowl) key values
    let mut m = vec![
        md.epoch.to_string(),
        md.name.clone(),
        md.timestamp.to_string(),
        md.val.to_string(),
    ];

    m.extend(
        md.key_values
            .iter()
            .map(|(k, v)| format!("{k}{DELIMITER}{v}")),
    );

    m.join(DELIMITER)
}

pub fn serialize_multiple<M: Borrow<MeasurementData>>(measurement_data: &[M]) -> String {
    let mut result = String::new();

    for md in measurement_data {
        let md = md.borrow();
        let record = [md.epoch.to_string(), md.timestamp.to_string()].join(DELIMITER);

        result.push_str(&record);
        result.push_str("\n");
    }

    return result;
}

pub fn deserialize(lines: &str) -> Vec<MeasurementData> {
    let mut result = vec![];

    for line in lines.lines() {
        let components = line.split(DELIMITER).collect_vec();
        if components.len() < 4 {
            eprintln!("Too few items, skipping record");
            continue;
        }

        // TODO(kaihowl) test this
        let epoch = components[0];
        let epoch = match epoch.parse::<u32>() {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Cannot parse epoch '{epoch}': {err}, skipping record");
                continue;
            }
        };

        let name = components[1].to_string();

        let timestamp = components[2];
        let timestamp = match timestamp.parse::<f64>() {
            Ok(ts) => ts,
            Err(err) => {
                eprintln!("Cannot parse timestamp '{timestamp}': {err}, skipping record");
                continue;
            }
        };

        let val = components[3];
        let val = match val.parse::<f64>() {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Cannot parse value '{val}': {err}, skipping record");
                continue;
            }
        };

        let mut key_values = HashMap::new();

        let mut skip_record = false;
        if components.len() > 4 {
            for kv in components.iter().skip(4) {
                // TODO(kaihowl) different delimiter?
                if let Some((key, value)) = kv.split_once('=') {
                    let entry = key_values.entry(key.to_string());
                    let value = value.to_string();
                    match entry {
                        Occupied(mut e) => {
                            eprintln!("Duplicate entries for key {key}");
                            e.insert(value);
                        }
                        Vacant(e) => {
                            e.insert(value);
                        }
                    }
                } else {
                    eprintln!("No equals sign in key value pair, skipping record");
                    skip_record = true;
                }
            }
        }

        if skip_record {
            continue;
        }

        result.push(MeasurementData {
            epoch,
            name,
            timestamp,
            val,
            key_values,
        });
    }

    return result;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key_value_deserialization() {
        let lines = "0 test 1234 123 key1=value1 key2=value2";
        let actual = deserialize(lines);
        let expected = MeasurementData {
            epoch: 0,
            name: "test".to_string(),
            timestamp: 1234.0,
            val: 123.0,
            key_values: [
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ]
            .into(),
        };
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0], expected);
    }

    #[test]
    fn key_value_invalid_pair() {
        // Missing equals sign in first line, should be skipped
        let lines = "0 test 1234 123 key1 value1\n\
                     0 test2 4567 890 key2=value2";

        let expected = [MeasurementData {
            epoch: 0,
            name: "test2".to_string(),
            timestamp: 4567.0,
            val: 890.0,
            key_values: [("key2".to_string(), "value2".to_string())].into(),
        }];
        let actual = deserialize(lines);
        assert_eq!(actual, expected);
    }

    #[test]
    fn additional_whitespace_deserialization() {
        let lines = "0     test     1234     123";
        let actual = deserialize(lines);
        assert_eq!(1, actual.len());
    }

    #[test]
    fn test_serialize_single() {
        let md = MeasurementData {
            epoch: 3,
            name: "Mymeasurement".into(),
            timestamp: 1234567.0,
            val: 42.0,
            key_values: [("mykey".to_string(), "myvalue".to_string())].into(),
        };
        let serialized = serialize_single(&md);
        assert_eq!(serialized, "3 Mymeasurement 1234567.0 42.0 mykey=myvalue\n");
    }
}
