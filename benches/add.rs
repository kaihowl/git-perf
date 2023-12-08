use std::env::set_current_dir;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use git2::Signature;
use git_perf::git_interop::{add_note_line_to_head, add_note_line_to_head2};
use tempfile::tempdir;

fn prep_repo() -> (tempfile::TempDir, git2::Repository) {
    let temp_dir = tempdir().unwrap();
    set_current_dir(temp_dir.path()).expect("Failed to change current path");

    let repo = git2::Repository::init(temp_dir.path()).expect("Failed to create repo");
    {
        let signature = Signature::now("fake", "fake@example.com").expect("No signature");
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
                "my commit",
                tree,
                &[],
            )
            .expect("Failed to create first commit");
    }

    (temp_dir, repo)
}

fn add_measurements(c: &mut Criterion) {
    let (_temp_dir, _repo) = prep_repo();

    let mut group = c.benchmark_group("add_measurements");
    for num_measurements in [1, 50, 100].into_iter() {
        group.bench_with_input(
            BenchmarkId::new("non-append", num_measurements),
            &num_measurements,
            |b, i| {
                b.iter(|| {
                    for _ in 0..*i {
                        add_note_line_to_head("some line measurement test").expect("Oh no");
                    }
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("git-append", num_measurements),
            &num_measurements,
            |b, i| {
                b.iter(|| {
                    for _ in 0..*i {
                        add_note_line_to_head2("some line measurement test").expect("Oh no 2!");
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, add_measurements);
criterion_main!(benches);
