mod utils;

use std::env::set_current_dir;

use utils::{empty_commit, hermetic_git_env, init_repo};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use git_perf::{
    data::ReductionFunc,
    measurement_retrieval::{self, summarize_measurements},
};
use tempfile::tempdir;

fn prep_repo(number_commits: usize, number_measurements: usize) -> tempfile::TempDir {
    let temp_dir = tempdir().unwrap();

    set_current_dir(temp_dir.path()).expect("Failed to change current path");
    hermetic_git_env();

    init_repo();

    for _ in 1..number_commits {
        empty_commit();

        let measurements = [10.0].repeat(number_measurements);
        git_perf::measurement_storage::add_multiple("test_measurement", &measurements, &[])
            .expect("Could not add measurements");
    }

    temp_dir
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("read");
    let num_commits = 40;

    for num_measurements in [10, 100, 500].iter() {
        let _temp_dir = prep_repo(num_commits, *num_measurements);

        group.throughput(Throughput::Elements(*num_measurements as u64));
        group.bench_function(BenchmarkId::new("read", num_measurements), |b| {
            b.iter(|| {
                let measurements = measurement_retrieval::walk_commits(num_commits)
                    .expect("Could not get measurements");
                let summaries =
                    summarize_measurements(measurements, &ReductionFunc::Min, &|_| true);
                git_perf::stats::aggregate_measurements(
                    summaries.map(|x| x.unwrap().measurement.unwrap().val),
                );
            })
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
