use anyhow::Result;
use itertools::Itertools;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config,
    data::MeasurementData,
    git_interop::add_note_line_to_head,
    serialization::{serialize_multiple, serialize_single},
};

pub fn add_multiple(
    measurement: &str,
    values: &[f64],
    key_values: &[(String, String)],
) -> Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("TODO(kaihowl)");

    let timestamp = timestamp.as_secs_f64();
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();

    // TODO(kaihowl) inefficient recopying
    let mds = values
        .iter()
        .map(|v| MeasurementData {
            // TODO(hoewelmk)
            epoch: config::determine_epoch_from_config(measurement).unwrap_or(0),
            name: measurement.to_owned(),
            timestamp,
            val: *v,
            key_values: key_values.clone(),
        })
        .collect_vec();

    let serialized = serialize_multiple(&mds);

    add_note_line_to_head(&serialized)?;

    Ok(())
}

pub fn add(measurement: &str, value: f64, key_values: &[(String, String)]) -> Result<()> {
    // TODO(kaihowl) configure path
    // TODO(kaihowl) configure
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("TODO(kaihowl)");

    let timestamp = timestamp.as_secs_f64();
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();

    let md = MeasurementData {
        // TODO(hoewelmk)
        epoch: config::determine_epoch_from_config(measurement).unwrap_or(0),
        name: measurement.to_owned(),
        timestamp,
        val: value,
        key_values,
    };

    let serialized = serialize_single(&md);

    add_note_line_to_head(&serialized)?;

    Ok(())
}
