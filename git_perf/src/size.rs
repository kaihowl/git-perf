use crate::git::size_ops::{get_notes_size, get_repo_stats, NotesSizeInfo, RepoStats};
use anyhow::Result;
use git_perf_cli_types::SizeFormat;
use human_repr::HumanCount;

/// Calculate and display measurement storage size
pub fn calculate_measurement_size(
    detailed: bool,
    format: SizeFormat,
    disk_size: bool,
    include_objects: bool,
) -> Result<()> {
    // 1. Get notes size information
    let notes_info = get_notes_size(detailed, disk_size)?;

    // 2. Optionally get repository statistics
    let repo_stats = if include_objects {
        Some(get_repo_stats()?)
    } else {
        None
    };

    // 3. Display results
    display_size_report(&notes_info, repo_stats.as_ref(), format, disk_size)?;

    Ok(())
}

/// Display size report to stdout
fn display_size_report(
    info: &NotesSizeInfo,
    repo_stats: Option<&RepoStats>,
    format: SizeFormat,
    disk_size: bool,
) -> Result<()> {
    let size_type = if disk_size {
        "on-disk (compressed)"
    } else {
        "logical (uncompressed)"
    };

    println!("Live Measurement Size Report");
    println!("============================");
    println!();

    println!("Number of commits with measurements: {}", info.note_count);
    println!(
        "Total measurement data size ({}): {}",
        size_type,
        format_size(info.total_bytes, format)
    );

    // Show repository context if requested
    if let Some(stats) = repo_stats {
        println!();
        println!("Repository Statistics (for context):");
        println!("-------------------------------------");
        println!(
            "  Loose objects: {} ({})",
            stats.loose_objects,
            format_size(stats.loose_size, format)
        );
        println!(
            "  Packed objects: {} ({})",
            stats.packed_objects,
            format_size(stats.pack_size, format)
        );
        println!(
            "  Total repository size: {}",
            format_size(stats.loose_size + stats.pack_size, format)
        );
    }

    // Show detailed breakdown if requested
    if let Some(by_name) = &info.by_measurement {
        println!();
        println!("Breakdown by Measurement Name ({}):", size_type);
        println!("------------------------------");

        // Sort by size descending
        let mut sorted: Vec<_> = by_name.iter().collect();
        sorted.sort_by(|a, b| b.1.total_bytes.cmp(&a.1.total_bytes));

        for (name, size_info) in sorted {
            println!(
                "  {} ({} occurrences): {}",
                name,
                size_info.count,
                format_size(size_info.total_bytes, format)
            );
        }
    }

    Ok(())
}

/// Format size according to requested format
fn format_size(bytes: u64, format: SizeFormat) -> String {
    match format {
        SizeFormat::Bytes => bytes.to_string(),
        SizeFormat::Human => bytes.human_count_bytes().to_string(),
    }
}
