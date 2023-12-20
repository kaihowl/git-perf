use std::{process, time::Instant};

use anyhow::Result;

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
        let output = process
            .output()
            .expect("Command failed to spawn TODO(kaihowl)");
        output
            .status
            .success()
            .then_some(())
            .ok_or("TODO(kaihowl) running error")
            .expect("TODO(kaihowl)");
        let duration = start.elapsed();
        let duration_usec = duration.as_nanos() as f64;
        measurement_storage::add(measurement, duration_usec, key_values)?;
    }
    Ok(())
}
