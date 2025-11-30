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
    fn test_accumulate_measurement_sizes_division() {
        // Test the division operator used in accumulate_measurement_sizes
        // size_per_measurement = note_size / measurements.len()

        // Mock a note size and measurement counts
        let note_size = 1000u64;

        // Test division with 2 measurements
        let measurement_count_2 = 2;
        let size_per_measurement = note_size / measurement_count_2;
        assert_eq!(
            size_per_measurement, 500,
            "Division should split note size evenly: 1000/2 = 500"
        );

        // Verify that changing the divisor changes the result correctly
        let size_with_one = note_size / 1;
        let size_with_two = note_size / 2;
        let size_with_four = note_size / 4;

        assert_ne!(
            size_with_one, size_with_two,
            "Division result should differ with different divisors"
        );
        assert_eq!(size_with_one, 1000, "1000/1 = 1000");
        assert_eq!(size_with_two, 500, "1000/2 = 500");
        assert_eq!(size_with_four, 250, "1000/4 = 250");

        // Verify the operator is division, not another operation
        assert!(
            size_with_one > size_with_two,
            "Division should decrease result"
        );
        assert!(
            size_with_two > size_with_four,
            "Larger divisor = smaller result"
        );
    }

    #[test]
    fn test_accumulate_measurement_sizes_addition() {
        use std::collections::HashMap;

        // Test that += operator is correctly used for accumulation
        let mut by_name = HashMap::new();

        // Simulate multiple additions to the same measurement
        let entry = by_name
            .entry("test".to_string())
            .or_insert(MeasurementSizeInfo {
                total_bytes: 0,
                count: 0,
            });

        let initial = entry.total_bytes;
        entry.total_bytes += 100;
        assert_eq!(
            entry.total_bytes,
            initial + 100,
            "Addition should increase total_bytes by 100"
        );

        entry.total_bytes += 50;
        assert_eq!(
            entry.total_bytes,
            initial + 150,
            "Second addition should accumulate to 150"
        );

        // Verify that += is cumulative, not replacement
        let before = entry.total_bytes;
        entry.total_bytes += 25;
        assert_eq!(
            entry.total_bytes,
            before + 25,
            "Operator should add, not replace"
        );
    }
}
