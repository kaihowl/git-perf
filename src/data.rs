use std::collections::HashMap;

use clap::ValueEnum;
use serde::Deserialize;

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReductionFunc {
    Min,
    Max,
    Median,
    Mean,
}

#[derive(Debug)]
pub struct MeasurementSummary {
    pub epoch: u32,
    pub val: f64,
}

#[derive(Debug)]
pub struct CommitSummary {
    pub commit: String,
    pub measurement: Option<MeasurementSummary>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct MeasurementData {
    pub epoch: u32,
    pub name: String,
    pub timestamp: f64,
    // TODO(kaihowl) check size of type
    pub val: f64,
    #[serde(flatten)]
    pub key_values: HashMap<String, String>,
}
