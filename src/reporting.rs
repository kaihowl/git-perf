use std::{
    fs::File,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use anyhow::{bail, Result};
use itertools::Itertools;
use plotly::{
    common::{Font, LegendGroupTitle, Title},
    layout::{Axis, Legend},
    Configuration, Layout, Plot,
};
use serde::Serialize;

// TODO(kaihowl) find central place for the data structures
use crate::{
    data::{MeasurementData, MeasurementSummary, ReductionFunc},
    measurement_retrieval::{self, Commit, ReductionFuncIterator},
};

trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);
    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        measurement_name: &str,
        group_value: Option<&String>,
    );
    fn add_summarized_trace(
        &mut self,
        indexed_measurements: Vec<(usize, MeasurementSummary)>,
        measurement_name: &str,
        group_value: Option<&String>,
    );
    fn as_bytes(&self) -> Vec<u8>;
}

struct PlotlyReporter {
    plot: Plot,
    // TODO(kaihowl) hack until we can auto_range 'reverse' the axis in plotly directly
    size: usize,
}

impl PlotlyReporter {
    fn new() -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(true);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter { plot, size: 0 }
    }

    fn convert_to_x_y(&self, indexed_measurements: Vec<(usize, f64)>) -> (Vec<usize>, Vec<f64>) {
        indexed_measurements
            .iter()
            .map(|(i, m)| (self.size - i - 1, m))
            .unzip()
    }
}

impl<'a> Reporter<'a> for PlotlyReporter {
    fn add_commits(&mut self, commits: &'a [Commit]) {
        let enumerated_commits = commits.iter().rev().enumerate();
        self.size = commits.len();

        let (commit_nrs, short_hashes): (Vec<_>, Vec<_>) = enumerated_commits
            .map(|(n, c)| (n as f64, c.commit[..6].to_owned()))
            .unzip();
        let x_axis = Axis::new()
            .tick_values(commit_nrs)
            .tick_text(short_hashes)
            .tick_angle(45.0)
            .tick_font(Font::new().family("monospace"));
        let layout = Layout::new()
            .title(Title::new("Performance Measurements"))
            .x_axis(x_axis)
            .legend(
                Legend::new()
                    .group_click(plotly::layout::GroupClick::ToggleItem)
                    .orientation(plotly::common::Orientation::Horizontal),
            );

        self.plot.set_layout(layout);
    }

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        measurement_name: &str,
        group_value: Option<&String>,
    ) {
        let (x, y) = self.convert_to_x_y(
            indexed_measurements
                .into_iter()
                .map(|(i, m)| (i, m.val))
                .collect_vec(),
        );

        let trace = plotly::BoxPlot::new_xy(x, y);

        let trace = if let Some(group_value) = group_value {
            trace
                .name(group_value)
                .legend_group(measurement_name)
                .legend_group_title(LegendGroupTitle::new(measurement_name))
        } else {
            trace.name(measurement_name)
        };

        self.plot.add_trace(trace);
    }

    fn add_summarized_trace(
        &mut self,
        indexed_measurements: Vec<(usize, MeasurementSummary)>,
        measurement_name: &str,
        group_value: Option<&String>,
    ) {
        let (x, y) = self.convert_to_x_y(
            indexed_measurements
                .into_iter()
                .map(|(i, m)| (i, m.val))
                .collect_vec(),
        );

        let trace = plotly::Scatter::new(x, y).name(measurement_name);

        let trace = if let Some(group_value) = group_value {
            trace
                .name(group_value)
                .legend_group(measurement_name)
                .legend_group_title(LegendGroupTitle::new(measurement_name))
        } else {
            trace.name(measurement_name)
        };

        self.plot.add_trace(trace);
    }

    fn as_bytes(&self) -> Vec<u8> {
        self.plot.to_html().as_bytes().to_vec()
    }
}

struct CsvReporter<'a> {
    hashes: Vec<String>,
    indexed_measurements: Vec<(usize, &'a MeasurementData)>,
}

impl CsvReporter<'_> {
    fn new() -> Self {
        CsvReporter {
            hashes: Vec::new(),
            indexed_measurements: Vec::new(),
        }
    }
}
#[derive(Serialize)]
struct HashAndMeasurement<'a> {
    commit: &'a str,
    measurement: &'a MeasurementData,
}

impl<'a> Reporter<'a> for CsvReporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]) {
        self.hashes = hashes.iter().map(|c| c.commit.to_owned()).collect();
    }

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        _measurement_name: &str,
        _group_value: Option<&String>,
    ) {
        self.indexed_measurements
            .extend_from_slice(indexed_measurements.as_slice());
    }

    fn as_bytes(&self) -> Vec<u8> {
        // TODO(kaihowl) write to path directly instead?
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_writer(vec![]);

        for (index, measurement_data) in &self.indexed_measurements {
            writer
                .serialize(HashAndMeasurement {
                    commit: &self.hashes[*index],
                    measurement: measurement_data,
                })
                .expect("inner serialization error TODO(kaihowl)")
        }
        writer
            .into_inner()
            .expect("serialization error TODO(kaihowl)")
    }

    fn add_summarized_trace(
        &mut self,
        _indexed_measurements: Vec<(usize, MeasurementSummary)>,
        _measurement_name: &str,
        _group_value: Option<&String>,
    ) {
        todo!()
    }
}

struct ReporterFactory {}

impl ReporterFactory {
    fn from_file_name<'a, 'b: 'a>(path: &'b Path) -> Option<Box<dyn Reporter + 'a>> {
        if path == Path::new("-") {
            return Some(Box::new(CsvReporter::new()) as Box<dyn Reporter + 'a>);
        }
        let mut res = None;
        if let Some(ext) = path.extension() {
            let extension = ext.to_ascii_lowercase().into_string().unwrap();
            res = match extension.as_str() {
                "html" => Some(Box::new(PlotlyReporter::new()) as Box<dyn Reporter>),
                "csv" => Some(Box::new(CsvReporter::new()) as Box<dyn Reporter + 'a>),
                _ => None,
            }
        }
        res
    }
}
// TODO(kaihowl) needs more fine grained output e2e tests
pub fn report(
    output: PathBuf,
    separate_by: Option<String>,
    num_commits: usize,
    measurement_names: &[String],
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
) -> Result<()> {
    let commits: Vec<Commit> = measurement_retrieval::walk_commits(num_commits)?.try_collect()?;

    let mut plot =
        ReporterFactory::from_file_name(&output).ok_or(anyhow!("Could not infer output format"))?;

    plot.add_commits(&commits);

    let relevant = |m: &MeasurementData| {
        if !measurement_names.is_empty() && !measurement_names.contains(&m.name) {
            return false;
        }
        // TODO(kaihowl) express this and the audit-fn equivalent as subset relations
        key_values
            .iter()
            .all(|(k, v)| m.key_values.get(k).map(|mv| v == mv).unwrap_or(false))
    };

    let relevant_measurements = commits
        .iter()
        .map(|commit| commit.measurements.iter().filter(|m| relevant(m)));

    let unique_measurement_names: Vec<_> = relevant_measurements
        .clone()
        .flat_map(|m| m.map(|m| &m.name))
        .unique()
        .collect();

    if unique_measurement_names.is_empty() {
        bail!("No performance measurements found.")
    }

    for measurement_name in unique_measurement_names {
        let filtered_measurements = relevant_measurements
            .clone()
            .map(|ms| ms.filter(|m| m.name == *measurement_name));

        let group_values = if let Some(separate_by) = &separate_by {
            filtered_measurements
                .clone()
                .flat_map(|ms| {
                    ms.flat_map(|m| {
                        m.key_values
                            .iter()
                            .filter(|(k, _v)| *k == separate_by)
                            .map(|(_k, v)| v)
                    })
                })
                .unique()
                .map(|val| (Some(separate_by), Some(val)))
                .collect_vec()
        } else {
            vec![(None, None)]
        };

        if group_values.is_empty() {
            bail!("Invalid separator supplied, no measurements.")
        }

        for (group_key, group_value) in group_values {
            let group_measurements = filtered_measurements.clone().map(|ms| {
                ms.filter(|m| {
                    group_key
                        .map(|gk| m.key_values.get(gk) == group_value)
                        .unwrap_or(true)
                })
            });

            if let Some(reduction_func) = aggregate_by {
                let trace_measurements = group_measurements
                    .clone()
                    .enumerate()
                    .flat_map(move |(i, ms)| {
                        ms.reduce_by(reduction_func)
                            .into_iter()
                            .map(move |m| (i, m))
                    })
                    .collect_vec();
                plot.add_summarized_trace(trace_measurements, measurement_name, group_value);
            } else {
                let trace_measurements: Vec<_> = group_measurements
                    .clone()
                    .enumerate()
                    .flat_map(|(i, ms)| ms.map(move |m| (i, m)))
                    .collect();
                plot.add_trace(trace_measurements, measurement_name, group_value);
            }
        }
    }

    // TODO(kaihowl) fewer than the -n specified measurements appear in plot (old problem, even in
    // python)

    if output == Path::new("-") {
        match io::stdout().write_all(&plot.as_bytes()) {
            Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
            res => res,
        }?;
    } else {
        File::create(&output)?.write_all(&plot.as_bytes())?;
    }

    Ok(())
}
