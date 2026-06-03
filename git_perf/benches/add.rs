use std::env::set_current_dir;
use std::process::Command;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use git_perf::git::git_interop::add_note_line_to_head;
use git_perf::test_helpers::{empty_commit, hermetic_git_env};
use tempfile::tempdir;

fn prep_repo() -> tempfile::TempDir {
    let temp_dir = tempdir().unwrap();

    set_current_dir(temp_dir.path()).expect("Failed to change current path");
    hermetic_git_env();

    // Explicit --initial-branch for consistent behaviour across git versions.
    assert!(Command::new("git")
        .args(["init", "--initial-branch", "master"])
        .output()
        .expect("Failed to init git repo")
        .status
        .success());

    empty_commit();

    temp_dir
}

fn add_measurements(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_measurements");
    for num_measurements in [1, 50, 100].into_iter() {
        group.throughput(Throughput::Elements(num_measurements as u64));
        // The single-measurement case has the highest relative variance (process-spawn
        // jitter dominates) so it needs more samples for a stable median estimate.
        if num_measurements == 1 {
            group.sample_size(50);
        } else {
            group.sample_size(10);
        }
        group.bench_with_input(
            BenchmarkId::new("add_measurement", num_measurements),
            &num_measurements,
            |b, &i| {
                b.iter_batched(
                    prep_repo,
                    |_temp_dir| {
                        for _ in 0..i {
                            add_note_line_to_head("some line measurement test").expect("Oh no");
                        }
                    },
                    BatchSize::PerIteration,
                );
            },
        );
    }

    group.finish();
}

fn add_multiple_measurements(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_multiple_measurements");
    group.sample_size(10);
    for num_measurements in [1, 50, 100].into_iter() {
        let lines = ["some line measurement test"]
            .repeat(num_measurements)
            .join("\n");
        group.throughput(Throughput::Elements(num_measurements as u64));
        group.bench_with_input(
            BenchmarkId::new("add_multiple_measurements", num_measurements),
            &num_measurements,
            |b, _i| {
                b.iter_batched(
                    prep_repo,
                    |_temp_dir| {
                        add_note_line_to_head(&lines).expect("failed to add lines");
                    },
                    BatchSize::PerIteration,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, add_measurements, add_multiple_measurements);
criterion_main!(benches);
