use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use anyhow::{bail, Result};
use chrono::Utc;
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
    reporting_config::{parse_template_sections, SectionConfig},
    stats::ReductionFunc,
};

// Re-export for backwards compatibility with CLI
pub use crate::reporting_config::ReportTemplateConfig;

/// Metadata for rendering report templates
struct ReportMetadata {
    title: String,
    custom_css: String,
    timestamp: String,
    commit_range: String,
    depth: usize,
}

impl ReportMetadata {
    fn new(
        title: Option<String>,
        custom_css_content: String,
        commits: &[Commit],
    ) -> ReportMetadata {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

        let commit_range = if commits.is_empty() {
            "No commits".to_string()
        } else if commits.len() == 1 {
            commits[0].commit[..7].to_string()
        } else {
            format!(
                "{}..{}",
                &commits.last().unwrap().commit[..7],
                &commits[0].commit[..7]
            )
        };

        let depth = commits.len();

        let default_title = "Performance Measurements".to_string();
        let title = title.unwrap_or(default_title);

        ReportMetadata {
            title,
            custom_css: custom_css_content,
            timestamp,
            commit_range,
            depth,
        }
    }
}

/// Parameters for change point and epoch detection
struct ChangePointDetectionParams<'a> {
    commit_indices: &'a [usize],
    values: &'a [f64],
    epochs: &'a [u32],
    commit_shas: &'a [String],
    measurement_name: &'a str,
    group_values: &'a [String],
    show_epochs: bool,
    detect_changes: bool,
}

/// Parameters for processing a measurement group
struct MeasurementGroupParams {
    group_value: Vec<String>,
    separate_by: Vec<String>,
    aggregate_by: Option<ReductionFunc>,
    show_epochs: bool,
    detect_changes: bool,
}

/// Extract Plotly JavaScript dependencies and plot content
///
/// Uses Plotly's native API to generate proper HTML components:
/// - `plotly_head`: Script tags for Plotly.js library (from CDN by default)
/// - `plotly_body`: Inline div + script for the actual plot content
///
/// This approach is more robust than HTML string parsing and leverages
/// Plotly's to_inline_html() method which generates embeddable content
/// assuming Plotly.js is already available on the page.
fn extract_plotly_parts(plot: &Plot) -> (String, String) {
    // Get the Plotly.js library script tags from CDN
    // This returns script tags that load plotly.min.js from CDN
    let plotly_head = Plot::online_cdn_js();

    // Get the inline plot HTML (div + script) without full HTML document
    // This assumes plotly.js is already loaded (which we handle via plotly_head)
    // Pass None to auto-generate a unique div ID
    let plotly_body = plot.to_inline_html(None);

    (plotly_head, plotly_body)
}

/// Apply template with placeholder substitution
fn apply_template(template: &str, plot: &Plot, metadata: &ReportMetadata) -> Vec<u8> {
    let (plotly_head, plotly_body) = extract_plotly_parts(plot);

    let output = template
        .replace("{{TITLE}}", &metadata.title)
        .replace("{{PLOTLY_HEAD}}", &plotly_head)
        .replace("{{PLOTLY_BODY}}", &plotly_body)
        .replace("{{CUSTOM_CSS}}", &metadata.custom_css)
        .replace("{{TIMESTAMP}}", &metadata.timestamp)
        .replace("{{COMMIT_RANGE}}", &metadata.commit_range)
        .replace("{{DEPTH}}", &metadata.depth.to_string())
        .replace("{{AUDIT_SECTION}}", ""); // Future enhancement

    output.as_bytes().to_vec()
}

/// Load template from file or return default
fn load_template(template_path: Option<&PathBuf>) -> Result<Option<String>> {
    let template_path = match template_path {
        Some(path) => path.clone(),
        None => {
            // Try config
            if let Some(config_path) = config::report_template_path() {
                config_path
            } else {
                // No template specified
                return Ok(None);
            }
        }
    };

    if !template_path.exists() {
        bail!("Template file not found: {}", template_path.display());
    }

    let template_content = fs::read_to_string(&template_path).map_err(|e| {
        anyhow!(
            "Failed to read template file {}: {}",
            template_path.display(),
            e
        )
    })?;

    Ok(Some(template_content))
}

/// Load custom CSS content from file
fn load_custom_css(custom_css_path: Option<&PathBuf>) -> Result<String> {
    let css_path = match custom_css_path {
        Some(path) => path.clone(),
        None => {
            // Try config
            if let Some(config_path) = config::report_custom_css_path() {
                config_path
            } else {
                // No custom CSS
                return Ok(String::new());
            }
        }
    };

    if !css_path.exists() {
        bail!("Custom CSS file not found: {}", css_path.display());
    }

    fs::read_to_string(&css_path).map_err(|e| {
        anyhow!(
            "Failed to read custom CSS file {}: {}",
            css_path.display(),
            e
        )
    })
}

/// Default number of characters to display from commit SHA in report x-axis.
///
/// This value is used when displaying commit hashes on the x-axis of plots,
/// optimized for display space and readability in interactive visualizations.
const DEFAULT_COMMIT_HASH_DISPLAY_LENGTH: usize = 6;

// Color constants for change point visualization
/// RGBA color for performance regressions (increases in metrics like execution time).
/// Red color with 80% opacity.
const REGRESSION_COLOR: &str = "rgba(220, 53, 69, 0.8)";

/// RGBA color for performance improvements (decreases in metrics like execution time).
/// Green color with 80% opacity.
const IMPROVEMENT_COLOR: &str = "rgba(40, 167, 69, 0.8)";

/// Color for epoch markers in the plot.
const EPOCH_MARKER_COLOR: &str = "gray";

// Line width constants for plot styling
/// Line width for epoch markers (vertical dashed lines).
const EPOCH_MARKER_LINE_WIDTH: f64 = 2.0;

/// Default HTML template used when no custom template is provided.
/// Replicates the behavior of plotly.rs's to_html() method while maintaining
/// consistency with the template-based approach.
const DEFAULT_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    <style>{{CUSTOM_CSS}}</style>
</head>
<body>
    {{PLOTLY_BODY}}
</body>
</html>"#;

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
        values: &[f64],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
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
    // Template and metadata for customized output
    template: Option<String>,
    metadata: Option<ReportMetadata>,
}

impl PlotlyReporter {
    fn new() -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(false);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter {
            plot,
            size: 0,
            measurement_units: Vec::new(),
            template: None,
            metadata: None,
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

    /// Helper to process a vertical marker (epoch or change point) and add its coordinates.
    ///
    /// Returns Ok(x_pos) if successful, Err if index out of bounds.
    fn process_vertical_marker(
        &self,
        index: usize,
        commit_indices: &[usize],
        measurement_name: &str,
        marker_type: &str,
    ) -> Result<usize, ()> {
        if index >= commit_indices.len() {
            log::warn!(
                "[{}] {} index {} out of bounds (max: {})",
                measurement_name,
                marker_type,
                index,
                commit_indices.len()
            );
            return Err(());
        }
        let commit_idx = commit_indices[index];
        let x_pos = self.size - commit_idx - 1;
        Ok(x_pos)
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
            let x_pos = match self.process_vertical_marker(
                transition.index,
                commit_indices,
                measurement_name,
                "Epoch transition",
            ) {
                Ok(pos) => pos,
                Err(()) => continue,
            };

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
            .line(
                Line::new()
                    .color(EPOCH_MARKER_COLOR)
                    .dash(DashType::Dash)
                    .width(EPOCH_MARKER_LINE_WIDTH),
            )
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
        values: &[f64],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
    ) {
        if change_points.is_empty() {
            return;
        }

        let measurement_display = format_measurement_with_unit(measurement_name);

        // Collect all change points into a single trace with markers
        let mut x_coords: Vec<usize> = vec![];
        let mut y_coords: Vec<f64> = vec![];
        let mut hover_texts: Vec<String> = vec![];
        let mut marker_colors: Vec<String> = vec![];

        for cp in change_points {
            let x_pos = match self.process_vertical_marker(
                cp.index,
                commit_indices,
                measurement_name,
                "Change point",
            ) {
                Ok(pos) => pos,
                Err(()) => continue,
            };

            // Get the actual y value from the measurement data
            let y_value = if cp.index < values.len() {
                values[cp.index]
            } else {
                log::warn!(
                    "Change point index {} out of bounds for values (len={})",
                    cp.index,
                    values.len()
                );
                continue;
            };

            let (color, symbol) = match cp.direction {
                ChangeDirection::Increase => (REGRESSION_COLOR, "⚠ Regression"),
                ChangeDirection::Decrease => (IMPROVEMENT_COLOR, "✓ Improvement"),
            };

            let hover_text = format!(
                "{}: {:+.1}%<br>Commit: {}<br>Confidence: {:.1}%",
                symbol,
                cp.magnitude_pct,
                &cp.commit_sha[..8.min(cp.commit_sha.len())],
                cp.confidence * 100.0
            );

            // Add single point at the actual measurement value
            x_coords.push(x_pos);
            y_coords.push(y_value);
            hover_texts.push(hover_text);
            marker_colors.push(color.to_string());
        }

        let trace = Scatter::new(x_coords, y_coords)
            .mode(Mode::Markers)
            .marker(
                plotly::common::Marker::new()
                    .color_array(marker_colors)
                    .size(12),
            )
            .show_legend(true)
            .hover_text_array(hover_texts);

        let trace = Self::configure_trace_legend(
            trace,
            group_values,
            measurement_name,
            &measurement_display,
            "Change Points",
            "change_points",
        );

        self.plot.add_trace(trace);
    }
}

impl<'a> Reporter<'a> for PlotlyReporter {
    fn add_commits(&mut self, commits: &'a [Commit]) {
        let enumerated_commits = commits.iter().rev().enumerate();
        self.size = commits.len();

        let (commit_nrs, short_hashes): (Vec<_>, Vec<_>) = enumerated_commits
            .map(|(n, c)| {
                (
                    n as f64,
                    c.commit[..DEFAULT_COMMIT_HASH_DISPLAY_LENGTH].to_owned(),
                )
            })
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
        values: &[f64],
        commit_indices: &[usize],
        measurement_name: &str,
        group_values: &[String],
    ) {
        self.add_change_point_traces_with_indices(
            change_points,
            values,
            commit_indices,
            measurement_name,
            group_values,
        );
    }

    fn as_bytes(&self) -> Vec<u8> {
        // Get the final plot (with or without custom y-axis)
        let final_plot = if let Some(y_axis) = self.compute_y_axis() {
            let mut plot_with_y_axis = self.plot.clone();
            let mut layout = plot_with_y_axis.layout().clone();
            layout = layout.y_axis(y_axis);
            plot_with_y_axis.set_layout(layout);
            plot_with_y_axis
        } else {
            self.plot.clone()
        };

        // Always use template approach for consistency
        // If no custom template is provided, use the default template
        let template = self.template.as_deref().unwrap_or(DEFAULT_HTML_TEMPLATE);

        // Use metadata if available, otherwise create a minimal default
        let default_metadata = ReportMetadata {
            title: "Performance Measurements".to_string(),
            custom_css: String::new(),
            timestamp: String::new(),
            commit_range: String::new(),
            depth: 0,
        };
        let metadata = self.metadata.as_ref().unwrap_or(&default_metadata);

        apply_template(template, &final_plot, metadata)
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
        _values: &[f64],
        _commit_indices: &[usize],
        _measurement_name: &str,
        _group_values: &[String],
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

/// Compute group value combinations for splitting measurements by metadata keys.
///
/// Returns a vector of group values where each inner vector contains the values
/// for the split keys. If no splits are specified, returns a single empty group.
///
/// # Errors
/// Returns error if separate_by is non-empty but no measurements have all required keys
fn compute_group_values_to_process<'a>(
    filtered_measurements: impl Iterator<Item = impl Iterator<Item = &'a MeasurementData>> + Clone,
    separate_by: &[String],
    context_id: &str, // For error messages (e.g., "Section 'test-overview'" or "measurement X")
) -> Result<Vec<Vec<String>>> {
    if separate_by.is_empty() {
        return Ok(vec![vec![]]);
    }

    let group_values: Vec<Vec<String>> = filtered_measurements
        .flatten()
        .filter_map(|m| {
            let values: Vec<String> = separate_by
                .iter()
                .filter_map(|key| m.key_values.get(key).cloned())
                .collect();

            if values.len() == separate_by.len() {
                Some(values)
            } else {
                None
            }
        })
        .unique()
        .collect();

    if group_values.is_empty() {
        bail!(
            "{}: Invalid separator supplied, no measurements have all required keys: {:?}",
            context_id,
            separate_by
        );
    }

    Ok(group_values)
}

/// Filter measurements that match all key-value pairs in the group.
///
/// If group_value is empty, returns all measurements (no filtering).
fn filter_group_measurements<'a>(
    measurements: impl Iterator<Item = &'a MeasurementData> + 'a,
    separate_by: Vec<String>,
    group_value: Vec<String>,
) -> Box<dyn Iterator<Item = &'a MeasurementData> + 'a> {
    Box::new(measurements.filter(move |m| {
        if group_value.is_empty() {
            return true;
        }

        separate_by
            .iter()
            .zip(group_value.iter())
            .all(|(key, expected_val)| {
                m.key_values
                    .get(key)
                    .map(|v| v == expected_val)
                    .unwrap_or(false)
            })
    }))
}

/// Collect and aggregate measurement data for change point detection.
///
/// Returns tuple of (commit_indices, values, epochs, commit_shas).
/// Each vector has one entry per commit with measurements.
fn collect_measurement_data_for_change_detection<'a>(
    group_measurements: impl Iterator<Item = impl Iterator<Item = &'a MeasurementData>> + Clone,
    commits: &[Commit],
    reduction_func: ReductionFunc,
) -> (Vec<usize>, Vec<f64>, Vec<u32>, Vec<String>) {
    let measurement_data: Vec<(usize, f64, u32, String)> = group_measurements
        .enumerate()
        .flat_map(|(i, ms)| {
            let commit_sha = commits[i].commit.clone();
            ms.reduce_by(reduction_func)
                .into_iter()
                .map(move |m| (i, m.val, m.epoch, commit_sha.clone()))
        })
        .collect();

    let commit_indices: Vec<usize> = measurement_data.iter().map(|(i, _, _, _)| *i).collect();
    let values: Vec<f64> = measurement_data.iter().map(|(_, v, _, _)| *v).collect();
    let epochs: Vec<u32> = measurement_data.iter().map(|(_, _, e, _)| *e).collect();
    let commit_shas: Vec<String> = measurement_data
        .iter()
        .map(|(_, _, _, s)| s.clone())
        .collect();

    (commit_indices, values, epochs, commit_shas)
}

/// Add a trace (line) to the plot for a measurement group.
///
/// If aggregate_by is Some, adds a summarized trace with aggregated values.
/// If aggregate_by is None, adds a raw trace with all individual measurements.
fn add_trace_for_measurement_group<'a>(
    reporter: &mut dyn Reporter<'a>,
    group_measurements: impl Iterator<Item = impl Iterator<Item = &'a MeasurementData>> + Clone,
    measurement_name: &str,
    group_value: &[String],
    aggregate_by: Option<ReductionFunc>,
) {
    if let Some(reduction_func) = aggregate_by {
        let trace_measurements = group_measurements
            .enumerate()
            .flat_map(move |(i, ms)| {
                ms.reduce_by(reduction_func)
                    .into_iter()
                    .map(move |m| (i, m))
            })
            .collect_vec();

        reporter.add_summarized_trace(trace_measurements, measurement_name, group_value);
    } else {
        let trace_measurements: Vec<_> = group_measurements
            .enumerate()
            .flat_map(|(i, ms)| ms.map(move |m| (i, m)))
            .collect();

        reporter.add_trace(trace_measurements, measurement_name, group_value);
    }
}

/// Add change point and epoch boundary annotations to a reporter.
///
/// This function handles:
/// - Data reversal for plotly's reversed axis display
/// - Epoch transition detection and visualization
/// - Change point detection with configurable config
/// - Adding visualization traces via the Reporter trait
///
/// Data is always reversed because plotly doesn't support reversed axes natively,
/// so we manually reverse the data to display newest commits on the right.
/// This ensures change point direction matches visual interpretation.
///
/// # Arguments
/// * `reporter` - Mutable reference to any Reporter implementation
/// * `params` - Detection parameters (indices, values, epochs, etc.)
/// * `change_point_config_fn` - Function to retrieve change point config
fn add_change_point_and_epoch_traces<F>(
    reporter: &mut dyn Reporter,
    params: ChangePointDetectionParams,
    change_point_config_fn: F,
) where
    F: Fn(&str) -> crate::change_point::ChangePointConfig,
{
    if params.values.is_empty() {
        return;
    }

    log::debug!(
        "Change point detection for {}: {} measurements, indices {:?}, epochs {:?}",
        params.measurement_name,
        params.values.len(),
        params.commit_indices,
        params.epochs
    );

    // Calculate y-axis bounds for vertical lines (10% padding)
    let y_min = params.values.iter().copied().fold(f64::INFINITY, f64::min) * 0.9;
    let y_max = params
        .values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        * 1.1;

    // Reverse data for display
    // Reversal is needed because plotly doesn't support reversed axes natively,
    // so we manually reverse the data to display newest commits on the right.
    // This ensures change point direction matches visual interpretation.
    let indices: Vec<usize> = params.commit_indices.iter().rev().copied().collect();
    let vals: Vec<f64> = params.values.iter().rev().copied().collect();
    let eps: Vec<u32> = params.epochs.iter().rev().copied().collect();
    let shas: Vec<String> = params.commit_shas.iter().rev().cloned().collect();

    // Add epoch boundary traces if requested
    if params.show_epochs {
        let transitions = crate::change_point::detect_epoch_transitions(&eps);
        log::debug!(
            "Epoch transitions for {}: {:?}",
            params.measurement_name,
            transitions
        );
        reporter.add_epoch_boundaries(
            &transitions,
            &indices,
            params.measurement_name,
            params.group_values,
            y_min,
            y_max,
        );
    }

    // Add change point traces if requested
    if params.detect_changes {
        let config = change_point_config_fn(params.measurement_name);
        let raw_cps = crate::change_point::detect_change_points(&vals, &config);
        log::debug!(
            "Raw change points for {}: {:?}",
            params.measurement_name,
            raw_cps
        );

        let enriched_cps =
            crate::change_point::enrich_change_points(&raw_cps, &vals, &shas, &config);
        log::debug!(
            "Enriched change points for {}: {:?}",
            params.measurement_name,
            enriched_cps
        );

        reporter.add_change_points(
            &enriched_cps,
            &vals,
            &indices,
            params.measurement_name,
            params.group_values,
        );
    }
}

/// Process a single measurement group: add trace and optionally detect changes/epochs.
///
/// This is a high-level orchestration function that combines multiple steps:
/// 1. Filter measurements for the specific group
/// 2. Add trace visualization
/// 3. Collect data for change detection
/// 4. Add change point and epoch annotations
///
/// # Arguments
/// * `reporter` - Mutable reference to any Reporter implementation
/// * `filtered_measurements` - Iterator over measurements for this measurement name
/// * `commits` - All commits (needed for change detection data collection)
/// * `measurement_name` - Name of the measurement being processed
/// * `params` - Measurement group parameters
/// * `change_point_config_fn` - Function to retrieve change point config
fn process_measurement_group<'a, F>(
    reporter: &mut dyn Reporter<'a>,
    filtered_measurements: impl Iterator<Item = impl Iterator<Item = &'a MeasurementData> + 'a>
        + Clone
        + 'a,
    commits: &[Commit],
    measurement_name: &str,
    params: MeasurementGroupParams,
    change_point_config_fn: F,
) where
    F: Fn(&str) -> crate::change_point::ChangePointConfig,
{
    // Step 1: Filter measurements for this specific group
    let sep_by = params.separate_by.clone();
    let grp_val = params.group_value.clone();
    let group_measurements = filtered_measurements
        .map(move |ms| filter_group_measurements(ms, sep_by.clone(), grp_val.clone()));

    // Step 2: Add trace visualization
    add_trace_for_measurement_group(
        reporter,
        group_measurements.clone(),
        measurement_name,
        &params.group_value,
        params.aggregate_by,
    );

    // Step 3: Add change points and epochs if requested
    if params.show_epochs || params.detect_changes {
        let reduction_func = params.aggregate_by.unwrap_or(ReductionFunc::Min);

        let (commit_indices, values, epochs, commit_shas) =
            collect_measurement_data_for_change_detection(
                group_measurements,
                commits,
                reduction_func,
            );

        let detection_params = ChangePointDetectionParams {
            commit_indices: &commit_indices,
            values: &values,
            epochs: &epochs,
            commit_shas: &commit_shas,
            measurement_name,
            group_values: &params.group_value,
            show_epochs: params.show_epochs,
            detect_changes: params.detect_changes,
        };

        add_change_point_and_epoch_traces(reporter, detection_params, change_point_config_fn);
    }
}

/// Generate a plot for a single section with specific configuration
#[allow(clippy::too_many_arguments)]
fn generate_section_plot(
    commits: &[Commit],
    section: &SectionConfig,
    global_show_epochs: bool,
    global_detect_changes: bool,
) -> Result<Plot> {
    let mut reporter = PlotlyReporter::new();
    reporter.add_commits(commits);

    // Determine the number of commits to use for this section
    let section_commits = if let Some(depth) = section.depth {
        if depth > commits.len() {
            log::warn!(
                "Section '{}' requested depth {} but only {} commits available",
                section.id,
                depth,
                commits.len()
            );
            commits
        } else {
            &commits[..depth]
        }
    } else {
        commits
    };

    // Compile filter pattern if specified
    let filters = if let Some(ref pattern) = section.measurement_filter {
        crate::filter::compile_filters(std::slice::from_ref(pattern))?
    } else {
        vec![]
    };

    // Filter measurements and collect to avoid iterator cloning
    let relevant_measurements: Vec<Vec<&MeasurementData>> = section_commits
        .iter()
        .map(|commit| {
            commit
                .measurements
                .iter()
                .filter(|m| {
                    // Apply regex filter if specified
                    if !filters.is_empty() && !crate::filter::matches_any_filter(&m.name, &filters)
                    {
                        return false;
                    }
                    // Apply key-value filters
                    m.key_values_is_superset_of(&section.key_value_filter)
                })
                .collect()
        })
        .collect();

    let unique_measurement_names: Vec<_> = relevant_measurements
        .iter()
        .flat_map(|ms| ms.iter().map(|m| &m.name))
        .unique()
        .collect();

    if unique_measurement_names.is_empty() {
        log::warn!("Section '{}' has no matching measurements", section.id);
        // Return an empty plot
        return Ok(reporter.plot);
    }

    // Determine epoch and change point settings for this section
    let show_epochs = section.show_epochs || global_show_epochs;
    let detect_changes = section.detect_changes || global_detect_changes;

    for measurement_name in unique_measurement_names {
        // Get group values first
        let filtered_for_grouping = relevant_measurements
            .iter()
            .map(|ms| ms.iter().filter(|m| m.name == *measurement_name).copied());

        let group_values_to_process = compute_group_values_to_process(
            filtered_for_grouping,
            &section.separate_by,
            &format!("Section '{}'", section.id),
        )?;

        for group_value in group_values_to_process {
            // Filter and collect measurements directly from relevant_measurements
            let group_measurements: Vec<Vec<&MeasurementData>> = relevant_measurements
                .iter()
                .map(|ms| {
                    ms.iter()
                        .filter(|m| {
                            // Filter by measurement name
                            if m.name != *measurement_name {
                                return false;
                            }
                            // Filter by group value
                            if group_value.is_empty() {
                                return true;
                            }
                            section.separate_by.iter().zip(group_value.iter()).all(
                                |(key, expected_val)| {
                                    m.key_values
                                        .get(key)
                                        .map(|v| v == expected_val)
                                        .unwrap_or(false)
                                },
                            )
                        })
                        .copied()
                        .collect()
                })
                .collect();

            let params = MeasurementGroupParams {
                group_value,
                separate_by: section.separate_by.clone(),
                aggregate_by: section.aggregate_by,
                show_epochs,
                detect_changes,
            };

            process_measurement_group(
                &mut reporter as &mut dyn Reporter,
                group_measurements.iter().map(|v| v.iter().copied()),
                section_commits,
                measurement_name,
                params,
                |_| crate::change_point::ChangePointConfig::default(),
            );
        }
    }

    Ok(reporter.plot)
}

/// Generate a multi-section report from a template with section placeholders
fn generate_multi_section_report(
    template: &str,
    commits: &[Commit],
    metadata: &ReportMetadata,
    global_show_epochs: bool,
    global_detect_changes: bool,
) -> Result<Vec<u8>> {
    let sections = parse_template_sections(template)?;

    if sections.is_empty() {
        // No sections found - this shouldn't happen if called correctly
        bail!("Template contains no section placeholders");
    }

    log::info!(
        "Generating multi-section report with {} sections",
        sections.len()
    );

    let mut output = template.to_string();

    // Generate each section
    for section in sections {
        log::info!("Generating section: {}", section.id);

        // Generate the plot for this section
        let plot =
            generate_section_plot(commits, &section, global_show_epochs, global_detect_changes)?;

        // Extract plotly parts
        let (_plotly_head, plotly_body) = extract_plotly_parts(&plot);

        // For sections, we only want the body (the actual plot div + script)
        // The head (plotly.js library) will be included once in the global template
        // Replace the section placeholder with just the plotly body
        output = output.replace(&section.placeholder, &plotly_body);
    }

    // Now apply global placeholders
    let (plotly_head, _) = extract_plotly_parts(&Plot::new()); // Get just the plotly.js script tags

    output = output
        .replace("{{TITLE}}", &metadata.title)
        .replace("{{PLOTLY_HEAD}}", &plotly_head)
        .replace("{{CUSTOM_CSS}}", &metadata.custom_css)
        .replace("{{TIMESTAMP}}", &metadata.timestamp)
        .replace("{{COMMIT_RANGE}}", &metadata.commit_range)
        .replace("{{DEPTH}}", &metadata.depth.to_string())
        .replace("{{AUDIT_SECTION}}", ""); // Future enhancement

    Ok(output.as_bytes().to_vec())
}

#[allow(clippy::too_many_arguments)]
pub fn report(
    output: PathBuf,
    separate_by: Vec<String>,
    num_commits: usize,
    key_values: &[(String, String)],
    aggregate_by: Option<ReductionFunc>,
    combined_patterns: &[String],
    template_config: ReportTemplateConfig,
    show_epochs: bool,
    detect_changes: bool,
) -> Result<()> {
    // Compile combined regex patterns (measurements as exact matches + filter patterns)
    // early to fail fast on invalid patterns
    let _filters = crate::filter::compile_filters(combined_patterns)?;

    let commits: Vec<Commit> = measurement_retrieval::walk_commits(num_commits)?.try_collect()?;

    if commits.is_empty() {
        bail!(
            "No commits found in repository. Ensure commits exist and were pushed to the remote."
        );
    }

    let mut plot =
        ReporterFactory::from_file_name(&output).ok_or(anyhow!("Could not infer output format"))?;

    plot.add_commits(&commits);

    // For HTML reports, always use the template-based approach (single or multi-section)
    if output.extension().and_then(|s| s.to_str()) == Some("html") {
        let template = load_template(template_config.template_path.as_ref())?;

        // Resolve title: CLI > config > None
        let resolved_title = template_config.title.or_else(config::report_title);

        let custom_css_content = load_custom_css(template_config.custom_css_path.as_ref())?;
        let metadata = ReportMetadata::new(resolved_title, custom_css_content, &commits);

        // Determine the template to use (custom or default)
        let template_str = template.as_deref().unwrap_or(DEFAULT_HTML_TEMPLATE);

        // Check if template contains section placeholders
        let sections = parse_template_sections(template_str)?;

        let report_bytes = if !sections.is_empty() {
            // Multi-section template - use generate_multi_section_report
            log::info!(
                "Multi-section template detected with {} sections. CLI arguments for filtering/aggregation will be ignored.",
                sections.len()
            );

            generate_multi_section_report(
                template_str,
                &commits,
                &metadata,
                show_epochs,
                detect_changes,
            )?
        } else {
            // Single-section template (or no template/default template)
            // Create a synthetic section from CLI arguments
            let single_section = SectionConfig {
                id: "main".to_string(),
                placeholder: "{{PLOTLY_BODY}}".to_string(),
                measurement_filter: if combined_patterns.is_empty() {
                    None
                } else {
                    Some(combined_patterns.join("|"))
                },
                key_value_filter: key_values.to_vec(),
                separate_by: separate_by.clone(),
                aggregate_by,
                depth: Some(num_commits),
                show_epochs,
                detect_changes,
            };

            // Generate the single plot
            let plot =
                generate_section_plot(&commits, &single_section, show_epochs, detect_changes)?;

            // Apply template rendering
            let (_plotly_head, plotly_body) = extract_plotly_parts(&plot);
            let (plotly_head, _) = extract_plotly_parts(&Plot::new());

            let output = template_str
                .replace("{{TITLE}}", &metadata.title)
                .replace("{{PLOTLY_HEAD}}", &plotly_head)
                .replace("{{PLOTLY_BODY}}", &plotly_body)
                .replace("{{CUSTOM_CSS}}", &metadata.custom_css)
                .replace("{{TIMESTAMP}}", &metadata.timestamp)
                .replace("{{COMMIT_RANGE}}", &metadata.commit_range)
                .replace("{{DEPTH}}", &metadata.depth.to_string())
                .replace("{{AUDIT_SECTION}}", "");

            output.as_bytes().to_vec()
        };

        // Write output
        if output == Path::new("-") {
            match io::stdout().write_all(&report_bytes) {
                Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
                res => res,
            }?;
        } else {
            File::create(&output)?.write_all(&report_bytes)?;
        }

        return Ok(());
    }

    // CSV output path - use the old Reporter trait approach
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
    fn test_plotly_reporter_uses_default_template() {
        let reporter = PlotlyReporter::new();
        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);

        // Verify default template structure is present
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html>"));
        assert!(html.contains("<head>"));
        assert!(html.contains("<title>Performance Measurements</title>"));
        assert!(html.contains("</head>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("</body>"));
        assert!(html.contains("</html>"));
        // Verify plotly content is embedded
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
    fn test_extract_plotly_parts() {
        // Create a simple plot
        let mut plot = Plot::new();
        let trace = plotly::Scatter::new(vec![1, 2, 3], vec![4, 5, 6]).name("test");
        plot.add_trace(trace);

        let (head, body) = extract_plotly_parts(&plot);

        // Head should contain script tags for plotly.js from CDN
        assert!(head.contains("<script"));
        assert!(head.contains("plotly"));

        // Body should contain the plot div and script
        assert!(body.contains("<div"));
        assert!(body.contains("<script"));
        assert!(body.contains("Plotly.newPlot"));
    }

    #[test]
    fn test_extract_plotly_parts_structure() {
        // Verify the structure of extracted parts
        let mut plot = Plot::new();
        let trace = plotly::Scatter::new(vec![1], vec![1]).name("data");
        plot.add_trace(trace);

        let (head, body) = extract_plotly_parts(&plot);

        // Head should be CDN script tags only (no full HTML structure)
        assert!(!head.contains("<html>"));
        assert!(!head.contains("<head>"));
        assert!(!head.contains("<body>"));

        // Body should be inline content (div + script), not full HTML
        assert!(!body.contains("<html>"));
        assert!(!body.contains("<head>"));
        assert!(!body.contains("<body>"));
    }

    #[test]
    fn test_apply_template_basic() {
        let template = r#"<html><head>{{PLOTLY_HEAD}}</head><body><h1>{{TITLE}}</h1>{{PLOTLY_BODY}}</body></html>"#;

        // Create a simple plot
        let mut plot = Plot::new();
        let trace = plotly::Scatter::new(vec![1, 2], vec![3, 4]).name("test");
        plot.add_trace(trace);

        let metadata = ReportMetadata {
            title: "Test Report".to_string(),
            custom_css: "".to_string(),
            timestamp: "2024-01-01 00:00:00 UTC".to_string(),
            commit_range: "abc123".to_string(),
            depth: 1,
        };

        let result = apply_template(template, &plot, &metadata);
        let result_str = String::from_utf8(result).unwrap();

        assert!(result_str.contains("Test Report"));
        // Should contain plotly script tags in head
        assert!(result_str.contains("<script"));
        assert!(result_str.contains("plotly"));
        // Should contain plot div and script in body
        assert!(result_str.contains("<div"));
        assert!(result_str.contains("Plotly.newPlot"));
    }

    #[test]
    fn test_apply_template_all_placeholders() {
        let template = r#"
            <html>
            <head>
                <title>{{TITLE}}</title>
                {{PLOTLY_HEAD}}
                <style>{{CUSTOM_CSS}}</style>
            </head>
            <body>
                <h1>{{TITLE}}</h1>
                <p>Generated: {{TIMESTAMP}}</p>
                <p>Commits: {{COMMIT_RANGE}}</p>
                <p>Depth: {{DEPTH}}</p>
                {{PLOTLY_BODY}}
                {{AUDIT_SECTION}}
            </body>
            </html>
        "#;

        // Create a simple plot
        let mut plot = Plot::new();
        let trace = plotly::Scatter::new(vec![1, 2, 3], vec![10, 20, 30]).name("chart");
        plot.add_trace(trace);

        let metadata = ReportMetadata {
            title: "Performance Report".to_string(),
            custom_css: "body { color: red; }".to_string(),
            timestamp: "2024-01-15 12:00:00 UTC".to_string(),
            commit_range: "abc123..def456".to_string(),
            depth: 50,
        };

        let result = apply_template(template, &plot, &metadata);
        let result_str = String::from_utf8(result).unwrap();

        assert!(result_str.contains("Performance Report"));
        assert!(result_str.contains("body { color: red; }"));
        assert!(result_str.contains("2024-01-15 12:00:00 UTC"));
        assert!(result_str.contains("abc123..def456"));
        assert!(result_str.contains("50"));
        // Should contain plotly script tags and plot content
        assert!(result_str.contains("<script"));
        assert!(result_str.contains("plotly"));
        assert!(result_str.contains("<div"));
    }

    #[test]
    fn test_report_metadata_new() {
        use crate::data::Commit;

        let commits = vec![
            Commit {
                commit: "abc1234567890".to_string(),
                measurements: vec![],
            },
            Commit {
                commit: "def0987654321".to_string(),
                measurements: vec![],
            },
        ];

        let metadata =
            ReportMetadata::new(Some("Custom Title".to_string()), "".to_string(), &commits);

        assert_eq!(metadata.title, "Custom Title");
        assert_eq!(metadata.commit_range, "def0987..abc1234");
        assert_eq!(metadata.depth, 2);
    }

    #[test]
    fn test_report_metadata_new_default_title() {
        use crate::data::Commit;

        let commits = vec![Commit {
            commit: "abc1234567890".to_string(),
            measurements: vec![],
        }];

        let metadata = ReportMetadata::new(None, "".to_string(), &commits);

        assert_eq!(metadata.title, "Performance Measurements");
        assert_eq!(metadata.commit_range, "abc1234");
        assert_eq!(metadata.depth, 1);
    }

    #[test]
    fn test_report_metadata_new_empty_commits() {
        let commits = vec![];
        let metadata = ReportMetadata::new(None, "".to_string(), &commits);

        assert_eq!(metadata.commit_range, "No commits");
        assert_eq!(metadata.depth, 0);
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

        let values = vec![50.0, 75.0]; // Measurement values
        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &values,
            &commit_indices,
            "build_time",
            &[],
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Check for change point trace (single trace for all change points)
        assert!(html.contains("build_time (Change Points)"));
        // Verify markers mode is used
        assert!(html.contains("\"mode\":\"markers\""));
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

        let values = vec![50.0, 55.0, 62.5, 60.0, 42.0]; // Measurement values
        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &values,
            &commit_indices,
            "metric",
            &[],
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Should have single change points trace containing both directions
        assert!(html.contains("metric (Change Points)"));
        // Verify both regression and improvement symbols are present in hover text
        assert!(html.contains("⚠ Regression"));
        assert!(html.contains("✓ Improvement"));
    }

    #[test]
    fn test_change_point_traces_empty() {
        let mut reporter = PlotlyReporter::new();
        reporter.size = 10;

        let change_points: Vec<ChangePoint> = vec![];
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &values,
            &commit_indices,
            "test",
            &[],
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

        let values = vec![100.0, 123.5]; // Measurement values
        let commit_indices: Vec<usize> = (0..reporter.size).collect();
        reporter.add_change_point_traces_with_indices(
            &change_points,
            &values,
            &commit_indices,
            "test",
            &[],
        );

        let bytes = reporter.as_bytes();
        let html = String::from_utf8_lossy(&bytes);
        // Hover text should contain percentage and short SHA
        assert!(html.contains("+23.5%"));
        assert!(html.contains("xyz789"));
    }
}
