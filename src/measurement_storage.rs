use std::{
    collections::HashMap,
    fmt::Display,
    process,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

use crate::{
    config, data::MeasurementData, git_interop::add_note_line_to_head,
    serialization::serialize_single,
};

// TODO(kaihowl) use anyhow / thiserror for error propagation
#[derive(Debug, Error)]
pub enum AddError {
    Git(git2::Error),
}

impl Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::Git(e) => write!(f, "git error, {e}"),
        }
    }
}

impl From<git2::Error> for AddError {
    fn from(e: git2::Error) -> Self {
        AddError::Git(e)
    }
}

pub fn add(measurement: &str, value: f64, key_values: &[(String, String)]) -> Result<(), AddError> {
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

pub fn measure(
    measurement: &str,
    repetitions: u16,
    command: &[String],
    key_values: &[(String, String)],
) -> Result<(), AddError> {
    let exe = command.first().unwrap();
    let args = &command[1..];
    for _ in 0..repetitions {
        let mut process = process::Command::new(exe);
        process.args(args);
        let start = Instant::now();
        let output = process
            .output()
            .expect("Command failed to spawn TODO(kaihowl)");
        output
            .status
            .success()
            .then_some(())
            .ok_or("TODO(kaihowl) running error")
            .expect("TODO(kaihowl)");
        let duration = start.elapsed();
        let duration_usec = duration.as_micros() as f64;
        add(measurement, duration_usec, key_values)?;
    }
    Ok(())
}
