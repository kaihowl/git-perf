# Performance Measurements in Git

Test tracking performance measurements in git-notes.

Example report for [master](https://kaihowl.github.io/git-perf/master.html).

## Warning
Experimental only.
Repeated additions of measurements (instead of bulk additions) will be costly:
Each time the entire previous measurements are copied and a single line is
appended.

# Limitations

Contrary to git itself, git-perf does not support decentralized collection of
performance measurements. Instead, git-perf assumes that there is a single,
central place for the collection of metrics. This should usually be your source
foundry, e.g., GitHub. As performance measurements become less relevant over
time, we allow metrics to be purged. As a delete in git still preserves the
history before that deletion event, we have to rewrite history. To make
rewriting of shared history safe, git-perf deliberately dropped some basic
ideas of decentralized version control and instead focuses on the collection of
metrics in a single central location.

## Migrate measurements
TODO document this

# Setup Different Remote
TODO document this

# Docs

See [manpages](./docs/manpage.md).

## Manpage Generation

The manpages are automatically generated during the build process using `clap_mangen`. To regenerate the documentation:

```bash
# Build the project to generate manpages
cargo build

# Convert main manpage to markdown
pandoc -f man -t gfm target/man/man1/git-perf.1 > docs/manpage.md

# Or convert the main and all subcommand manpages to markdown
for file in target/man/man1/git-perf.1 target/man/man1/git-perf-add.1 target/man/man1/git-perf-audit.1 target/man/man1/git-perf-bump-epoch.1 target/man/man1/git-perf-measure.1 target/man/man1/git-perf-prune.1 target/man/man1/git-perf-pull.1 target/man/man1/git-perf-push.1 target/man/man1/git-perf-remove.1 target/man/man1/git-perf-report.1; do
    echo "$(basename "$file" .1)";
    echo "================";
    pandoc -f man -t gfm "$file";
    echo -e "\n\n";
done > docs/manpage.md
```

# Development

## Development dependencies

- libfaketime

Install with 
```
if [[ $(uname -s) = Darwin ]]; then
    brew install libfaketime
else # ubuntu
    sudo apt-get install libfaketime
fi
```

## Rust tests
```
cargo test
```

Exclude slow integration tests with:
```
cargo test -- --skip slow
```
