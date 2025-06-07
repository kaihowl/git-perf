use std::{process, time::Instant};

use anyhow::{bail, Context, Result};

use crate::measurement_storage::{self};

pub fn measure(
    measurement: &str,
    repetitions: u16,
    command: &[String],
    key_values: &[(String, String)],
) -> Result<()> {
    let exe = command.first().unwrap();
    let args = &command[1..];
    for _ in 0..repetitions {
        let mut process = process::Command::new(exe);
        process.args(args);
        let start = Instant::now();
        let output = process.output().context("Command failed to spawn")?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Command '{}' failed to run:\nstdout:\n{}\nstderr:\n{}",
                exe,
                stdout,
                stderr,
            );
        }
        let duration = start.elapsed();
        let duration_usec = duration.as_nanos() as f64;
        measurement_storage::add(measurement, duration_usec, key_values)?;
    }
    Ok(())
}
