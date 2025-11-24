use std::{
    collections::HashMap,
    fs::File,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use anyhow::{bail, Result};
use itertools::Itertools;
use plotly::{
    common::{DashType, Font, LegendGroupTitle, Line, Mode, Title, Visible},
    layout::{Axis, Legend},
    Configuration, Layout, Plot, Scatter,
};

use crate::{
    change_point::{ChangeDirection, ChangePoint, EpochTransition},
    config,
    data::{Commit, MeasurementData, MeasurementSummary},
    measurement_retrieval::{self, MeasurementReducer},
    stats::ReductionFunc,
};

/// Formats a measurement name with its configured unit, if available.
/// Returns "measurement_name (unit)" if unit is configured, otherwise just "measurement_name".
fn format_measurement_with_unit(measurement_name: &str) -> String {
    match config::measurement_unit(measurement_name) {
        Some(unit) => format!("{} ({})", measurement_name, unit),
        None => measurement_name.to_string(),
    }
}

/// CSV row representation of a measurement with unit column.
/// Metadata is stored separately and concatenated during serialization.
struct CsvMeasurementRow {
    commit: String,
    epoch: u32,
    measurement: String,
    timestamp: f64,
    value: f64,
    unit: String,
    metadata: HashMap<String, String>,
}

impl CsvMeasurementRow {
    /// Create a CSV row from MeasurementData
    fn from_measurement(commit: &str, measurement: &MeasurementData) -> Self {
        let unit = config::measurement_unit(&measurement.name).unwrap_or_default();
        CsvMeasurementRow {
            commit: commit.to_string(),
            epoch: measurement.epoch,
            measurement: measurement.name.clone(),
            timestamp: measurement.timestamp,
            value: measurement.val,
            unit,
            metadata: measurement.key_values.clone(),
        }
    }

    /// Create a CSV row from MeasurementSummary
    fn from_summary(
        commit: &str,
        measurement_name: &str,
        summary: &MeasurementSummary,
        group_value: Option<&String>,
    ) -> Self {
        let unit = config::measurement_unit(measurement_name).unwrap_or_default();
        let mut metadata = HashMap::new();
        if let Some(gv) = group_value {
            metadata.insert("group".to_string(), gv.clone());
        }
        CsvMeasurementRow {
            commit: commit.to_string(),
            epoch: summary.epoch,
            measurement: measurement_name.to_string(),
            timestamp: 0.0,
            value: summary.val,
            unit,
            metadata,
        }
    }

    /// Format as a tab-delimited CSV line
    /// Float values are formatted to always include at least one decimal place
    fn to_csv_line(&self) -> String {
        // Format floats with appropriate precision
        // If value is a whole number, format as X.0, otherwise use default precision
        let value_str = if self.value.fract() == 0.0 && self.value.is_finite() {
            format!("{:.1}", self.value)
        } else {
            self.value.to_string()
        };

        let timestamp_str = if self.timestamp.fract() == 0.0 && self.timestamp.is_finite() {
            format!("{:.1}", self.timestamp)
        } else {
            self.timestamp.to_string()
        };

        let mut line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.commit, self.epoch, self.measurement, timestamp_str, value_str, self.unit
        );

        // Add metadata key-value pairs
        for (k, v) in &self.metadata {
            line.push('\t');
            line.push_str(k);
            line.push('=');
            line.push_str(v);
        }

        line
    }
}

trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);
    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        measurement_name: &str,
        group_values: &[String],
    );
    fn add_summarized_trace(
        &mut self,
        indexed_measurements: Vec<(usize, MeasurementSummary)>,
        measurement_name: &str,
        group_values: &[String],
    );
    fn add_epoch_boundaries(
        &mut self,
        transitions: &[EpochTransition],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    );
    fn add_change_points(
        &mut self,
        change_points: &[ChangePoint],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    );
    fn as_bytes(&self) -> Vec<u8>;
}

struct PlotlyReporter {
    plot: Plot,
    // Manual axis data reversal implementation: plotly-rs does not support autorange="reversed"
    // The autorange parameter only accepts boolean values (as of v0.13.5), requiring manual
    // index reversal to achieve reversed axis display (newest commits on right, oldest on left)
    // See: https://github.com/kaihowl/git-perf/issues/339
    size: usize,
    // Track units for all measurements to determine if we should add unit to Y-axis label
    measurement_units: Vec<Option<String>>,
}

impl PlotlyReporter {
    fn new() -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(true);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter {
            plot,
            size: 0,
            measurement_units: Vec::new(),
        }
    }

    fn convert_to_x_y(&self, indexed_measurements: Vec<(usize, f64)>) -> (Vec<usize>, Vec<f64>) {
        indexed_measurements
            .iter()
            .map(|(i, m)| (self.size - i - 1, *m))
            .unzip()
    }

    /// Returns the Y-axis with unit label if all measurements share the same unit.
    fn compute_y_axis(&self) -> Option<Axis> {
        // Check if all measurements have the same unit (and at least one unit exists)
        if self.measurement_units.is_empty() {
            return None;
        }

        let first_unit = self.measurement_units.first();
        let all_same_unit = self
            .measurement_units
            .iter()
            .all(|u| u == first_unit.unwrap());

        if all_same_unit {
            if let Some(Some(unit)) = first_unit {
                // All measurements share the same unit - add it to Y-axis label
                return Some(Axis::new().title(Title::from(format!("Value ({})", unit))));
            }
        }
        None
    }

    /// Helper function to add a vertical line segment to coordinate vectors.
    ///
    /// Adds two points (bottom and top of the line) plus a separator (None).
    fn add_vertical_line_segment(
        x_coords: &mut Vec<Option<usize>>,
        y_coords: &mut Vec<Option<f64>>,
        hover_texts: &mut Vec<String>,
        x_pos: usize,
        y_min: f64,
        y_max: f64,
        hover_text: String,
    ) {
        // Bottom point
        x_coords.push(Some(x_pos));
        y_coords.push(Some(y_min));
        hover_texts.push(hover_text.clone());

        // Top point
        x_coords.push(Some(x_pos));
        y_coords.push(Some(y_max));
        hover_texts.push(hover_text);

        // Separator (breaks the line for next segment)
        x_coords.push(None);
        y_coords.push(None);
        hover_texts.push(String::new());
    }

    /// Helper function to configure trace legend based on group values.
    ///
    /// If group_values is non-empty, uses group label with legend grouping.
    /// Otherwise, uses measurement display name directly.
    fn configure_trace_legend<X, Y>(
        trace: Box<Scatter<X, Y>>,
        group_values: &[String],
        measurement_name: &str,
        measurement_display: &str,
        label_suffix: &str,
        legend_group_suffix: &str,
    ) -> Box<Scatter<X, Y>>
    where
        X: serde::Serialize + Clone,
        Y: serde::Serialize + Clone,
    {
        if !group_values.is_empty() {
            let group_label = group_values.join("/");
            trace
                .name(format!("{} ({})", group_label, label_suffix))
                .legend_group(format!("{}_{}", measurement_name, legend_group_suffix))
                .legend_group_title(LegendGroupTitle::from(
                    format!("{} - {}", measurement_display, label_suffix).as_str(),
                ))
        } else {
            trace
                .name(format!("{} ({})", measurement_display, label_suffix))
                .legend_group(format!("{}_{}", measurement_name, legend_group_suffix))
        }
    }

    /// Add epoch boundary traces to the plot.
    ///
    /// These are vertical dashed gray lines where measurement epochs change.
    /// Hidden by default (legendonly), user clicks legend to toggle visibility.
    /// Uses actual commit indices to properly map epoch transitions when measurements
    /// don't exist for all commits.
    pub fn add_epoch_boundary_traces(
        &mut self,
        transitions: &[EpochTransition],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    ) {
        if transitions.is_empty() {
            return;
        }

        let mut x_coords: Vec<Option<usize>> = vec![];
        let mut y_coords: Vec<Option<f64>> = vec![];
        let mut hover_texts: Vec<String> = vec![];

        for transition in transitions {
            // Map the transition index (within measurement data) to the actual commit index
            if transition.index >= commit_indices.len() {
                log::warn!(
                    "Epoch transition index {} out of bounds for commit_indices len {}",
                    transition.index,
                    commit_indices.len()
                );
                continue;
            }
            let commit_idx = commit_indices[transition.index];
            let x_pos = self.size - commit_idx - 1;

            let hover_text = format!("Epoch {}→{}", transition.from_epoch, transition.to_epoch);

            Self::add_vertical_line_segment(
                &mut x_coords,
                &mut y_coords,
                &mut hover_texts,
                x_pos,
                y_min,
                y_max,
                hover_text,
            );
        }

        let measurement_display = format_measurement_with_unit(measurement_name);

        let trace = Scatter::new(x_coords, y_coords)
            .visible(Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color("gray").dash(DashType::Dash).width(2.0))
            .show_legend(true)
            .hover_text_array(hover_texts);

        let trace = Self::configure_trace_legend(
            trace,
            group_values,
            measurement_name,
            &measurement_display,
            "Epochs",
            "epochs",
        );

        self.plot.add_trace(trace);
    }

    /// Add change point traces with explicit commit index mapping.
    ///
    /// This version uses the actual commit indices to properly map change points
    /// when measurements don't exist for all commits.
    pub fn add_change_point_traces_with_indices(
        &mut self,
        change_points: &[ChangePoint],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    ) {
        let measurement_display = format_measurement_with_unit(measurement_name);

        for cp in change_points {
            if cp.index >= commit_indices.len() {
                log::warn!(
                    "Change point index {} out of bounds for commit_indices len {}",
                    cp.index,
                    commit_indices.len()
                );
                continue;
            }

            let (color, label_prefix, symbol) = match cp.direction {
                ChangeDirection::Increase => ("rgba(220, 53, 69, 0.8)", "Increase", "⚠ Regression"),
                ChangeDirection::Decrease => {
                    ("rgba(40, 167, 69, 0.8)", "Decrease", "✓ Improvement")
                }
            };

            let commit_idx = commit_indices[cp.index];
            let x_pos = self.size - commit_idx - 1;

            let hover_text = format!(
                "{}: {:+.1}%<br>Commit: {}<br>Confidence: {:.1}%",
                symbol,
                cp.magnitude_pct,
                &cp.commit_sha[..8.min(cp.commit_sha.len())],
                cp.confidence * 100.0
            );

            // Create vertical line from y_min to y_max
            let mut x_coords = vec![];
            let mut y_coords = vec![];
            let mut hover_texts = vec![];

            Self::add_vertical_line_segment(
                &mut x_coords,
                &mut y_coords,
                &mut hover_texts,
                x_pos,
                y_min,
                y_max,
                hover_text,
            );

            // Remove the trailing separator (None values) since we're creating one trace per change point
            x_coords.truncate(2);
            y_coords.truncate(2);
            hover_texts.truncate(2);

            let trace = Scatter::new(x_coords, y_coords)
                .visible(Visible::LegendOnly)
                .mode(Mode::Lines)
                .line(Line::new().color(color).width(3.0))
                .show_legend(false)
                .hover_text_array(hover_texts);

            let trace = Self::configure_trace_legend(
                trace,
                group_values,
                measurement_name,
                &measurement_display,
                label_prefix,
                "change_points",
            );

            self.plot.add_trace(trace);
        }
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
            .title(Title::from("Performance Measurements"))
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
        group_values: &[String],
    ) {
        let (x, y) = self.convert_to_x_y(
            indexed_measurements
                .into_iter()
                .map(|(i, m)| (i, m.val))
                .collect_vec(),
        );

        // Track unit for this measurement
        self.measurement_units
            .push(config::measurement_unit(measurement_name));

        let measurement_display = format_measurement_with_unit(measurement_name);

        let trace = plotly::BoxPlot::new_xy(x, y);

        let trace = if !group_values.is_empty() {
            // Join group values with "/" for display (only at display time)
            let group_label = group_values.join("/");
            trace
                .name(&group_label)
                .legend_group(measurement_name)
                .legend_group_title(LegendGroupTitle::from(measurement_display))
                .show_legend(true)
        } else {
            trace.name(&measurement_display)
        };

        self.plot.add_trace(trace);
    }

    fn add_summarized_trace(
        &mut self,
        indexed_measurements: Vec<(usize, MeasurementSummary)>,
        measurement_name: &str,
        group_values: &[String],
    ) {
        let (x, y) = self.convert_to_x_y(
            indexed_measurements
                .into_iter()
                .map(|(i, m)| (i, m.val))
                .collect_vec(),
        );

        // Track unit for this measurement
        self.measurement_units
            .push(config::measurement_unit(measurement_name));

        let measurement_display = format_measurement_with_unit(measurement_name);

        let trace = plotly::Scatter::new(x, y).name(&measurement_display);

        let trace = if !group_values.is_empty() {
            // Join group values with "/" for display (only at display time)
            let group_label = group_values.join("/");
            trace
                .name(&group_label)
                .legend_group(measurement_name)
                .legend_group_title(LegendGroupTitle::from(measurement_display))
                .show_legend(true)
        } else {
            trace.name(&measurement_display)
        };

        self.plot.add_trace(trace);
    }

    fn add_epoch_boundaries(
        &mut self,
        transitions: &[EpochTransition],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    ) {
        self.add_epoch_boundary_traces(
            transitions,
            commit_indices,
            measurement_name,
            group_values,
            y_min,
            y_max,
        );
    }

    fn add_change_points(
        &mut self,
        change_points: &[ChangePoint],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
        y_min: f64,
        y_max: f64,
    ) {
        self.add_change_point_traces_with_indices(
            change_points,
            commit_indices,
            measurement_name,
            group_values,
            y_min,
            y_max,
        );
    }

    fn as_bytes(&self) -> Vec<u8> {
        // If all measurements share the same unit, add it to Y-axis label
        if let Some(y_axis) = self.compute_y_axis() {
            let mut plot_with_y_axis = self.plot.clone();
            let mut layout = plot_with_y_axis.layout().clone();
            layout = layout.y_axis(y_axis);
            plot_with_y_axis.set_layout(layout);
            plot_with_y_axis.to_html().as_bytes().to_vec()
        } else {
            self.plot.to_html().as_bytes().to_vec()
        }
    }
}

struct CsvReporter<'a> {
    hashes: Vec<String>,
    indexed_measurements: Vec<(usize, &'a MeasurementData)>,
    summarized_measurements: Vec<(usize, String, Option<String>, MeasurementSummary)>,
}

impl CsvReporter<'_> {
    fn new() -> Self {
        CsvReporter {
            hashes: Vec::new(),
            indexed_measurements: Vec::new(),
            summarized_measurements: Vec::new(),
        }
    }
}

impl<'a> Reporter<'a> for CsvReporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]) {
        self.hashes = hashes.iter().map(|c| c.commit.to_owned()).collect();
    }

    fn add_trace(
        &mut self,
        indexed_measurements: Vec<(usize, &'a MeasurementData)>,
        _measurement_name: &str,
        _group_values: &[String],
    ) {
        self.indexed_measurements
            .extend_from_slice(indexed_measurements.as_slice());
    }

    fn as_bytes(&self) -> Vec<u8> {
        if self.indexed_measurements.is_empty() && self.summarized_measurements.is_empty() {
            return Vec::new();
        }

        let mut lines = Vec::new();

        // Add header
        lines.push("commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit".to_string());

        // Add raw measurements
        for (index, measurement_data) in &self.indexed_measurements {
            let commit = &self.hashes[*index];
            let row = CsvMeasurementRow::from_measurement(commit, measurement_data);
            lines.push(row.to_csv_line());
        }

        // Add summarized measurements
        for (index, measurement_name, group_value, summary) in &self.summarized_measurements {
            let commit = &self.hashes[*index];
            let row = CsvMeasurementRow::from_summary(
                commit,
                measurement_name,
                summary,
                group_value.as_ref(),
            );
            lines.push(row.to_csv_line());
        }

        let mut output = lines.join("\n");
        output.push('\n');
        output.into_bytes()
    }

    fn add_summarized_trace(
        &mut self,
        _indexed_measurements: Vec<(usize, MeasurementSummary)>,
        _measurement_name: &str,
        _group_values: &[String],
    ) {
        // Store summarized data to be serialized in as_bytes
        // Join group values for CSV output (flat format)
        let group_label = if !_group_values.is_empty() {
            Some(_group_values.join("/"))
        } else {
            None
        };

        for (index, summary) in _indexed_measurements.into_iter() {
            self.summarized_measurements.push((
                index,
                _measurement_name.to_string(),
                group_label.clone(),
                summary,
            ));
        }
    }

    fn add_epoch_boundaries(
        &mut self,
        _transitions: &[EpochTransition],
        _commit_indices: &[usize],
        _measurement_name: &str,
        _group_values: &[String],
        _y_min: f64,
        _y_max: f64,
    ) {
        // CSV reporter does not support epoch boundary visualization
    }

    fn add_change_points(
        &mut self,
        _change_points: &[ChangePoint],
        _commit_indices: &[usize],
        _measurement_name: &str,
        _group_values: &[String],
        _y_min: f64,
        _y_max: f64,
    ) {
        // CSV reporter does not support change point visualization
    }
}

struct ReporterFactory {}

impl ReporterFactory {
    fn from_file_name(path: &Path) -> Option<Box<dyn Reporter<'_> + '_>> {
        if path == Path::new("-") {
            return Some(Box::new(CsvReporter::new()) as Box<dyn Reporter<'_>>);
        }
        let mut res = None;
        if let Some(ext) = path.extension() {
            let extension = ext.to_ascii_lowercase().into_string().unwrap();
            res = match extension.as_str() {
                "html" => Some(Box::new(PlotlyReporter::new()) as Box<dyn Reporter<'_>>),
                "csv" => Some(Box::new(CsvReporter::new()) as Box<dyn Reporter<'_>>),
                _ => None,
            }
        }
        res
    }
}

#[allow(clippy::too_many_arguments)]
pub fn report(
    output: PathBuf,
    separate_by: Vec<String>,
    num_commits: usize,
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
    combined_patterns: &[String],
    show_epochs: bool,
    detect_changes: bool,
) -> Result<()> {
    // Compile combined regex patterns (measurements as exact matches + filter patterns)
    // early to fail fast on invalid patterns
    let filters = crate::filter::compile_filters(combined_patterns)?;

    let commits: Vec<Commit> = measurement_retrieval::walk_commits(num_commits)?.try_collect()?;

    if commits.is_empty() {
        bail!(
            "No commits found in repository. Ensure commits exist and were pushed to the remote."
        );
    }

    let mut plot =
        ReporterFactory::from_file_name(&output).ok_or(anyhow!("Could not infer output format"))?;

    plot.add_commits(&commits);

    let relevant = |m: &MeasurementData| {
        // Apply regex filters (handles both exact measurement matches and filter patterns)
        if !crate::filter::matches_any_filter(&m.name, &filters) {
            return false;
        }

        // Filter using subset relation: key_values ⊆ measurement.key_values
        m.key_values_is_superset_of(key_values)
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

        let group_values: Vec<Vec<String>> = if !separate_by.is_empty() {
            // Find all unique combinations of the split keys
            filtered_measurements
                .clone()
                .flatten()
                .filter_map(|m| {
                    // Extract values for all split keys
                    let values: Vec<String> = separate_by
                        .iter()
                        .filter_map(|key| m.key_values.get(key).cloned())
                        .collect();

                    // Only include if all split keys are present
                    if values.len() == separate_by.len() {
                        Some(values)
                    } else {
                        None
                    }
                })
                .unique()
                .collect_vec()
        } else {
            vec![]
        };

        // When no splits specified, create a single group with all measurements
        let group_values_to_process: Vec<Vec<String>> = if group_values.is_empty() {
            if !separate_by.is_empty() {
                bail!(
                    "Invalid separator supplied, no measurements have all required keys: {:?}",
                    separate_by
                );
            }
            vec![vec![]]
        } else {
            group_values
        };

        for group_value in group_values_to_process {
            let group_measurements = filtered_measurements.clone().map(|ms| {
                ms.filter(|m| {
                    if !group_value.is_empty() {
                        // Check if measurement has ALL the expected key-value pairs
                        separate_by
                            .iter()
                            .zip(group_value.iter())
                            .all(|(key, expected_val)| {
                                m.key_values
                                    .get(key)
                                    .map(|v| v == expected_val)
                                    .unwrap_or(false)
                            })
                    } else {
                        true
                    }
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

                plot.add_summarized_trace(trace_measurements, measurement_name, &group_value);
            } else {
                let trace_measurements: Vec<_> = group_measurements
                    .clone()
                    .enumerate()
                    .flat_map(|(i, ms)| ms.map(move |m| (i, m)))
                    .collect();

                plot.add_trace(trace_measurements, measurement_name, &group_value);
            }

            // Add change point detection for this measurement
            // Collect measurement values, epochs, commit indices and SHAs for change point detection
            // Note: We need the original commit index (i) to map back to the correct x-coordinate
            // IMPORTANT: We must aggregate multiple measurements per commit to get one value per commit
            // Otherwise, change point detection will see incorrect patterns
            let measurement_data: Vec<(usize, f64, u32, String)> = if aggregate_by.is_some() {
                // Already aggregated, use the same reduction function
                group_measurements
                    .clone()
                    .enumerate()
                    .flat_map(|(i, ms)| {
                        let commit_sha = commits[i].commit.clone();
                        ms.reduce_by(aggregate_by.unwrap())
                            .into_iter()
                            .map(move |m| (i, m.val, m.epoch, commit_sha.clone()))
                    })
                    .collect()
            } else {
                // No aggregation specified, use min (default) to get one value per commit
                group_measurements
                    .clone()
                    .enumerate()
                    .filter_map(|(i, ms)| {
                        let measurements: Vec<_> = ms.collect();
                        if measurements.is_empty() {
                            None
                        } else {
                            let commit_sha = commits[i].commit.clone();
                            // Use min for change point detection when multiple measurements exist
                            let min_val = measurements
                                .iter()
                                .map(|m| m.val)
                                .min_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap();
                            // Use the first measurement's epoch (they should all be the same)
                            let epoch = measurements[0].epoch;
                            Some((i, min_val, epoch, commit_sha))
                        }
                    })
                    .collect()
            };

            if measurement_data.len() >= 2 {
                let commit_indices: Vec<usize> =
                    measurement_data.iter().map(|(i, _, _, _)| *i).collect();
                let values: Vec<f64> = measurement_data.iter().map(|(_, v, _, _)| *v).collect();
                let epochs: Vec<u32> = measurement_data.iter().map(|(_, _, e, _)| *e).collect();
                let commit_shas: Vec<String> = measurement_data
                    .iter()
                    .map(|(_, _, _, s)| s.clone())
                    .collect();

                log::debug!(
                    "Change point detection for {}: {} measurements, indices {:?}, epochs {:?}",
                    measurement_name,
                    values.len(),
                    commit_indices,
                    epochs
                );

                // Calculate y-axis bounds for vertical lines
                let y_min = values.iter().cloned().fold(f64::INFINITY, f64::min) * 0.9;
                let y_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.1;

                // Add epoch boundary traces if requested
                if show_epochs {
                    // Reverse epochs and commit indices to match display order (newest on left, oldest on right)
                    let reversed_epochs: Vec<u32> = epochs.iter().rev().cloned().collect();
                    let reversed_commit_indices: Vec<usize> =
                        commit_indices.iter().rev().cloned().collect();
                    let transitions =
                        crate::change_point::detect_epoch_transitions(&reversed_epochs);
                    log::debug!(
                        "Epoch transitions for {}: {:?}",
                        measurement_name,
                        transitions
                    );
                    plot.add_epoch_boundaries(
                        &transitions,
                        &reversed_commit_indices,
                        measurement_name,
                        &group_value,
                        y_min,
                        y_max,
                    );
                }

                // Add change point traces if requested
                if detect_changes {
                    let config = crate::config::change_point_config(measurement_name);
                    // Reverse measurements to match display order (newest on left, oldest on right)
                    // This ensures change point direction (regression/improvement) matches visual interpretation
                    let reversed_values: Vec<f64> = values.iter().rev().cloned().collect();
                    let reversed_commit_shas: Vec<String> =
                        commit_shas.iter().rev().cloned().collect();
                    let reversed_commit_indices: Vec<usize> =
                        commit_indices.iter().rev().cloned().collect();
                    let raw_cps =
                        crate::change_point::detect_change_points(&reversed_values, &config);
                    log::debug!("Raw change points for {}: {:?}", measurement_name, raw_cps);
                    let enriched_cps = crate::change_point::enrich_change_points(
                        &raw_cps,
                        &reversed_values,
                        &reversed_commit_shas,
                        &config,
                    );
                    log::debug!(
                        "Enriched change points for {}: {:?}",
                        measurement_name,
                        enriched_cps
                    );
                    plot.add_change_points(
                        &enriched_cps,
                        &reversed_commit_indices,
                        measurement_name,
                        &group_value,
                        y_min,
                        y_max,
                    );
                }
            }
        }
    }

    // Write output
    let output_bytes = plot.as_bytes();

    if output == Path::new("-") {
        match io::stdout().write_all(&output_bytes) {
            Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
            res => res,
        }?;
    } else {
        File::create(&output)?.write_all(&output_bytes)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_x_y_empty() {
        let reporter = PlotlyReporter::new();
        let (x, y) = reporter.convert_to_x_y(vec![]);
        assert!(x.is_empty());
        assert!(y.is_empty());
    }

    #[test]
    fn test_convert_to_x_y_single_value() {
        let mut reporter = PlotlyReporter::new();
        reporter.size = 3;
        let (x, y) = reporter.convert_to_x_y(vec![(0, 1.5)]);
        assert_eq!(x, vec![2]);
        assert_eq!(y, vec![1.5]);
    }

    #[test]
    fn test_convert_to_x_y_multiple_values() {
        let mut reporter = PlotlyReporter::new();
        reporter.size = 5;
        let (x, y) = reporter.convert_to_x_y(vec![(0, 10.0), (2, 20.0), (4, 30.0)]);
        assert_eq!(x, vec![4, 2, 0]);
        assert_eq!(y, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_convert_to_x_y_negative_values() {
        let mut reporter = PlotlyReporter::new();
        reporter.size = 2;
        let (x, y) = reporter.convert_to_x_y(vec![(0, -5.5), (1, -10.2)]);
        assert_eq!(x, vec![1, 0]);
        assert_eq!(y, vec![-5.5, -10.2]);
    }

    #[test]
    fn test_plotly_reporter_as_bytes_not_empty() {
        let reporter = PlotlyReporter::new();
        let bytes = reporter.as_bytes();
        assert!(!bytes.is_empty());
        // HTML output should contain plotly-related content
        let html = String::from_utf8_lossy(&bytes);
        assert!(html.contains("plotly") || html.contains("Plotly"));
    }

    #[test]
    fn test_reporter_factory_html_extension() {
        let path = Path::new("output.html");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_some());
    }

    #[test]
    fn test_reporter_factory_csv_extension() {
        let path = Path::new("output.csv");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_some());
    }

    #[test]
    fn test_reporter_factory_stdout() {
        let path = Path::new("-");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_some());
    }

    #[test]
    fn test_reporter_factory_unsupported_extension() {
        let path = Path::new("output.txt");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_none());
    }

    #[test]
    fn test_reporter_factory_no_extension() {
        let path = Path::new("output");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_none());
    }

    #[test]
    fn test_reporter_factory_uppercase_extension() {
        let path = Path::new("output.HTML");
        let reporter = ReporterFactory::from_file_name(path);
        assert!(reporter.is_some());
    }

    #[test]
    fn test_format_measurement_with_unit_no_unit() {
        // Test measurement without unit configured
        let result = format_measurement_with_unit("unknown_measurement");
        assert_eq!(result, "unknown_measurement");
    }

    #[test]
    fn test_compute_y_axis_empty_measurements() {
        let reporter = PlotlyReporter::new();
        let y_axis = reporter.compute_y_axis();
        assert!(y_axis.is_none());
    }

    #[test]
    fn test_compute_y_axis_single_unit() {
        let mut reporter = PlotlyReporter::new();
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(Some("ms".to_string()));

        let y_axis = reporter.compute_y_axis();
        assert!(y_axis.is_some());
    }

    #[test]
    fn test_compute_y_axis_mixed_units() {
        let mut reporter = PlotlyReporter::new();
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(Some("bytes".to_string()));

        let y_axis = reporter.compute_y_axis();
        assert!(y_axis.is_none());
    }

    #[test]
    fn test_compute_y_axis_no_units() {
        let mut reporter = PlotlyReporter::new();
        reporter.measurement_units.push(None);
        reporter.measurement_units.push(None);

        let y_axis = reporter.compute_y_axis();
        assert!(y_axis.is_none());
    }

    #[test]
    fn test_compute_y_axis_some_with_unit_some_without() {
        let mut reporter = PlotlyReporter::new();
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(None);

        let y_axis = reporter.compute_y_axis();
        assert!(y_axis.is_none());
    }

    #[test]
    fn test_plotly_reporter_adds_units_to_legend() {
        use crate::data::Commit;

        let mut reporter = PlotlyReporter::new();

        // Add commits
        let commits = vec![
            Commit {
                commit: "abc123".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "def456".to_string(),
                measurements: vec![],
            },
        ];
        reporter.add_commits(&commits);

        // Add trace with a measurement (simulate tracking units)
        reporter.measurement_units.push(Some("ms".to_string()));

        // Get HTML output
        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);

        // The HTML should be generated
        assert!(!html.is_empty());
        assert!(html.contains("plotly") || html.contains("Plotly"));
    }

    #[test]
    fn test_plotly_reporter_y_axis_with_same_units() {
        let mut reporter = PlotlyReporter::new();

        // Simulate multiple measurements with same unit
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(Some("ms".to_string()));

        // Get HTML output - should include Y-axis with unit
        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);

        // The HTML should contain the Y-axis label with unit
        assert!(html.contains("Value (ms)"));
    }

    #[test]
    fn test_plotly_reporter_no_y_axis_with_mixed_units() {
        let mut reporter = PlotlyReporter::new();

        // Simulate measurements with different units
        reporter.measurement_units.push(Some("ms".to_string()));
        reporter.measurement_units.push(Some("bytes".to_string()));

        // Get HTML output - should NOT include Y-axis with unit
        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);

        // The HTML should not contain a Y-axis label with a specific unit
        assert!(!html.contains("Value (ms)"));
        assert!(!html.contains("Value (bytes)"));
    }

    #[test]
    fn test_csv_reporter_as_bytes_empty_on_init() {
        let reporter = CsvReporter::new();
        let bytes = reporter.as_bytes();
        // Empty reporter should produce empty bytes
        assert!(bytes.is_empty() || String::from_utf8_lossy(&bytes).trim().is_empty());
    }

    #[test]
    fn test_csv_reporter_includes_header() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        let mut reporter = CsvReporter::new();

        // Add commits
        let commits = vec![Commit {
            commit: "abc123".to_string(),
            measurements: vec![],
        }];
        reporter.add_commits(&commits);

        // Add a measurement
        let measurement = MeasurementData {
            epoch: 0,
            name: "test_measurement".to_string(),
            timestamp: 1234.0,
            val: 42.5,
            key_values: HashMap::new(),
        };
        reporter.add_trace(vec![(0, &measurement)], "test_measurement", &[]);

        // Get CSV output
        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        // Should contain header row with unit column
        assert!(csv.starts_with("commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\n"));

        // Should contain data row with commit and measurement data
        assert!(csv.contains("abc123"));
        assert!(csv.contains("test_measurement"));
        assert!(csv.contains("42.5"));
    }

    #[test]
    fn test_csv_exact_output_single_measurement() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        let mut reporter = CsvReporter::new();

        let commits = vec![Commit {
            commit: "abc123def456".to_string(),
            measurements: vec![],
        }];
        reporter.add_commits(&commits);

        let measurement = MeasurementData {
            epoch: 0,
            name: "build_time".to_string(),
            timestamp: 1234567890.5,
            val: 42.0,
            key_values: HashMap::new(),
        };
        reporter.add_trace(vec![(0, &measurement)], "build_time", &[]);

        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        let expected = "commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\nabc123def456\t0\tbuild_time\t1234567890.5\t42.0\t\n";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_csv_exact_output_with_metadata() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        let mut reporter = CsvReporter::new();

        let commits = vec![Commit {
            commit: "commit123".to_string(),
            measurements: vec![],
        }];
        reporter.add_commits(&commits);

        let mut metadata = HashMap::new();
        metadata.insert("os".to_string(), "linux".to_string());
        metadata.insert("arch".to_string(), "x64".to_string());

        let measurement = MeasurementData {
            epoch: 1,
            name: "test".to_string(),
            timestamp: 1000.0,
            val: 3.5,
            key_values: metadata,
        };
        reporter.add_trace(vec![(0, &measurement)], "test", &[]);

        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        // Check that header and base fields are correct
        assert!(csv.starts_with("commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\n"));
        assert!(csv.contains("commit123\t1\ttest\t1000.0\t3.5\t"));
        // Check that metadata is present (order may vary due to HashMap)
        assert!(csv.contains("os=linux"));
        assert!(csv.contains("arch=x64"));
        // Check trailing newline
        assert!(csv.ends_with('\n'));
    }

    #[test]
    fn test_csv_exact_output_multiple_measurements() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        let mut reporter = CsvReporter::new();

        let commits = vec![
            Commit {
                commit: "commit1".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "commit2".to_string(),
                measurements: vec![],
            },
        ];
        reporter.add_commits(&commits);

        let m1 = MeasurementData {
            epoch: 0,
            name: "timer".to_string(),
            timestamp: 100.0,
            val: 1.5,
            key_values: HashMap::new(),
        };

        let m2 = MeasurementData {
            epoch: 0,
            name: "timer".to_string(),
            timestamp: 200.0,
            val: 2.0,
            key_values: HashMap::new(),
        };

        reporter.add_trace(vec![(0, &m1), (1, &m2)], "timer", &[]);

        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        let expected = "commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\n\
                        commit1\t0\ttimer\t100.0\t1.5\t\n\
                        commit2\t0\ttimer\t200.0\t2.0\t\n";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_csv_exact_output_whole_number_formatting() {
        use crate::data::{Commit, MeasurementData};
        use std::collections::HashMap;

        let mut reporter = CsvReporter::new();

        let commits = vec![Commit {
            commit: "hash1".to_string(),
            measurements: vec![],
        }];
        reporter.add_commits(&commits);

        let measurement = MeasurementData {
            epoch: 0,
            name: "count".to_string(),
            timestamp: 500.0,
            val: 10.0,
            key_values: HashMap::new(),
        };
        reporter.add_trace(vec![(0, &measurement)], "count", &[]);

        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        // Whole numbers should be formatted with .0
        let expected =
            "commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\nhash1\t0\tcount\t500.0\t10.0\t\n";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_csv_exact_output_summarized_measurement() {
        use crate::data::{Commit, MeasurementSummary};

        let mut reporter = CsvReporter::new();

        let commits = vec![Commit {
            commit: "abc".to_string(),
            measurements: vec![],
        }];
        reporter.add_commits(&commits);

        let summary = MeasurementSummary { epoch: 0, val: 5.5 };

        reporter.add_summarized_trace(vec![(0, summary)], "avg_time", &[]);

        let bytes = reporter.as_bytes();
        let csv = String::from_utf8_lossy(&bytes);

        // Summarized measurements have timestamp 0.0
        let expected =
            "commit\tepoch\tmeasurement\ttimestamp\tvalue\tunit\nabc\t0\tavg_time\t0.0\t5.5\t\n";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_epoch_boundary_traces_hidden_by_default() {
        use crate::change_point::EpochTransition;
        use crate::data::Commit;

        let mut reporter = PlotlyReporter::new();

        let commits = vec![
            Commit {
                commit: "abc123".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "def456".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "ghi789".to_string(),
                measurements: vec![],
            },
        ];
        reporter.add_commits(&commits);

        let transitions = vec![EpochTransition {
            index: 1,
            from_epoch: 1,
            to_epoch: 2,
        }];

        let commit_indices = vec![0, 1, 2];
        let group_values: Vec<String> = vec![];
        reporter.add_epoch_boundary_traces(
            &transitions,
            &commit_indices,
            "test_metric",
            &group_values,
            0.0,
            100.0,
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Check that trace is set to legendonly (hidden by default)
        assert!(html.contains("legendonly"));
        // Check that the trace name includes "Epochs"
        assert!(html.contains("test_metric (Epochs)"));
    }

    #[test]
    fn test_epoch_boundary_traces_empty() {
        use crate::change_point::EpochTransition;

        let mut reporter = PlotlyReporter::new();
        reporter.size = 10;

        let transitions: Vec<EpochTransition> = vec![];
        let commit_indices: Vec<usize> = vec![];
        let group_values: Vec<String> = vec![];
        reporter.add_epoch_boundary_traces(
            &transitions,
            &commit_indices,
            "test",
            &group_values,
            0.0,
            100.0,
        );

        // Should not crash and plot should still be valid
        let bytes = reporter.as_bytes();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_change_point_traces_hidden_by_default() {
        use crate::change_point::{ChangeDirection, ChangePoint};
        use crate::data::Commit;

        let mut reporter = PlotlyReporter::new();

        let commits = vec![
            Commit {
                commit: "abc123".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "def456".to_string(),
                measurements: vec![],
            },
        ];
        reporter.add_commits(&commits);

        let change_points = vec![ChangePoint {
            index: 1,
            commit_sha: "def456".to_string(),
            magnitude_pct: 50.0,
            confidence: 0.9,
            direction: ChangeDirection::Increase,
        }];

        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &commit_indices,
            "build_time",
            &[],
            0.0,
            100.0,
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Check that trace is set to legendonly
        assert!(html.contains("legendonly"));
        // Check for change point trace (increase)
        assert!(html.contains("build_time (Increase)"));
    }

    #[test]
    fn test_change_point_traces_both_directions() {
        use crate::change_point::{ChangeDirection, ChangePoint};
        use crate::data::Commit;

        let mut reporter = PlotlyReporter::new();

        let commits: Vec<Commit> = (0..5)
            .map(|i| Commit {
                commit: format!("sha{:06}", i),
                measurements: vec![],
            })
            .collect();
        reporter.add_commits(&commits);

        let change_points = vec![
            ChangePoint {
                index: 2,
                commit_sha: "sha000002".to_string(),
                magnitude_pct: 25.0,
                confidence: 0.85,
                direction: ChangeDirection::Increase,
            },
            ChangePoint {
                index: 4,
                commit_sha: "sha000004".to_string(),
                magnitude_pct: -30.0,
                confidence: 0.90,
                direction: ChangeDirection::Decrease,
            },
        ];

        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &commit_indices,
            "metric",
            &[],
            0.0,
            100.0,
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Should have both increase and decrease traces
        assert!(html.contains("metric (Increase)"));
        assert!(html.contains("metric (Decrease)"));
    }

    #[test]
    fn test_change_point_traces_empty() {
        let mut reporter = PlotlyReporter::new();
        reporter.size = 10;

        let change_points: Vec<ChangePoint> = vec![];
        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &commit_indices,
            "test",
            &[],
            0.0,
            100.0,
        );

        // Should not crash and plot should still be valid
        let bytes = reporter.as_bytes();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_change_point_hover_text_format() {
        use crate::change_point::{ChangeDirection, ChangePoint};
        use crate::data::Commit;

        let mut reporter = PlotlyReporter::new();

        let commits = vec![
            Commit {
                commit: "abc123def".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "xyz789abc".to_string(),
                measurements: vec![],
            },
        ];
        reporter.add_commits(&commits);

        let change_points = vec![ChangePoint {
            index: 1,
            commit_sha: "xyz789abc".to_string(),
            magnitude_pct: 23.5,
            confidence: 0.88,
            direction: ChangeDirection::Increase,
        }];

        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &commit_indices,
            "test",
            &[],
            0.0,
            100.0,
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Hover text should contain percentage and short SHA
        assert!(html.contains("+23.5%"));
        assert!(html.contains("xyz789"));
    }
}
