use crate::git::git_interop::get_repository_root;
use crate::serialization::deserialize;
use anyhow::{Context, Result};
use git_perf_cli_types::SizeFormat;
use human_repr::HumanCount;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

/// Information about measurement storage size
struct NotesSizeInfo {
    /// Total size in bytes
    total_bytes: u64,
    /// Number of commits with measurements
    note_count: usize,
    /// Optional breakdown by measurement name
    by_measurement: Option<HashMap<String, MeasurementSizeInfo>>,
}

/// Size information for a specific measurement name
struct MeasurementSizeInfo {
    /// Total bytes for this measurement
    total_bytes: u64,
    /// Number of occurrences
    count: usize,
}

/// Git repository statistics from count-objects
struct RepoStats {
    /// Number of loose objects
    loose_objects: u64,
    /// Size of loose objects in bytes
    loose_size: u64,
    /// Number of packed objects
    packed_objects: u64,
    /// Size of pack files in bytes
    pack_size: u64,
}

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

/// Get size information for all measurement notes
fn get_notes_size(detailed: bool, disk_size: bool) -> Result<NotesSizeInfo> {
    let repo_root =
        get_repository_root().map_err(|e| anyhow::anyhow!("Failed to get repo root: {}", e))?;

    let batch_format = if disk_size {
        "%(objectsize:disk)"
    } else {
        "%(objectsize)"
    };

    // Spawn git notes list process
    let mut list_notes = Command::new("git")
        .args(["notes", "--ref", "refs/notes/perf-v3", "list"])
        .current_dir(&repo_root)
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn git notes list")?;

    let notes_out = list_notes
        .stdout
        .take()
        .context("Failed to take stdout from git notes list")?;

    // Spawn git cat-file process
    let mut cat_file = Command::new("git")
        .args(["cat-file", &format!("--batch-check={}", batch_format)])
        .current_dir(&repo_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn git cat-file")?;

    let cat_file_in = cat_file
        .stdin
        .take()
        .context("Failed to take stdin from git cat-file")?;
    let cat_file_out = cat_file
        .stdout
        .take()
        .context("Failed to take stdout from git cat-file")?;

    // Spawn a thread to pipe note OIDs from git notes list to git cat-file
    // Also collect the note OIDs for later use in detailed breakdown
    let note_oids_handle = thread::spawn(move || -> Result<Vec<String>> {
        let reader = BufReader::new(notes_out);
        let mut writer = BufWriter::new(cat_file_in);
        let mut note_oids = Vec::new();

        for line in reader.lines() {
            let line = line.context("Failed to read line from git notes list")?;
            if let Some(note_oid) = line.split_whitespace().next() {
                writeln!(writer, "{}", note_oid).context("Failed to write OID to git cat-file")?;
                note_oids.push(note_oid.to_string());
            }
        }
        // writer is dropped here, closing stdin to cat-file
        Ok(note_oids)
    });

    // Read sizes from git cat-file output
    let reader = BufReader::new(cat_file_out);
    let mut sizes = Vec::new();

    for line in reader.lines() {
        let line = line.context("Failed to read line from git cat-file")?;
        let size = line
            .trim()
            .parse::<u64>()
            .with_context(|| format!("Failed to parse size from: {}", line))?;
        sizes.push(size);
    }

    // Wait for processes to complete
    let note_oids = note_oids_handle
        .join()
        .map_err(|_| anyhow::anyhow!("Thread panicked"))?
        .context("Failed to collect note OIDs")?;

    list_notes
        .wait()
        .context("Failed to wait for git notes list")?;
    let cat_file_status = cat_file.wait().context("Failed to wait for git cat-file")?;

    if !cat_file_status.success() {
        anyhow::bail!("git cat-file process failed");
    }

    let note_count = note_oids.len();
    if note_count == 0 {
        return Ok(NotesSizeInfo {
            total_bytes: 0,
            note_count: 0,
            by_measurement: if detailed { Some(HashMap::new()) } else { None },
        });
    }

    if sizes.len() != note_count {
        anyhow::bail!("Expected {} sizes but got {}", note_count, sizes.len());
    }

    let total_bytes: u64 = sizes.iter().sum();

    let mut by_measurement = if detailed { Some(HashMap::new()) } else { None };

    // If detailed breakdown requested, parse measurement names
    if let Some(ref mut by_name) = by_measurement {
        for (note_oid, &size) in note_oids.iter().zip(sizes.iter()) {
            accumulate_measurement_sizes(Path::new(&repo_root), note_oid, size, by_name)?;
        }
    }

    Ok(NotesSizeInfo {
        total_bytes,
        note_count,
        by_measurement,
    })
}

/// Parse note contents and accumulate sizes by measurement name
fn accumulate_measurement_sizes(
    repo_root: &std::path::Path,
    note_oid: &str,
    note_size: u64,
    by_name: &mut HashMap<String, MeasurementSizeInfo>,
) -> Result<()> {
    // Get note content
    let output = Command::new("git")
        .args(["cat-file", "-p", note_oid])
        .current_dir(repo_root)
        .output()
        .context("Failed to execute git cat-file -p")?;

    if !output.status.success() {
        anyhow::bail!("git cat-file -p failed for {}", note_oid);
    }

    let content = String::from_utf8_lossy(&output.stdout);

    // Parse measurements from note
    let measurements = deserialize(&content);

    if measurements.is_empty() {
        return Ok(());
    }

    // Distribute note size evenly among measurements in this note
    // (Each measurement contributes roughly equally to the note size)
    let size_per_measurement = note_size / measurements.len() as u64;

    for measurement in measurements {
        let entry = by_name
            .entry(measurement.name.clone())
            .or_insert(MeasurementSizeInfo {
                total_bytes: 0,
                count: 0,
            });

        entry.total_bytes += size_per_measurement;
        entry.count += 1;
    }

    Ok(())
}

/// Get git repository statistics
fn get_repo_stats() -> Result<RepoStats> {
    let repo_root =
        get_repository_root().map_err(|e| anyhow::anyhow!("Failed to get repo root: {}", e))?;

    let output = Command::new("git")
        .args(["count-objects", "-v"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git count-objects")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git count-objects failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut loose_objects = 0;
    let mut loose_size = 0; // in KiB from git
    let mut packed_objects = 0;
    let mut pack_size = 0; // in KiB from git

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0].trim();
        let value = parts[1].trim().parse::<u64>().unwrap_or(0);

        match key {
            "count" => loose_objects = value,
            "size" => loose_size = value,
            "in-pack" => packed_objects = value,
            "size-pack" => pack_size = value,
            _ => {}
        }
    }

    Ok(RepoStats {
        loose_objects,
        loose_size: loose_size * 1024, // Convert KiB to bytes
        packed_objects,
        pack_size: pack_size * 1024, // Convert KiB to bytes
    })
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
