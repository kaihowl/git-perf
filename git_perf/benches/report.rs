use std::env::set_current_dir;
use std::process::Command;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use git_perf::test_helpers::{empty_commit, hermetic_git_env};
use tempfile::tempdir;

fn prep_repo(num_commits: usize, num_measurements: usize) -> tempfile::TempDir {
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

    for _ in 0..num_commits {
        empty_commit();
        let measurements = vec![100.0; num_measurements];
        git_perf::measurement_storage::add_multiple("benchmark-a", &measurements, &[])
            .expect("Could not add measurements");
    }

    temp_dir
}

fn report_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("report");
    group.sample_size(10);

    let num_commits = 10;
    let num_measurements = 10;

    let _temp_dir = prep_repo(num_commits, num_measurements);

    group.throughput(Throughput::Elements(num_commits as u64));
    group.bench_function(BenchmarkId::new("report_generation", num_commits), |b| {
        b.iter(|| {
            git_perf::reporting::report(
                "HEAD",
                std::path::PathBuf::from("report.html"),
                vec![],
                num_commits,
                None,
                None,
                &[],
                None,
                &[],
                git_perf::reporting::ReportTemplateConfig {
                    template_path: None,
                    custom_css_path: None,
                    title: None,
                },
                false,
                false,
            )
            .expect("Report generation failed");
        });
    });

    group.finish();
}

criterion_group!(benches, report_generation);
criterion_main!(benches);
