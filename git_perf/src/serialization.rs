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
            // TODO(kaihowl) different delimiter?
            if let Some((key, value)) = kv.split_once('=') {
                let entry = key_values.entry(key.to_string());
                let value = value.to_string();
                match entry {
                    Occupied(mut e) => {
                        // TODO(kaihowl) reinstate + only emit this (and other) errors once
                        // eprintln!("Duplicate entries for key {key}");
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
}
