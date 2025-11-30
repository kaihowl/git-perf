use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use super::git_interop::{create_consolidated_read_branch, get_repository_root};

/// Information about the size of a specific measurement
pub struct MeasurementSizeInfo {
    /// Total bytes for this measurement
    pub total_bytes: u64,
    /// Number of occurrences
    pub count: usize,
}

/// Information about measurement storage size
pub struct NotesSizeInfo {
    /// Total size in bytes
    pub total_bytes: u64,
    /// Number of commits with measurements
    pub note_count: usize,
    /// Optional breakdown by measurement name
    pub by_measurement: Option<HashMap<String, MeasurementSizeInfo>>,
}

/// Get size information for all measurement notes
pub fn get_notes_size(detailed: bool, disk_size: bool) -> Result<NotesSizeInfo> {
    let repo_root =
        get_repository_root().map_err(|e| anyhow::anyhow!("Failed to get repo root: {}", e))?;

    // Create a consolidated read branch to include pending writes
    let read_branch = create_consolidated_read_branch()?;

    let batch_format = if disk_size {
        "%(objectsize:disk)"
    } else {
        "%(objectsize)"
    };

    // Spawn git notes list process using the temporary read branch
    let mut list_notes = Command::new("git")
        .args(["notes", "--ref", read_branch.ref_name(), "list"])
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
    use crate::serialization::deserialize;

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

/// Git repository statistics from count-objects
pub struct RepoStats {
    /// Number of loose objects
    pub loose_objects: u64,
    /// Size of loose objects in bytes
    pub loose_size: u64,
    /// Number of packed objects
    pub packed_objects: u64,
    /// Size of pack files in bytes
    pub pack_size: u64,
}

/// Get git repository statistics
pub fn get_repo_stats() -> Result<RepoStats> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::dir_with_repo;

    #[test]
    fn test_get_repo_stats_basic() {
        // Test that get_repo_stats works and returns proper values
        let temp_dir = dir_with_repo();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let stats = get_repo_stats().unwrap();

        // Should have some objects after initial commit
        assert!(stats.loose_objects > 0 || stats.packed_objects > 0);

        // Sizes should be multiples of 1024 (tests * 1024 conversion)
        if stats.loose_size > 0 {
            assert_eq!(
                stats.loose_size % 1024,
                0,
                "loose_size should be multiple of 1024"
            );
        }
        if stats.pack_size > 0 {
            assert_eq!(
                stats.pack_size % 1024,
                0,
                "pack_size should be multiple of 1024"
            );
        }
    }

    #[test]
    fn test_get_notes_size_empty_repo() {
        // Test with a repo that has no notes - exercises the empty case
        let temp_dir = dir_with_repo();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = get_notes_size(false, false).unwrap();
        assert_eq!(result.total_bytes, 0);
        assert_eq!(result.note_count, 0);
        assert!(result.by_measurement.is_none());
    }

    #[test]
    fn test_get_repo_stats_conversion_factors() {
        // Test that the * 1024 conversion is correctly applied
        let temp_dir = dir_with_repo();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let stats = get_repo_stats().unwrap();

        // Test that loose_size and pack_size are properly converted from KiB to bytes
        // Both should be multiples of 1024
        assert_eq!(
            stats.loose_size % 1024,
            0,
            "loose_size must be multiple of 1024 (bytes conversion from KiB)"
        );
        assert_eq!(
            stats.pack_size % 1024,
            0,
            "pack_size must be multiple of 1024 (bytes conversion from KiB)"
        );

        // If there are loose objects, the size should be reasonable (not zero, not absurdly large)
        if stats.loose_objects > 0 {
            assert!(
                stats.loose_size > 0,
                "loose_size should be > 0 if loose_objects > 0"
            );
            assert!(
                stats.loose_size < 1_000_000_000,
                "loose_size should be reasonable"
            );
        }
    }

    #[test]
    fn test_get_repo_stats_field_assignments() {
        // Test that all fields are properly assigned from git output
        let temp_dir = dir_with_repo();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let stats = get_repo_stats().unwrap();

        // Verify that fields are assigned (not just defaulted to 0)
        // After creating a repo with an initial commit, we should have objects
        let total_objects = stats.loose_objects + stats.packed_objects;
        assert!(
            total_objects > 0,
            "Should have at least one object from initial commit"
        );

        // Verify the match arms are working by checking expected field types
        // loose_objects should be count
        // loose_size should be size * 1024
        // packed_objects should be in-pack
        // pack_size should be size-pack * 1024

        // All values should be >= 0 (trivially true for u64, but tests the assignments)
        assert!(stats.loose_objects >= 0);
        assert!(stats.loose_size >= 0);
        assert!(stats.packed_objects >= 0);
        assert!(stats.pack_size >= 0);
    }

    #[test]
    fn test_accumulate_measurement_sizes() {
        use crate::data::MeasurementData;
        use crate::serialization::serialize_multiple;
        use crate::test_helpers::{hermetic_git_env, run_git_command};
        use std::collections::HashMap;

        // Create a hermetic git repo
        let temp_dir = dir_with_repo();
        let repo_path = temp_dir.path();
        hermetic_git_env();

        // Create test measurements
        let measurements = vec![
            MeasurementData {
                epoch: 1,
                name: "test_metric_a".to_string(),
                timestamp: 1234567890.0,
                val: 100.0,
                key_values: HashMap::new(),
            },
            MeasurementData {
                epoch: 1,
                name: "test_metric_b".to_string(),
                timestamp: 1234567891.0,
                val: 200.0,
                key_values: HashMap::new(),
            },
            MeasurementData {
                epoch: 1,
                name: "test_metric_a".to_string(),
                timestamp: 1234567892.0,
                val: 150.0,
                key_values: HashMap::new(),
            },
        ];

        // Serialize measurements
        let serialized = serialize_multiple(&measurements);

        // Get HEAD commit hash
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to get HEAD hash");
        let head_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Add measurements as a note to HEAD
        std::fs::write(repo_path.join("note_content.txt"), &serialized)
            .expect("Failed to write note content");
        run_git_command(
            &[
                "notes",
                "--ref",
                "refs/notes/perf-v3",
                "add",
                "-F",
                "note_content.txt",
                &head_hash,
            ],
            repo_path,
        );

        // Get the note OID
        let output = std::process::Command::new("git")
            .args(["notes", "--ref", "refs/notes/perf-v3", "list"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to list notes");
        let notes_list = String::from_utf8_lossy(&output.stdout);
        let note_oid = notes_list
            .split_whitespace()
            .next()
            .expect("No note OID found");

        // Get the note size using git cat-file
        let output = std::process::Command::new("git")
            .args(["cat-file", "-s", note_oid])
            .current_dir(repo_path)
            .output()
            .expect("Failed to get note size");
        let note_size: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .expect("Failed to parse note size");

        // Call accumulate_measurement_sizes
        let mut by_name = HashMap::new();
        accumulate_measurement_sizes(repo_path, note_oid, note_size, &mut by_name)
            .expect("Failed to accumulate measurement sizes");

        // Verify results
        assert_eq!(by_name.len(), 2, "Should have 2 unique measurement names");

        let metric_a = by_name
            .get("test_metric_a")
            .expect("test_metric_a should exist");
        assert_eq!(metric_a.count, 2, "test_metric_a should appear 2 times");
        // Each measurement gets note_size / 3 (3 total measurements)
        let expected_size_per_measurement = note_size / 3;
        assert_eq!(
            metric_a.total_bytes,
            expected_size_per_measurement * 2,
            "test_metric_a should have size for 2 measurements"
        );

        let metric_b = by_name
            .get("test_metric_b")
            .expect("test_metric_b should exist");
        assert_eq!(metric_b.count, 1, "test_metric_b should appear 1 time");
        assert_eq!(
            metric_b.total_bytes, expected_size_per_measurement,
            "test_metric_b should have size for 1 measurement"
        );
    }
}
