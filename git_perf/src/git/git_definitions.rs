/// Min supported git version
/// Must be version 2.46.0 at least to support symref-update commands
/// This version introduced the symref-update instruction for atomic symref operations
pub const EXPECTED_VERSION: (i32, i32, i32) = (2, 46, 0);

/// The main branch where performance measurements are stored as git notes
pub const REFS_NOTES_BRANCH: &str = "refs/notes/perf-v3";

/// Symbolic reference that points to the current write target for performance measurements
pub const REFS_NOTES_WRITE_SYMBOLIC_REF: &str = "refs/notes/perf-v3-write";

/// Prefix for temporary write target references used during concurrent operations
pub const REFS_NOTES_WRITE_TARGET_PREFIX: &str = "refs/notes/perf-v3-write-";

/// Prefix for temporary references used when adding new measurements
pub const REFS_NOTES_ADD_TARGET_PREFIX: &str = "refs/notes/perf-v3-add-";

/// Prefix for temporary references used when rewriting existing measurements
pub const REFS_NOTES_REWRITE_TARGET_PREFIX: &str = "refs/notes/perf-v3-rewrite-";

/// Prefix for temporary references used during merge operations
pub const REFS_NOTES_MERGE_BRANCH_PREFIX: &str = "refs/notes/perf-v3-merge-";

/// Branch used for reconciling and then reading performance measurements
pub const REFS_NOTES_READ_PREFIX: &str = "refs/notes/perf-v3-read-";

/// The default remote name used for git-perf operations
pub const GIT_PERF_REMOTE: &str = "git-perf-origin";

/// The standard git remote name
pub const GIT_ORIGIN: &str = "origin";
