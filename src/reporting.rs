use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use anyhow::{bail, Result};
use itertools::Itertools;
use plotly::{
    common::{Font, LegendGroupTitle, Title},
    layout::{Axis, Legend},
    BoxPlot, Configuration, Layout, Plot, Scatter, Trace,
};
use serde::Serialize;

// TODO(kaihowl) find central place for the data structures
use crate::{
    data::MeasurementData,
    measurement_retrieval::{self, Commit},
};

trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);
    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        group_value: Option<&String>,
    );
    fn as_bytes(&self) -> Vec<u8>;
}

struct PlotlyReporter {
    plot: Plot,
}

impl PlotlyReporter {
    fn new() -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(true);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter { plot }
    }
}

trait NameAndLegend: Trace {
    fn legend_name(&mut self, name: &str) -> Box<dyn NameAndLegend>;
    fn legend_group_with_title(&mut self, name: &str) -> Box<dyn NameAndLegend>;
    fn as_trace(self) -> Box<dyn Trace>;
}

impl<X, Y> NameAndLegend for BoxPlot<X, Y>
where
    X: Serialize + Clone,
    Y: Serialize + Clone,
{
    fn legend_name(&mut self, name: &str) -> Box<dyn NameAndLegend> {
        self.name(name)
    }

    fn legend_group_with_title(&mut self, name: &str) -> Box<dyn NameAndLegend> {
        self.legend_group(name)
            .legend_group_title(LegendGroupTitle::new(name))
    }

    fn as_trace(self) -> Box<dyn Trace> {
        Box::new(self)
    }
}

impl<X, Y> NameAndLegend for Scatter<X, Y>
where
    X: Serialize + Clone,
    Y: Serialize + Clone,
{
    fn legend_name(&mut self, name: &str) -> Box<dyn NameAndLegend> {
        self.name(name)
    }

    fn legend_group_with_title(&mut self, name: &str) -> Box<dyn NameAndLegend> {
        self.legend_group(name)
            .legend_group_title(LegendGroupTitle::new(name))
    }

    fn as_trace(self) -> Box<dyn Trace> {
        Box::new(self)
    }
}

impl<'a> Reporter<'a> for PlotlyReporter {
    fn add_commits(&mut self, commits: &'a [Commit]) {
        let enumerated_commits = commits.iter().enumerate();

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
        group_value: Option<&String>,
    ) {
        let mut measurement_name = None;
        let (x, y): (Vec<usize>, Vec<f64>) = indexed_measurements
            .into_iter()
            .inspect(|(_, md)| {
                // TODO(kaihowl)
                measurement_name = Some(&md.name);
            })
            .map(|(i, m)| (i, m.val))
            .unzip();

        let num_commits = x.iter().unique().count();
        enum PlotType {
            Line,
            Box,
        }

        let trace: Box<dyn NameAndLegend> = if num_commits == y.len()
        // there is a single measurement per commit
        {
            plotly::Scatter::new(x, y)
        } else {
            plotly::BoxPlot::new_xy(x, y)
        };

        let measurement_name = measurement_name.expect("No measurements supplied for trace");
        let trace = if let Some(group_value) = group_value {
            trace
                .legend_name(&group_value)
                .legend_group_with_title(&measurement_name)
        } else {
            trace.legend_name(&measurement_name)
        };
        self.plot.add_trace(trace.as_trace());
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

    let indexed_measurements = commits.iter().enumerate().flat_map(|(index, commit)| {
        commit
            .measurements
            .iter()
            .map(move |m| (index, m))
            .filter(|(_, m)| relevant(m))
    });

    let unique_measurement_names: Vec<_> = indexed_measurements
        .clone()
        .map(|(_, m)| &m.name)
        .unique()
        .collect();

    if unique_measurement_names.is_empty() {
        bail!("No performance measurements found.")
    }

    for measurement_name in unique_measurement_names {
        let filtered_measurements = indexed_measurements
            .clone()
            .filter(|(_i, m)| m.name == *measurement_name);

        let group_values = if let Some(separate_by) = &separate_by {
            filtered_measurements
                .clone()
                .flat_map(|(_, m)| {
                    m.key_values
                        .iter()
                        .filter(|kv| kv.0 == separate_by)
                        .map(|kv| kv.1)
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
            let trace_measurements: Vec<_> = filtered_measurements
                .clone()
                .filter(|(_, m)| {
                    group_key
                        .map(|key| m.key_values.get(key) == group_value)
                        .unwrap_or(true)
                })
                .collect();
            plot.add_trace(trace_measurements, group_value);
        }
    }

    // TODO(kaihowl) fewer than the -n specified measurements appear in plot (old problem, even in
    // python)

    if output == Path::new("-") {
        io::stdout()
            .write_all(&plot.as_bytes())
            .expect("Could not write to stdout");
    } else {
        File::create(&output)
            .expect("Cannot open file")
            .write_all(&plot.as_bytes())
            .expect("Could not write file");
    }

    Ok(())
}
