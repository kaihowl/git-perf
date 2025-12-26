use anyhow::{Context, Result};
use itertools::Itertools;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::prelude::*;

use crate::{
    config,
    data::MeasurementData,
    defaults,
    git::git_interop::{add_note_line, resolve_committish},
    serialization::{serialize_multiple, serialize_single, DELIMITER},
};

pub fn add_multiple_to_commit(
    commit: &str,
    measurement: &str,
    values: &[f64],
    key_values: &[(String, String)],
) -> Result<()> {
    // Validate commit exists
    let resolved_commit =
        resolve_committish(commit).context(format!("Failed to resolve commit '{}'", commit))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get system time")?;

    let timestamp = timestamp.as_secs_f64();
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();
    let epoch = config::determine_epoch_from_config(measurement).unwrap_or(defaults::DEFAULT_EPOCH);
    let name = measurement.to_owned();

    let mds = values
        .iter()
        .map(|&val| MeasurementData {
            epoch,
            name: name.clone(),
            timestamp,
            val,
            key_values: key_values.clone(),
        })
        .collect_vec();

    let serialized = serialize_multiple(&mds);

    add_note_line(&resolved_commit, &serialized)?;

    Ok(())
}

pub fn add_multiple(
    measurement: &str,
    values: &[f64],
    key_values: &[(String, String)],
) -> Result<()> {
    let head =
        crate::git::git_interop::get_head_revision().context("Failed to get HEAD revision")?;
    add_multiple_to_commit(&head, measurement, values, key_values)
}

pub fn add_to_commit(
    commit: &str,
    measurement: &str,
    value: f64,
    key_values: &[(String, String)],
) -> Result<()> {
    // Validate commit exists
    let resolved_commit =
        resolve_committish(commit).context(format!("Failed to resolve commit '{}'", commit))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Failed to get system time")?;

    let timestamp = timestamp.as_secs_f64();
    let key_values: HashMap<_, _> = key_values.iter().cloned().collect();

    let md = MeasurementData {
        epoch: config::determine_epoch_from_config(measurement).unwrap_or(defaults::DEFAULT_EPOCH),
        name: measurement.to_owned(),
        timestamp,
        val: value,
        key_values,
    };

    let serialized = serialize_single(&md, DELIMITER);

    add_note_line(&resolved_commit, &serialized)?;

    Ok(())
}

pub fn add(measurement: &str, value: f64, key_values: &[(String, String)]) -> Result<()> {
    let head =
        crate::git::git_interop::get_head_revision().context("Failed to get HEAD revision")?;
    add_to_commit(&head, measurement, value, key_values)
}

pub fn remove_measurements_from_commits(
    older_than: DateTime<Utc>,
    prune: bool,
    dry_run: bool,
) -> Result<()> {
    crate::git::git_interop::remove_measurements_from_commits(older_than, prune, dry_run)
}
