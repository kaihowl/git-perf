use std::collections::HashMap;

use csv::StringRecord;
use itertools::{EitherOrBoth, Itertools};
use serde::{ser::SerializeSeq, Serialize, Serializer};

use crate::data::MeasurementData;

// TODO(kaihowl) serialization with flatten and custom function does not work
#[derive(Debug, PartialEq, Serialize)]
struct SerializeMeasurementData<'a> {
    epoch: u32,
    name: &'a str,
    timestamp: f64,
    val: f64,
    #[serde(serialize_with = "key_value_serialization")]
    key_values: &'a HashMap<String, String>,
}

impl Serialize for MeasurementData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeMeasurementData::from(self).serialize(serializer)
    }
}

impl<'a> From<&'a MeasurementData> for SerializeMeasurementData<'a> {
    fn from(md: &'a MeasurementData) -> Self {
        SerializeMeasurementData {
            epoch: md.epoch,
            name: md.name.as_str(),
            timestamp: md.timestamp,
            val: md.val,
            key_values: &md.key_values,
        }
    }
}

fn key_value_serialization<S>(
    key_values: &HashMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(key_values.len()))?;
    for (k, v) in key_values {
        seq.serialize_element(&format!("{}={}", k, v))?
    }
    seq.end()
}

pub fn serialize_single(measurement_data: &MeasurementData) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b' ')
        .has_headers(false)
        .flexible(true)
        .from_writer(vec![]);

    writer
        .serialize(measurement_data)
        .expect("TODO(kaihowl) fix me");
    let result = String::from_utf8(writer.into_inner().unwrap()).unwrap();
    // dbg!("My result: {}", &result);
    result
}

pub fn deserialize(lines: &str) -> Vec<MeasurementData> {
    let reader = csv::ReaderBuilder::new()
        .delimiter(b' ')
        .has_headers(false)
        .flexible(true)
        .from_reader(lines.as_bytes());

    reader
        .into_records()
        .filter_map(|r| {
            if let Err(e) = &r {
                eprintln!("{e}, skipping record.");
            }
            let record = r.ok()?;
            // Filter empty record fields: Repeated whitespace in records does not count as
            // a field separator.
            let record: StringRecord = record.into_iter().filter(|f| !f.is_empty()).collect();
            let fixed_headers = vec!["epoch", "name", "timestamp", "val"];

            let mut skip_record = false;
            let (headers, values): (csv::StringRecord, csv::StringRecord) = record
                .into_iter()
                .zip_longest(fixed_headers)
                .filter_map(|pair| match pair {
                    EitherOrBoth::Both(val, header) => Some((header, val)),
                    EitherOrBoth::Right(_) => {
                        eprintln!("Too few items, skipping record");
                        skip_record = true;
                        None
                    }
                    EitherOrBoth::Left(keyvalue) => match keyvalue.split_once('=') {
                        Some(a) => Some(a),
                        None => {
                            eprintln!("No equals sign in key value pair, skipping record");
                            skip_record = true;
                            None
                        }
                    },
                })
                .unzip();

            if skip_record {
                None
            } else {
                match values.deserialize(Some(&headers)) {
                    Ok(md) => Some(md),
                    Err(e) => {
                        eprintln!("{e}, skipping record");
                        None
                    }
                }
            }
        })
        .collect()
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
