use std::env::set_current_dir;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use git_perf::git::git_interop::add_note_line_to_head;
use git_perf::test_helpers::{empty_commit, hermetic_git_env, init_repo_simple};
use tempfile::tempdir;

fn prep_repo() -> tempfile::TempDir {
    let temp_dir = tempdir().unwrap();

    set_current_dir(temp_dir.path()).expect("Failed to change current path");
    hermetic_git_env();

    init_repo_simple();

    empty_commit();

    temp_dir
}

fn add_measurements(c: &mut Criterion) {
    let _temp_dir = prep_repo();

    let mut group = c.benchmark_group("add_measurements");
    for num_measurements in [1, 50, 100].into_iter() {
        group.bench_with_input(
            BenchmarkId::new("add_measurement", num_measurements),
            &num_measurements,
            |b, i| {
                b.iter(|| {
                    for _ in 0..*i {
                        add_note_line_to_head("some line measurement test").expect("Oh no");
                    }
                });
            },
        );
    }

    group.finish();
}

fn add_multiple_measurements(c: &mut Criterion) {
    let _temp_dir = prep_repo();

    let mut group = c.benchmark_group("add_multiple_measurements");
    for num_measurements in [1, 50, 100].into_iter() {
        let lines = ["some line measurement test"]
            .repeat(num_measurements)
            .join("\n");
        group.bench_with_input(
            BenchmarkId::new("add_multiple_measurements", num_measurements),
            &num_measurements,
            |b, _i| {
                b.iter(|| {
                    add_note_line_to_head(&lines).expect("failed to add lines");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, add_measurements, add_multiple_measurements);
criterion_main!(benches);
