use std::env::set_current_dir;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use git2::{Commit, Signature};
use git_perf::{
    data::ReductionFunc,
    measurement_retrieval::{self, summarize_measurements},
};
use itertools::{assert_equal, Itertools};
use tempfile::tempdir;

fn prep_repo(
    number_commits: usize,
    number_measurements: usize,
) -> (tempfile::TempDir, git2::Repository) {
    let temp_dir = tempdir().unwrap();
    set_current_dir(temp_dir.path()).expect("Failed to change current path");

    let repo = git2::Repository::init(temp_dir.path()).expect("Failed to create repo");
    {
        let mut parents = vec![];
        let signature = Signature::now("fake", "fake@example.com").expect("No signature");
        for i in 1..number_commits {
            let tree_oid = repo
                .treebuilder(None)
                .expect("Failed to create tree")
                .write()
                .expect("Failed to write tree");
            let tree = &repo
                .find_tree(tree_oid)
                .expect("Could not find written tree");
            let oid = repo
                .commit(
                    Some("refs/heads/master"),
                    &signature,
                    &signature,
                    format!("Commit {i}").as_str(),
                    tree,
                    &parents.iter().collect_vec(),
                )
                .expect("Failed to create first commit");

            for _ in 1..number_measurements {
                git_perf::measurement_storage::add("test_measurement", 10.0, &[])
                    .expect("Failed to create measurement");
            }

            let commit = repo.find_commit(oid).expect("Could not find new commit");
            parents = vec![commit];
        }
    }
    (temp_dir, repo)
}

fn criterion_benchmark(c: &mut Criterion) {
    return;
    let mut first = 0;
    let mut group = c.benchmark_group("walk");
    for num_measurements in [10, 100, 500].iter() {
        let (_temp_dir, repo) = prep_repo(40, *num_measurements);

        group.throughput(Throughput::Elements(*num_measurements as u64));
        group.bench_function(BenchmarkId::new("walk1", num_measurements), |b| {
            b.iter(|| {
                let measurements = measurement_retrieval::walk_commits(&repo, 10)
                    .expect("Could not get measurements");
                let summaries =
                    summarize_measurements(measurements, &ReductionFunc::Min, &|_| true);
                first = git_perf::stats::aggregate_measurements(
                    summaries.map(|x| x.unwrap().measurement.unwrap().val),
                )
                .len;
            })
        });
        let mut second = 0;
        group.bench_function(BenchmarkId::new("walk2", num_measurements), |b| {
            b.iter(|| {
                let measurements = measurement_retrieval::walk_commits2(&repo, 10)
                    .expect("Could not get measurements");
                let summaries =
                    summarize_measurements(measurements, &ReductionFunc::Min, &|_| true);
                second = git_perf::stats::aggregate_measurements(
                    summaries.map(|x| x.unwrap().measurement.unwrap().val),
                )
                .len;
            })
        });
        assert_eq!(first, second);
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
