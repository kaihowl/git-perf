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
#[derive(Clone)]
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
    show_changes: bool,
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

/// Load template from file or return default
fn load_template(template_path: &Path) -> Result<String> {
    if !template_path.exists() {
        bail!("Template file not found: {}", template_path.display());
    }

    let template_content = fs::read_to_string(template_path).map_err(|e| {
        anyhow!(
            "Failed to read template file {}: {}",
            template_path.display(),
            e
        )
    })?;

    Ok(template_content)
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

/// Write bytes to file or stdout, handling BrokenPipe error.
///
/// If `output_path` is "-", writes to stdout. Otherwise, creates/overwrites the file.
/// BrokenPipe errors are suppressed to allow piping to commands like `head` or `less`.
fn write_output(output_path: &Path, bytes: &[u8]) -> Result<()> {
    if output_path == Path::new("-") {
        // Write to stdout
        match io::stdout().write_all(bytes) {
            Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
            res => res,
        }
    } else {
        // Write to file
        File::create(output_path)?.write_all(bytes)
    }?;
    Ok(())
}

/// Formats a measurement name with its configured unit, if available.
/// Returns "measurement_name (unit)" if unit is configured, otherwise just "measurement_name".
fn format_measurement_with_unit(measurement_name: &str) -> String {
    match config::measurement_unit(measurement_name) {
        Some(unit) => format!("{} ({})", measurement_name, unit),
        None => measurement_name.to_string(),
    }
}

/// Output format for reports
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Html,
    Csv,
}

impl OutputFormat {
    /// Determine output format from file path
    fn from_path(path: &Path) -> Option<OutputFormat> {
        if path == Path::new("-") {
            return Some(OutputFormat::Csv);
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext_str| match ext_str.to_ascii_lowercase().as_str() {
                "html" => Some(OutputFormat::Html),
                "csv" => Some(OutputFormat::Csv),
                _ => None,
            })
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

/// Output from processing a single section.
///
/// For HTML reports, contains the plot div + script for template replacement.
/// For CSV reports, this is typically empty (CSV accumulates all data internally).
struct SectionOutput {
    #[allow(dead_code)]
    section_id: String,
    placeholder: String, // For HTML template replacement (e.g., "{{SECTION[id]}}")
    content: Vec<u8>,    // Section-specific content (plot HTML or empty for CSV)
}

trait Reporter<'a> {
    fn add_commits(&mut self, hashes: &'a [Commit]);

    // Section lifecycle methods
    fn begin_section(&mut self, section_id: &str, placeholder: &str);
    fn end_section(&mut self) -> Result<SectionOutput>;
    fn finalize(
        self: Box<Self>,
        sections: Vec<SectionOutput>,
        metadata: &ReportMetadata,
    ) -> Vec<u8>;

    // Data addition methods (section-scoped)
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

    // Deprecated: Use finalize() instead
    #[allow(dead_code)]
    fn as_bytes(&self) -> Vec<u8>;
}

struct PlotlyReporter {
    // Global state (set once via add_commits or with_template)
    all_commits: Vec<Commit>,
    // Manual axis data reversal implementation: plotly-rs does not support autorange="reversed"
    // The autorange parameter only accepts boolean values (as of v0.13.5), requiring manual
    // index reversal to achieve reversed axis display (newest commits on right, oldest on left)
    // See: https://github.com/kaihowl/git-perf/issues/339
    size: usize,
    template: Option<String>,
    #[allow(dead_code)]
    metadata: Option<ReportMetadata>,

    // Per-section state (reset on begin_section)
    current_section_id: Option<String>,
    current_placeholder: Option<String>,
    current_plot: Plot,
    // Track units for all measurements to determine if we should add unit to Y-axis label
    measurement_units: Vec<Option<String>>,
}

impl PlotlyReporter {
    #[allow(dead_code)]
    fn new() -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(false);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter {
            all_commits: Vec::new(),
            size: 0,
            template: None,
            metadata: None,
            current_section_id: None,
            current_placeholder: None,
            current_plot: plot,
            measurement_units: Vec::new(),
        }
    }

    fn with_template(template: String, metadata: ReportMetadata) -> PlotlyReporter {
        let config = Configuration::default().responsive(true).fill_frame(false);
        let mut plot = Plot::new();
        plot.set_configuration(config);
        PlotlyReporter {
            all_commits: Vec::new(),
            size: 0,
            template: Some(template),
            metadata: Some(metadata),
            current_section_id: None,
            current_placeholder: None,
            current_plot: plot,
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

        self.current_plot.add_trace(trace);
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

        self.current_plot.add_trace(trace);
    }
}

impl<'a> Reporter<'a> for PlotlyReporter {
    fn add_commits(&mut self, commits: &'a [Commit]) {
        // Store commits for later use in begin_section
        self.all_commits = commits.to_vec();
        self.size = commits.len();
    }

    fn begin_section(&mut self, section_id: &str, placeholder: &str) {
        self.current_section_id = Some(section_id.to_string());
        self.current_placeholder = Some(placeholder.to_string());

        // Create new plot for this section
        let config = Configuration::default().responsive(true).fill_frame(false);
        let mut plot = Plot::new();
        plot.set_configuration(config);

        // Set up layout with commit axis (from stored commits)
        let enumerated_commits = self.all_commits.iter().rev().enumerate();
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

        plot.set_layout(layout);
        self.current_plot = plot;
        self.measurement_units.clear();
    }

    fn end_section(&mut self) -> Result<SectionOutput> {
        let section_id = self
            .current_section_id
            .take()
            .ok_or_else(|| anyhow!("end_section called without begin_section"))?;

        let placeholder = self
            .current_placeholder
            .take()
            .ok_or_else(|| anyhow!("end_section called without placeholder"))?;

        // Finalize current plot (add y-axis if all units match)
        let final_plot = if let Some(y_axis) = self.compute_y_axis() {
            let mut plot_with_y_axis = self.current_plot.clone();
            let mut layout = plot_with_y_axis.layout().clone();
            layout = layout.y_axis(y_axis);
            plot_with_y_axis.set_layout(layout);
            plot_with_y_axis
        } else {
            self.current_plot.clone()
        };

        // Extract plotly body (just the div + script, no <html> wrapper)
        let (_plotly_head, plotly_body) = extract_plotly_parts(&final_plot);

        Ok(SectionOutput {
            section_id,
            placeholder,
            content: plotly_body.into_bytes(),
        })
    }

    fn finalize(
        self: Box<Self>,
        sections: Vec<SectionOutput>,
        metadata: &ReportMetadata,
    ) -> Vec<u8> {
        // If template is provided, use template replacement
        if let Some(template) = self.template {
            let mut output = template;

            // Replace section placeholders
            for section in &sections {
                output = output.replace(
                    &section.placeholder,
                    &String::from_utf8_lossy(&section.content),
                );
            }

            // Replace global placeholders
            let (plotly_head, _) = extract_plotly_parts(&Plot::new());
            output = output
                .replace("{{TITLE}}", &metadata.title)
                .replace("{{PLOTLY_HEAD}}", &plotly_head)
                .replace("{{CUSTOM_CSS}}", &metadata.custom_css)
                .replace("{{TIMESTAMP}}", &metadata.timestamp)
                .replace("{{COMMIT_RANGE}}", &metadata.commit_range)
                .replace("{{DEPTH}}", &metadata.depth.to_string())
                .replace("{{AUDIT_SECTION}}", "");

            output.into_bytes()
        } else {
            // No template - single section output (for backward compatibility)
            if sections.len() != 1 {
                panic!("Multiple sections require template");
            }
            sections[0].content.clone()
        }
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

        self.current_plot.add_trace(trace);
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

        self.current_plot.add_trace(trace);
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
            let mut plot_with_y_axis = self.current_plot.clone();
            let mut layout = plot_with_y_axis.layout().clone();
            layout = layout.y_axis(y_axis);
            plot_with_y_axis.set_layout(layout);
            plot_with_y_axis
        } else {
            self.current_plot.clone()
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

        // Apply template with placeholder substitution
        let (plotly_head, plotly_body) = extract_plotly_parts(&final_plot);
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

    fn begin_section(&mut self, _section_id: &str, _placeholder: &str) {
        // CSV doesn't care about section boundaries - no-op
    }

    fn end_section(&mut self) -> Result<SectionOutput> {
        // CSV returns empty SectionOutput - actual data stays in reporter
        // All data will be emitted in finalize()
        Ok(SectionOutput {
            section_id: "csv".to_string(),
            placeholder: String::new(),
            content: Vec::new(),
        })
    }

    fn finalize(
        self: Box<Self>,
        _sections: Vec<SectionOutput>,
        _metadata: &ReportMetadata,
    ) -> Vec<u8> {
        // Ignore sections parameter - CSV is flat
        // Generate single TSV output from accumulated data
        if self.indexed_measurements.is_empty() && self.summarized_measurements.is_empty() {
            return Vec::new();
        }

        let mut lines = Vec::new();
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

/// Compute group value combinations for splitting measurements by metadata keys.
///
/// Returns a vector of group values where each inner vector contains the values
/// for the split keys. If no splits are specified, returns a single empty group.
///
/// # Errors
/// Returns error if separate_by is non-empty but no measurements have all required keys
fn compute_group_values_to_process<'a>(
    filtered_measurements: impl Iterator<Item = &'a MeasurementData> + Clone,
    separate_by: &[String],
    context_id: &str, // For error messages (e.g., "Section 'test-overview'" or "measurement X")
) -> Result<Vec<Vec<String>>> {
    if separate_by.is_empty() {
        return Ok(vec![vec![]]);
    }

    let group_values: Vec<Vec<String>> = filtered_measurements
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

/// Filter measurements by regex patterns and key-value pairs.
///
/// This helper consolidates the filtering logic used by both HTML and CSV report paths.
/// Returns a nested vector where each inner vector contains filtered measurements for one commit.
///
/// # Arguments
/// * `commits` - The commits to filter measurements from
/// * `filters` - Compiled regex filters for measurement names (empty = no regex filtering)
/// * `key_values` - Key-value pairs that measurements must match
fn filter_measurements_by_criteria<'a>(
    commits: &'a [Commit],
    filters: &[regex::Regex],
    key_values: &[(String, String)],
) -> Vec<Vec<&'a MeasurementData>> {
    commits
        .iter()
        .map(|commit| {
            commit
                .measurements
                .iter()
                .filter(|m| {
                    // Apply regex filter if specified
                    if !filters.is_empty() && !crate::filter::matches_any_filter(&m.name, filters) {
                        return false;
                    }
                    // Apply key-value filters
                    m.key_values_is_superset_of(key_values)
                })
                .collect()
        })
        .collect()
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
fn add_change_point_and_epoch_traces(
    reporter: &mut dyn Reporter,
    params: ChangePointDetectionParams,
) {
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
    if params.show_changes {
        let config = crate::config::change_point_config(params.measurement_name);
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

/// Helper to add change point and epoch detection traces for a measurement group
///
/// This function encapsulates the common pattern of:
/// 1. Checking if change detection is requested
/// 2. Collecting measurement data for change detection
/// 3. Creating detection parameters
/// 4. Adding traces to the reporter
///
/// # Arguments
/// * `reporter` - Mutable reference to any Reporter implementation
/// * `group_measurements` - Iterator over measurements for this group
/// * `commits` - All commits (needed for change detection data collection)
/// * `measurement_name` - Name of the measurement being processed
/// * `group_value` - Group values for this specific group
/// * `aggregate_by` - Aggregation function to use
/// * `show_epochs` - Whether to show epoch annotations
/// * `show_changes` - Whether to detect and show change points
#[allow(clippy::too_many_arguments)]
fn add_detection_traces_if_requested<'a>(
    reporter: &mut dyn Reporter<'a>,
    group_measurements: impl Iterator<Item = impl Iterator<Item = &'a MeasurementData>> + Clone,
    commits: &[Commit],
    measurement_name: &str,
    group_value: &[String],
    aggregate_by: Option<ReductionFunc>,
    show_epochs: bool,
    show_changes: bool,
) {
    if !show_epochs && !show_changes {
        return;
    }

    let reduction_func = aggregate_by.unwrap_or(ReductionFunc::Min);

    let (commit_indices, values, epochs, commit_shas) =
        collect_measurement_data_for_change_detection(group_measurements, commits, reduction_func);

    let detection_params = ChangePointDetectionParams {
        commit_indices: &commit_indices,
        values: &values,
        epochs: &epochs,
        commit_shas: &commit_shas,
        measurement_name,
        group_values: group_value,
        show_epochs,
        show_changes,
    };

    add_change_point_and_epoch_traces(reporter, detection_params);
}

/// Wraps measurement filter patterns in non-capturing groups and joins them with |
/// This ensures correct precedence when combining multiple regex patterns
fn wrap_patterns_for_regex(patterns: &[String]) -> Option<String> {
    if patterns.is_empty() {
        None
    } else {
        Some(
            patterns
                .iter()
                .map(|p| format!("(?:{})", p))
                .collect::<Vec<_>>()
                .join("|"),
        )
    }
}

/// Builds a single-section config from CLI arguments
/// Used when template has no SECTION blocks (single-section mode)
fn build_single_section_config(
    combined_patterns: &[String],
    key_values: &[(String, String)],
    separate_by: Vec<String>,
    aggregate_by: Option<ReductionFunc>,
    show_epochs: bool,
    show_changes: bool,
) -> SectionConfig {
    SectionConfig {
        id: "main".to_string(),
        placeholder: "{{PLOTLY_BODY}}".to_string(),
        measurement_filter: wrap_patterns_for_regex(combined_patterns),
        key_value_filter: key_values.to_vec(),
        separate_by,
        aggregate_by,
        depth: None,
        show_epochs,
        show_changes,
    }
}

/// Merges global show flags with section-level flags using OR logic
/// Global flags override section flags (if global is true, result is true)
fn merge_show_flags(
    sections: Vec<SectionConfig>,
    global_show_epochs: bool,
    global_show_changes: bool,
) -> Vec<SectionConfig> {
    sections
        .into_iter()
        .map(|sc| SectionConfig {
            show_epochs: sc.show_epochs || global_show_epochs,
            show_changes: sc.show_changes || global_show_changes,
            ..sc
        })
        .collect()
}

/// Prepare sections, template, and metadata based on output format and configuration.
///
/// For HTML: Loads template, parses sections, creates metadata.
/// For CSV: Creates synthetic single section, minimal metadata.
///
/// Returns (sections, template_str, metadata).
#[allow(clippy::too_many_arguments)]
fn prepare_sections_and_metadata(
    output_format: OutputFormat,
    template_config: &ReportTemplateConfig,
    combined_patterns: &[String],
    key_values: &[(String, String)],
    separate_by: Vec<String>,
    aggregate_by: Option<ReductionFunc>,
    show_epochs: bool,
    show_changes: bool,
    commits: &[Commit],
) -> Result<(Vec<SectionConfig>, Option<String>, ReportMetadata)> {
    match output_format {
        OutputFormat::Html => {
            // Load template (custom or default)
            let template_path = template_config
                .template_path
                .clone()
                .or(config::report_template_path());
            let template_str = if let Some(path) = template_path {
                load_template(&path)?
            } else {
                DEFAULT_HTML_TEMPLATE.to_string()
            };

            // Parse or synthesize sections
            let sections = match parse_template_sections(&template_str)? {
                sections if sections.is_empty() => {
                    log::info!(
                        "Single-section template detected. Using CLI arguments for filtering/aggregation."
                    );
                    vec![build_single_section_config(
                        combined_patterns,
                        key_values,
                        separate_by,
                        aggregate_by,
                        show_epochs,
                        show_changes,
                    )]
                }
                sections => {
                    log::info!(
                        "Multi-section template detected with {} sections. CLI arguments for filtering/aggregation will be ignored.",
                        sections.len()
                    );
                    merge_show_flags(sections, show_epochs, show_changes)
                }
            };

            // Build metadata
            let resolved_title = template_config.title.clone().or_else(config::report_title);
            let custom_css_content = load_custom_css(template_config.custom_css_path.as_ref())?;
            let metadata = ReportMetadata::new(resolved_title, custom_css_content, commits);

            Ok((sections, Some(template_str), metadata))
        }
        OutputFormat::Csv => {
            // Warn if template provided
            if template_config.template_path.is_some() {
                log::warn!("Template argument is ignored for CSV output format");
            }

            // Create synthetic single section for CSV
            let section = build_single_section_config(
                combined_patterns,
                key_values,
                separate_by,
                aggregate_by,
                show_epochs,
                show_changes,
            );

            // CSV doesn't use metadata, but provide default for API consistency
            let metadata = ReportMetadata::new(None, String::new(), commits);

            Ok((vec![section], None, metadata))
        }
    }
}

/// Process a single section using the Reporter trait.
///
/// Calls reporter.begin_section(), filters measurements, processes groups,
/// and returns the section output via reporter.end_section().
fn process_section<'a>(
    reporter: &mut dyn Reporter<'a>,
    commits: &'a [Commit],
    section: &SectionConfig,
) -> Result<SectionOutput> {
    reporter.begin_section(&section.id, &section.placeholder);

    // Determine section-specific commits (depth override)
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

    // Filter measurements
    let filters = if let Some(ref pattern) = section.measurement_filter {
        crate::filter::compile_filters(std::slice::from_ref(pattern))?
    } else {
        vec![]
    };

    let relevant_measurements: Vec<Vec<&MeasurementData>> =
        filter_measurements_by_criteria(section_commits, &filters, &section.key_value_filter);

    let unique_measurement_names: Vec<_> = relevant_measurements
        .iter()
        .flat_map(|ms| ms.iter().map(|m| &m.name))
        .unique()
        .collect();

    if unique_measurement_names.is_empty() {
        log::warn!("Section '{}' has no matching measurements", section.id);
        // Return empty section output without generating plot content
        return Ok(SectionOutput {
            section_id: section.id.clone(),
            placeholder: section.placeholder.clone(),
            content: Vec::new(),
        });
    }

    // Process measurement groups (same logic as current generate_single_section_report)
    for measurement_name in unique_measurement_names {
        let filtered_for_grouping = relevant_measurements
            .iter()
            .flat_map(|ms| ms.iter().copied().filter(|m| m.name == *measurement_name));

        let group_values_to_process = compute_group_values_to_process(
            filtered_for_grouping,
            &section.separate_by,
            &format!("Section '{}'", section.id),
        )?;

        for group_value in group_values_to_process {
            let group_measurements_vec: Vec<Vec<&MeasurementData>> = relevant_measurements
                .iter()
                .map(|ms| {
                    ms.iter()
                        .filter(|m| {
                            if m.name != *measurement_name {
                                return false;
                            }
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

            // Add trace
            add_trace_for_measurement_group(
                reporter,
                group_measurements_vec.iter().map(|v| v.iter().copied()),
                measurement_name,
                &group_value,
                section.aggregate_by,
            );

            // Add detection traces
            add_detection_traces_if_requested(
                reporter,
                group_measurements_vec.iter().map(|v| v.iter().copied()),
                section_commits,
                measurement_name,
                &group_value,
                section.aggregate_by,
                section.show_epochs,
                section.show_changes,
            );
        }
    }

    reporter.end_section()
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
    show_changes: bool,
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

    // Determine output format
    let output_format = OutputFormat::from_path(&output)
        .ok_or_else(|| anyhow!("Could not determine output format from file extension"))?;

    // Parse or synthesize sections and metadata
    let (sections, template_str, metadata) = prepare_sections_and_metadata(
        output_format,
        &template_config,
        combined_patterns,
        key_values,
        separate_by.clone(),
        aggregate_by,
        show_epochs,
        show_changes,
        &commits,
    )?;

    // Create appropriate reporter
    let mut reporter: Box<dyn Reporter> = match output_format {
        OutputFormat::Html => {
            let template = template_str.expect("HTML requires template");
            Box::new(PlotlyReporter::with_template(template, metadata.clone()))
        }
        OutputFormat::Csv => Box::new(CsvReporter::new()),
    };

    // UNIFIED PATH: Process all sections using Reporter trait
    reporter.add_commits(&commits);

    let section_outputs = sections
        .iter()
        .map(|section| process_section(&mut *reporter, &commits, section))
        .collect::<Result<Vec<SectionOutput>>>()?;

    // Check if any section found measurements
    // For HTML: check if any section has non-empty content
    // For CSV: report_bytes will be empty if no measurements
    let has_measurements = match output_format {
        OutputFormat::Html => section_outputs.iter().any(|s| !s.content.is_empty()),
        OutputFormat::Csv => !section_outputs.is_empty(), // Will check after finalize
    };

    // Finalize report
    let report_bytes = reporter.finalize(section_outputs, &metadata);

    // Check if any measurements were found
    // For multi-section templates (>1 section), allow empty sections (just log warnings)
    // For single-section reports, bail if no measurements found
    let is_multi_section = sections.len() > 1;
    if !is_multi_section
        && ((output_format == OutputFormat::Html && !has_measurements)
            || (output_format == OutputFormat::Csv && report_bytes.is_empty()))
    {
        bail!("No performance measurements found.");
    }

    // Write output
    write_output(&output, &report_bytes)?;

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

    #[test]
    fn test_default_template_has_no_sections() {
        // The default template should not have sections
        // It should be a single-section template
        let sections = parse_template_sections(DEFAULT_HTML_TEMPLATE)
            .expect("Failed to parse default template");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_wrap_patterns_for_regex_empty() {
        // Test with empty patterns - should return None
        let patterns = vec![];
        let result = wrap_patterns_for_regex(&patterns);
        assert_eq!(result, None);
    }

    #[test]
    fn test_wrap_patterns_for_regex_single() {
        // Test with single pattern - should wrap in non-capturing group
        let patterns = vec!["test.*".to_string()];
        let result = wrap_patterns_for_regex(&patterns);
        assert_eq!(result, Some("(?:test.*)".to_string()));
    }

    #[test]
    fn test_wrap_patterns_for_regex_multiple() {
        // Test with multiple patterns - should wrap each and join with |
        let patterns = vec!["test.*".to_string(), "bench.*".to_string()];
        let result = wrap_patterns_for_regex(&patterns);
        assert_eq!(result, Some("(?:test.*)|(?:bench.*)".to_string()));
    }

    #[test]
    fn test_wrap_patterns_for_regex_complex() {
        // Test with complex regex patterns
        let patterns = vec!["^test-[0-9]+$".to_string(), "bench-(foo|bar)".to_string()];
        let result = wrap_patterns_for_regex(&patterns);
        assert_eq!(
            result,
            Some("(?:^test-[0-9]+$)|(?:bench-(foo|bar))".to_string())
        );
    }

    #[test]
    fn test_build_single_section_config_no_filters() {
        // Test building section config with no filters
        let section = build_single_section_config(&[], &[], vec![], None, false, false);

        assert_eq!(section.id, "main");
        assert_eq!(section.placeholder, "{{PLOTLY_BODY}}");
        assert_eq!(section.measurement_filter, None);
        assert!(section.key_value_filter.is_empty());
        assert!(section.separate_by.is_empty());
        assert_eq!(section.aggregate_by, None);
        assert_eq!(section.depth, None);
        assert!(!section.show_epochs);
        assert!(!section.show_changes);
    }

    #[test]
    fn test_build_single_section_config_with_patterns() {
        // Test building section config with measurement patterns
        let patterns = vec!["test.*".to_string(), "bench.*".to_string()];
        let section = build_single_section_config(&patterns, &[], vec![], None, false, false);

        assert_eq!(
            section.measurement_filter,
            Some("(?:test.*)|(?:bench.*)".to_string())
        );
    }

    #[test]
    fn test_build_single_section_config_with_all_params() {
        // Test building section config with all parameters
        let patterns = vec!["test.*".to_string()];
        let kv_filters = vec![
            ("os".to_string(), "linux".to_string()),
            ("arch".to_string(), "x64".to_string()),
        ];
        let separate = vec!["os".to_string(), "arch".to_string()];

        let section = build_single_section_config(
            &patterns,
            &kv_filters,
            separate.clone(),
            Some(ReductionFunc::Median),
            true,
            true,
        );

        assert_eq!(section.measurement_filter, Some("(?:test.*)".to_string()));
        assert_eq!(section.key_value_filter, kv_filters);
        assert_eq!(section.separate_by, separate);
        assert_eq!(section.aggregate_by, Some(ReductionFunc::Median));
        assert!(section.show_epochs);
        assert!(section.show_changes);
    }

    #[test]
    fn test_merge_show_flags_both_false() {
        // When both section and global flags are false, result should be false
        let sections = vec![SectionConfig {
            id: "test".to_string(),
            placeholder: "{{SECTION[test]}}".to_string(),
            measurement_filter: None,
            key_value_filter: vec![],
            separate_by: vec![],
            aggregate_by: None,
            depth: None,
            show_epochs: false,
            show_changes: false,
        }];

        let merged = merge_show_flags(sections, false, false);

        assert_eq!(merged.len(), 1);
        assert!(!merged[0].show_epochs);
        assert!(!merged[0].show_changes);
    }

    #[test]
    fn test_merge_show_flags_section_true_global_false() {
        // When section flag is true and global is false, result should be true (OR logic)
        let sections = vec![SectionConfig {
            id: "test".to_string(),
            placeholder: "{{SECTION[test]}}".to_string(),
            measurement_filter: None,
            key_value_filter: vec![],
            separate_by: vec![],
            aggregate_by: None,
            depth: None,
            show_epochs: true,
            show_changes: true,
        }];

        let merged = merge_show_flags(sections, false, false);

        assert_eq!(merged.len(), 1);
        assert!(merged[0].show_epochs);
        assert!(merged[0].show_changes);
    }

    #[test]
    fn test_merge_show_flags_section_false_global_true() {
        // When global flag is true and section is false, result should be true (OR logic)
        let sections = vec![SectionConfig {
            id: "test".to_string(),
            placeholder: "{{SECTION[test]}}".to_string(),
            measurement_filter: None,
            key_value_filter: vec![],
            separate_by: vec![],
            aggregate_by: None,
            depth: None,
            show_epochs: false,
            show_changes: false,
        }];

        let merged = merge_show_flags(sections, true, true);

        assert_eq!(merged.len(), 1);
        assert!(merged[0].show_epochs);
        assert!(merged[0].show_changes);
    }

    #[test]
    fn test_merge_show_flags_both_true() {
        // When both section and global flags are true, result should be true
        let sections = vec![SectionConfig {
            id: "test".to_string(),
            placeholder: "{{SECTION[test]}}".to_string(),
            measurement_filter: None,
            key_value_filter: vec![],
            separate_by: vec![],
            aggregate_by: None,
            depth: None,
            show_epochs: true,
            show_changes: true,
        }];

        let merged = merge_show_flags(sections, true, true);

        assert_eq!(merged.len(), 1);
        assert!(merged[0].show_epochs);
        assert!(merged[0].show_changes);
    }

    #[test]
    fn test_merge_show_flags_mixed_flags() {
        // Test with mixed flag combinations
        let sections = vec![SectionConfig {
            id: "test".to_string(),
            placeholder: "{{SECTION[test]}}".to_string(),
            measurement_filter: None,
            key_value_filter: vec![],
            separate_by: vec![],
            aggregate_by: None,
            depth: None,
            show_epochs: true,
            show_changes: false,
        }];

        let merged = merge_show_flags(sections, false, true);

        assert_eq!(merged.len(), 1);
        assert!(merged[0].show_epochs); // section true OR global false = true
        assert!(merged[0].show_changes); // section false OR global true = true
    }

    #[test]
    fn test_merge_show_flags_multiple_sections() {
        // Test merging flags for multiple sections
        let sections = vec![
            SectionConfig {
                id: "section1".to_string(),
                placeholder: "{{SECTION[section1]}}".to_string(),
                measurement_filter: None,
                key_value_filter: vec![],
                separate_by: vec![],
                aggregate_by: None,
                depth: None,
                show_epochs: false,
                show_changes: false,
            },
            SectionConfig {
                id: "section2".to_string(),
                placeholder: "{{SECTION[section2]}}".to_string(),
                measurement_filter: None,
                key_value_filter: vec![],
                separate_by: vec![],
                aggregate_by: None,
                depth: None,
                show_epochs: true,
                show_changes: false,
            },
        ];

        let merged = merge_show_flags(sections, true, true);

        assert_eq!(merged.len(), 2);
        // Both sections should have both flags true due to global flags
        assert!(merged[0].show_epochs);
        assert!(merged[0].show_changes);
        assert!(merged[1].show_epochs);
        assert!(merged[1].show_changes);
    }
}
