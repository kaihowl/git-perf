use std::{
    borrow::Borrow,
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        HashMap,
    },
};

use itertools::Itertools;
use log::warn;

use crate::data::MeasurementData;

pub const DELIMITER: &str = "";

pub fn serialize_single<M>(measurement_data: &M, custom_delimiter: &str) -> String
where
    M: Borrow<MeasurementData>,
{
    let md: &MeasurementData = measurement_data.borrow();

    let mut m = vec![
        format!("{:?}", md.epoch),
        md.name.clone(),
        format!("{:?}", md.timestamp),
        format!("{:?}", md.val),
    ];

    m.extend(md.key_values.iter().map(|(k, v)| format!("{k}={v}")));

    m.join(custom_delimiter) + "\n"
}

pub fn serialize_multiple<M: Borrow<MeasurementData>>(measurement_data: &[M]) -> String {
    measurement_data
        .iter()
        .map(|md| serialize_single(md, DELIMITER))
        .join("")
}

fn deserialize_single(line: &str) -> Option<MeasurementData> {
    let components = line
        .split(DELIMITER)
        .filter(|item| !item.is_empty())
        .collect_vec();

    let num_components = components.len();
    if num_components < 4 {
        warn!("Too few items with {num_components}, skipping record");
        return None;
    }

    let epoch = components[0];
    let epoch = match epoch.parse::<u32>() {
        Ok(e) => e,
        Err(err) => {
            warn!("Cannot parse epoch '{epoch}': {err}, skipping record");
            return None;
        }
    };

    let name = components[1].to_string();

    let timestamp = components[2];
    let timestamp = match timestamp.parse::<f64>() {
        Ok(ts) => ts,
        Err(err) => {
            warn!("Cannot parse timestamp '{timestamp}': {err}, skipping record");
            return None;
        }
    };

    let val = components[3];
    let val = match val.parse::<f64>() {
        Ok(val) => val,
        Err(err) => {
            warn!("Cannot parse value '{val}': {err}, skipping record");
            return None;
        }
    };

    let mut key_values = HashMap::new();

    if components.len() > 4 {
        for kv in components.iter().skip(4) {
            if let Some((key, value)) = kv.split_once('=') {
                let entry = key_values.entry(key.to_string());
                let value = value.to_string();
                match entry {
                    Occupied(mut e) => {
                        if e.get() == &value {
                            static DUPLICATE_KEY_SAME_VALUE: std::sync::Once =
                                std::sync::Once::new();
                            DUPLICATE_KEY_SAME_VALUE.call_once(|| {
                                warn!("Duplicate entries for key {key} with same value");
                            });
                        } else {
                            static DUPLICATE_KEY_CONFLICT: std::sync::Once = std::sync::Once::new();
                            DUPLICATE_KEY_CONFLICT.call_once(|| {
                                warn!(
                                    "Conflicting values for key {key}: '{}' vs '{}'",
                                    e.get(),
                                    value
                                );
                            });
                        }
                        e.insert(value);
                    }
                    Vacant(e) => {
                        e.insert(value);
                    }
                }
            } else {
                warn!("No equals sign in key value pair, skipping record");
                return None;
            }
        }
    }

    Some(MeasurementData {
        epoch,
        name,
        timestamp,
        val,
        key_values,
    })
}

pub fn deserialize(lines: &str) -> Vec<MeasurementData> {
    lines
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(deserialize_single)
        .collect_vec()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key_value_deserialization() {
        let lines = "0test1234123key1=value1key2=value2";
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
        let lines = "0test1234123key1value1\n\
                     0test24567890key2=value2";

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
        let lines = "0test1234123";
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
        let serialized = serialize_single(&md, DELIMITER);
        assert_eq!(serialized, "3Mymeasurement1234567.042.0mykey=myvalue\n");
    }

    #[test]
    fn test_epoch_parsing() {
        // Test valid epoch
        let valid_line = "42test1234123";
        let result = deserialize_single(valid_line);
        assert!(result.is_some());
        assert_eq!(result.unwrap().epoch, 42);

        // Test invalid epoch (non-numeric)
        let invalid_line = "not_a_numbertest1234123";
        let result = deserialize_single(invalid_line);
        assert!(result.is_none());

        // Test invalid epoch (out of range)
        let out_of_range_line = "4294967296test1234123"; // u32::MAX + 1
        let result = deserialize_single(out_of_range_line);
        assert!(result.is_none());
    }

    #[test]
    fn test_serialize_multiple_empty() {
        let measurements: Vec<MeasurementData> = vec![];
        let serialized = serialize_multiple(&measurements);
        assert_eq!(serialized, "");
    }

    #[test]
    fn test_serialize_multiple_single() {
        let md = MeasurementData {
            epoch: 1,
            name: "test".into(),
            timestamp: 1000.0,
            val: 5.0,
            key_values: HashMap::new(),
        };
        let serialized = serialize_multiple(&[md]);
        let expected = format!("1{}test{}1000.0{}5.0\n", DELIMITER, DELIMITER, DELIMITER);
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_serialize_multiple_multiple() {
        let md1 = MeasurementData {
            epoch: 1,
            name: "test1".into(),
            timestamp: 1000.0,
            val: 5.0,
            key_values: HashMap::new(),
        };
        let md2 = MeasurementData {
            epoch: 2,
            name: "test2".into(),
            timestamp: 2000.0,
            val: 10.0,
            key_values: HashMap::new(),
        };
        let serialized = serialize_multiple(&[md1, md2]);
        let expected = format!(
            "1{}test1{}1000.0{}5.0\n2{}test2{}2000.0{}10.0\n",
            DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER
        );
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_deserialize_single_exactly_four_components() {
        // Test boundary case: exactly 4 components (no key-value pairs)
        let line = format!(
            "5{}measurement{}1234.5{}67.8",
            DELIMITER, DELIMITER, DELIMITER
        );
        let result = deserialize_single(&line);
        assert!(result.is_some());
        let md = result.unwrap();
        assert_eq!(md.epoch, 5);
        assert_eq!(md.name, "measurement");
        assert_eq!(md.timestamp, 1234.5);
        assert_eq!(md.val, 67.8);
        assert!(md.key_values.is_empty());
    }

    #[test]
    fn test_deserialize_single_more_than_four_components() {
        // Test with more than 4 components (includes key-value pairs)
        let line = format!(
            "0{}test{}1234{}123{}foo=bar",
            DELIMITER, DELIMITER, DELIMITER, DELIMITER
        );
        let result = deserialize_single(&line);
        assert!(result.is_some());
        let md = result.unwrap();
        assert_eq!(md.key_values.len(), 1);
        assert_eq!(md.key_values.get("foo"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_deserialize_serialize_roundtrip() {
        let original = MeasurementData {
            epoch: 10,
            name: "roundtrip_test".into(),
            timestamp: 9999.5,
            val: 42.42,
            key_values: [
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ]
            .into(),
        };

        let serialized = serialize_single(&original, DELIMITER);
        let deserialized_vec = deserialize(&serialized);

        assert_eq!(deserialized_vec.len(), 1);
        let deserialized = &deserialized_vec[0];

        assert_eq!(deserialized.epoch, original.epoch);
        assert_eq!(deserialized.name, original.name);
        assert_eq!(deserialized.timestamp, original.timestamp);
        assert_eq!(deserialized.val, original.val);
        assert_eq!(deserialized.key_values, original.key_values);
    }

    #[test]
    fn test_deserialize_multiple_lines() {
        let lines = format!(
            "1{}test1{}1000.0{}5.0{}key1=val1\n2{}test2{}2000.0{}10.0{}key2=val2\n",
            DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER, DELIMITER
        );
        let results = deserialize(&lines);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "test1");
        assert_eq!(results[1].name, "test2");
    }

    #[test]
    fn test_serialize_single_with_custom_delimiter() {
        let md = MeasurementData {
            epoch: 0,
            name: "test".into(),
            timestamp: 100.0,
            val: 50.0,
            key_values: [("k".to_string(), "v".to_string())].into(),
        };
        let serialized = serialize_single(&md, ",");
        assert_eq!(serialized, "0,test,100.0,50.0,k=v\n");
    }
}
