//! Demonstration of change point detection visualization in HTML reports.
//!
//! This example creates a fake repository scenario with constructed change points
//! and generates an HTML report showing how they appear.
//!
//! Run with: cargo run --example change_point_demo

use git_perf::change_point::{
    detect_change_points, detect_epoch_transitions, enrich_change_points, ChangePointConfig,
};
use plotly::{
    common::{DashType, Font, Line, Mode, Title, Visible},
    layout::{Axis, Legend},
    Configuration, Layout, Plot, Scatter,
};
use std::fs::File;
use std::io::Write;

fn main() {
    println!("Creating change point detection demo report...");

    // Simulate 30 commits worth of build time measurements
    // Scenario: Initial baseline ~10s, regression to ~15s at commit 10, improvement to ~12s at commit 20
    let measurements = vec![
        10.2, 9.8, 10.1, 10.3, 9.9, 10.0, 10.1, 9.7, 10.4, 10.0, // Commits 0-9: ~10s baseline
        15.1, 14.9, 15.3, 15.0, 14.8, 15.2, 15.1, 14.7, 15.4, 15.0, // Commits 10-19: ~15s (50% regression)
        12.0, 11.9, 12.2, 12.1, 11.8, 12.3, 12.0, 11.7, 12.1, 12.0, // Commits 20-29: ~12s (20% improvement)
    ];

    // Generate fake commit SHAs
    let commit_shas: Vec<String> = (0..30)
        .map(|i| format!("{:040x}", i * 12345 + 67890))
        .collect();

    // Simulate epoch changes
    let epochs = vec![
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // Epoch 1
        1, 1, 1, 1, 1, 2, 2, 2, 2, 2, // Epoch transition at commit 15
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // Epoch 2
    ];

    // Detect change points
    let config = ChangePointConfig {
        min_data_points: 10,
        min_magnitude_pct: 10.0,
        confidence_threshold: 0.5,
        penalty: 3.0,
    };

    let raw_change_points = detect_change_points(&measurements, &config);
    println!("Detected raw change points at indices: {:?}", raw_change_points);

    let enriched_change_points =
        enrich_change_points(&raw_change_points, &measurements, &commit_shas, &config);
    println!("\nEnriched change points:");
    for cp in &enriched_change_points {
        println!(
            "  Index {}: {:+.1}% change ({:?}) at commit {}",
            cp.index,
            cp.magnitude_pct,
            cp.direction,
            &cp.commit_sha[..8]
        );
    }

    let epoch_transitions = detect_epoch_transitions(&epochs);
    println!("\nEpoch transitions:");
    for et in &epoch_transitions {
        println!(
            "  Index {}: Epoch {} → {}",
            et.index, et.from_epoch, et.to_epoch
        );
    }

    // Create Plotly visualization
    let mut plot = Plot::new();
    let plot_config = Configuration::default().responsive(true).fill_frame(true);
    plot.set_configuration(plot_config);

    let size = measurements.len();

    // X-axis setup (reversed to show newest on right)
    let commit_indices: Vec<usize> = (0..size).rev().collect();
    let short_hashes: Vec<String> = commit_shas.iter().rev().map(|s| s[..6].to_string()).collect();

    let x_axis = Axis::new()
        .tick_values(commit_indices.iter().map(|&x| x as f64).collect::<Vec<_>>())
        .tick_text(short_hashes)
        .tick_angle(45.0)
        .tick_font(Font::new().family("monospace"))
        .title(Title::from("Commits (oldest → newest)"));

    let y_axis = Axis::new().title(Title::from("Build Time (seconds)"));

    let layout = Layout::new()
        .title(Title::from("Build Time Performance with Change Point Detection"))
        .x_axis(x_axis)
        .y_axis(y_axis)
        .legend(
            Legend::new()
                .group_click(plotly::layout::GroupClick::ToggleItem)
                .orientation(plotly::common::Orientation::Horizontal),
        );

    plot.set_layout(layout);

    // Add main measurement trace
    let x_values: Vec<usize> = (0..size).map(|i| size - i - 1).collect();
    let main_trace = Scatter::new(x_values.clone(), measurements.clone())
        .name("build_time (seconds)")
        .mode(Mode::LinesMarkers)
        .line(Line::new().color("blue").width(2.0));
    plot.add_trace(main_trace);

    // Add epoch boundary traces (hidden by default)
    let y_min = 8.0;
    let y_max = 17.0;

    if !epoch_transitions.is_empty() {
        let mut x_coords: Vec<Option<usize>> = vec![];
        let mut y_coords: Vec<Option<f64>> = vec![];
        let mut hover_texts: Vec<String> = vec![];

        for transition in &epoch_transitions {
            let x_pos = size - transition.index - 1;
            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_min));
            hover_texts.push(format!(
                "Epoch {}→{}",
                transition.from_epoch, transition.to_epoch
            ));

            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_max));
            hover_texts.push(format!(
                "Epoch {}→{}",
                transition.from_epoch, transition.to_epoch
            ));

            x_coords.push(None);
            y_coords.push(None);
            hover_texts.push(String::new());
        }

        let epoch_trace = Scatter::new(x_coords, y_coords)
            .name("build_time (Epochs)")
            .legend_group("build_time_epochs")
            .visible(Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color("gray").dash(DashType::Dash).width(2.0))
            .show_legend(true)
            .hover_text_array(hover_texts);
        plot.add_trace(epoch_trace);
    }

    // Add change point traces (hidden by default)
    // Regressions (red)
    let regressions: Vec<_> = enriched_change_points
        .iter()
        .filter(|cp| cp.magnitude_pct > 0.0)
        .collect();
    if !regressions.is_empty() {
        let mut x_coords: Vec<Option<usize>> = vec![];
        let mut y_coords: Vec<Option<f64>> = vec![];
        let mut hover_texts: Vec<String> = vec![];

        for cp in &regressions {
            let x_pos = size - cp.index - 1;
            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_min));
            hover_texts.push(format!(
                "Regression {:+.1}% at {}",
                cp.magnitude_pct,
                &cp.commit_sha[..8]
            ));

            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_max));
            hover_texts.push(format!(
                "Regression {:+.1}% at {}",
                cp.magnitude_pct,
                &cp.commit_sha[..8]
            ));

            x_coords.push(None);
            y_coords.push(None);
            hover_texts.push(String::new());
        }

        let regression_trace = Scatter::new(x_coords, y_coords)
            .name("build_time (Regressions)")
            .legend_group("build_time_changes")
            .visible(Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color("rgba(220, 53, 69, 0.8)").width(3.0))
            .show_legend(true)
            .hover_text_array(hover_texts);
        plot.add_trace(regression_trace);
    }

    // Improvements (green)
    let improvements: Vec<_> = enriched_change_points
        .iter()
        .filter(|cp| cp.magnitude_pct < 0.0)
        .collect();
    if !improvements.is_empty() {
        let mut x_coords: Vec<Option<usize>> = vec![];
        let mut y_coords: Vec<Option<f64>> = vec![];
        let mut hover_texts: Vec<String> = vec![];

        for cp in &improvements {
            let x_pos = size - cp.index - 1;
            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_min));
            hover_texts.push(format!(
                "Improvement {:+.1}% at {}",
                cp.magnitude_pct,
                &cp.commit_sha[..8]
            ));

            x_coords.push(Some(x_pos));
            y_coords.push(Some(y_max));
            hover_texts.push(format!(
                "Improvement {:+.1}% at {}",
                cp.magnitude_pct,
                &cp.commit_sha[..8]
            ));

            x_coords.push(None);
            y_coords.push(None);
            hover_texts.push(String::new());
        }

        let improvement_trace = Scatter::new(x_coords, y_coords)
            .name("build_time (Improvements)")
            .legend_group("build_time_changes")
            .visible(Visible::LegendOnly)
            .mode(Mode::Lines)
            .line(Line::new().color("rgba(40, 167, 69, 0.8)").width(3.0))
            .show_legend(true)
            .hover_text_array(hover_texts);
        plot.add_trace(improvement_trace);
    }

    // Write HTML report
    let html_content = plot.to_html();
    let output_path = "change_point_demo_report.html";
    let mut file = File::create(output_path).expect("Failed to create output file");
    file.write_all(html_content.as_bytes())
        .expect("Failed to write HTML");

    println!("\n✅ Demo report created: {}", output_path);
    println!("\nHow to use the report:");
    println!("1. Open {} in a web browser", output_path);
    println!("2. The main measurement line (build_time) is visible by default");
    println!("3. Click on 'build_time (Epochs)' in the legend to show epoch boundaries (gray dashed lines)");
    println!("4. Click on 'build_time (Regressions)' in the legend to show regression change points (red lines)");
    println!("5. Click on 'build_time (Improvements)' in the legend to show improvement change points (green lines)");
    println!("6. Hover over the vertical lines to see details about the change");
}
