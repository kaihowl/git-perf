# Performance Measurements in Git

Test tracking performance measurements in git-notes.

Example report for [master](https://kaihowl.github.io/git-perf/master.html).

## Warning
Experimental only.
Repeated additions of measurements (instead of bulk additions) will be costly:
Each time the entire previous measurements are copied and a single line is appended.

# Development

## Rust tests
```
cargo test
```

## Integration / bash tests
```
cargo build && PATH=$(pwd)/target/debug:$PATH test/run_tests.sh
```
