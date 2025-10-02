# Command-Line Help for `git-perf`

This document contains the help content for the `git-perf` command-line program.

**Command Overview:**

* [`git-perf`↴](#git-perf)
* [`git-perf measure`↴](#git-perf-measure)
* [`git-perf add`↴](#git-perf-add)
* [`git-perf push`↴](#git-perf-push)
* [`git-perf pull`↴](#git-perf-pull)
* [`git-perf report`↴](#git-perf-report)
* [`git-perf audit`↴](#git-perf-audit)
* [`git-perf bump-epoch`↴](#git-perf-bump-epoch)
* [`git-perf remove`↴](#git-perf-remove)
* [`git-perf prune`↴](#git-perf-prune)
* [`git-perf list-commits`↴](#git-perf-list-commits)

## `git-perf`

**Usage:** `git-perf [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `measure` — Measure the runtime of the supplied command (in nanoseconds)
* `add` — Add single measurement
* `push` — Publish performance results to remote
* `pull` — Pull performance results from remote
* `report` — Create an HTML performance report
* `audit` — For given measurements, check perfomance deviations of the HEAD commit against `<n>` previous commits. Group previous results and aggregate their results before comparison
* `bump-epoch` — Accept HEAD commit's measurement for audit, even if outside of range. This is allows to accept expected performance changes. This is accomplished by starting a new epoch for the given measurement. The epoch is configured in the git perf config file. A change to the epoch therefore has to be committed and will result in a new HEAD for which new measurements have to be taken
* `remove` — Remove all performance measurements for commits that have been committed at or before the specified time (inclusive boundary, uses <=)
* `prune` — Remove all performance measurements for non-existent/unreachable objects. Will refuse to work if run on a shallow clone
* `list-commits` — List all commits that have performance measurements

###### **Options:**

* `-v`, `--verbose` — Increase verbosity level (can be specified multiple times.) The first level sets level "info", second sets level "debug", and third sets level "trace" for the logger



## `git-perf measure`

Measure the runtime of the supplied command (in nanoseconds)

**Usage:** `git-perf measure [OPTIONS] --measurement <NAME> -- <COMMAND>...`

###### **Arguments:**

* `<COMMAND>` — Command to measure

###### **Options:**

* `-n`, `--repetitions <REPETITIONS>` — Repetitions

  Default value: `1`
* `-m`, `--measurement <NAME>` — Name of the measurement
* `-k`, `--key-value <KEY_VALUE>` — Key-value pairs separated by '='



## `git-perf add`

Add single measurement

**Usage:** `git-perf add [OPTIONS] --measurement <NAME> <VALUE>`

###### **Arguments:**

* `<VALUE>` — Measured value to be added

###### **Options:**

* `-m`, `--measurement <NAME>` — Name of the measurement
* `-k`, `--key-value <KEY_VALUE>` — Key-value pairs separated by '='



## `git-perf push`

Publish performance results to remote

**Usage:** `git-perf push`



## `git-perf pull`

Pull performance results from remote

**Usage:** `git-perf pull`



## `git-perf report`

Create an HTML performance report

**Usage:** `git-perf report [OPTIONS]`

###### **Options:**

* `-o`, `--output <OUTPUT>` — HTML output file

  Default value: `output.html`
* `-n`, `--max-count <MAX_COUNT>` — Limit the number of previous commits considered. HEAD is included in this count

  Default value: `40`
* `-m`, `--measurement <MEASUREMENT>` — Select an individual measurements instead of all
* `-k`, `--key-value <KEY_VALUE>` — Key-value pairs separated by '=', select only matching measurements
* `-s`, `--separate-by <SEPARATE_BY>` — Create individual traces in the graph by grouping with the value of this selector
* `-a`, `--aggregate-by <AGGREGATE_BY>` — What to aggregate the measurements in each group with

  Possible values: `min`, `max`, `median`, `mean`




## `git-perf audit`

For given measurements, check perfomance deviations of the HEAD commit against `<n>` previous commits. Group previous results and aggregate their results before comparison.

The audit can be configured to ignore statistically significant deviations if they are below a minimum relative deviation threshold. This helps filter out noise while still catching meaningful performance changes.

## Statistical Dispersion Methods

The audit supports two methods for calculating statistical dispersion:

**Standard Deviation (stddev)**: Traditional method that is sensitive to outliers. Use when your performance data is normally distributed and you want to detect all performance changes, including those caused by outliers.

**Median Absolute Deviation (MAD)**: Robust method that is less sensitive to outliers. Use when your performance data has occasional outliers or spikes, or when you want to focus on typical performance changes rather than extreme values.

## Configuration

Configuration is done via the `.gitperfconfig` file:

**Default settings:** - `[measurement].min_relative_deviation = 5.0` - `[measurement].dispersion_method = "mad"`

**Measurement-specific settings (override defaults):** - `[measurement."name"].min_relative_deviation = 10.0` - `[measurement."name"].dispersion_method = "stddev"`

## Precedence

The dispersion method is determined in this order: 1. CLI option (`--dispersion-method` or `-D`) - highest priority 2. Measurement-specific config - overrides default 3. Default config - overrides built-in default 4. Built-in default (stddev) - lowest priority

When the relative deviation is below the threshold, the audit passes even if the z-score exceeds the sigma threshold. The relative deviation is calculated as: `|(head_value / tail_median - 1.0) * 100%|` where tail_median is the median of historical measurements (excluding HEAD).

The sparkline visualization shows the range of measurements relative to the tail median (historical measurements only).

**Usage:** `git-perf audit [OPTIONS] --measurement <MEASUREMENT>`

###### **Options:**

* `-m`, `--measurement <MEASUREMENT>`
* `-n`, `--max-count <MAX_COUNT>` — Limit the number of previous commits considered. HEAD is included in this count

  Default value: `40`
* `-s`, `--selectors <SELECTORS>` — Key-value pair separated by "=" with no whitespaces to subselect measurements
* `--min-measurements <MIN_MEASUREMENTS>` — Minimum number of measurements needed. If less, pass test and assume more measurements are needed. A minimum of two historic measurements are needed for proper evaluation of standard deviation

  Default value: `2`
* `-a`, `--aggregate-by <AGGREGATE_BY>` — What to aggregate the measurements in each group with

  Default value: `min`

  Possible values: `min`, `max`, `median`, `mean`

* `-d`, `--sigma <SIGMA>` — Multiple of the stddev after which a outlier is detected. If the HEAD measurement is within `[mean-<d>*sigma; mean+<d>*sigma]`, it is considered acceptable

  Default value: `4.0`
* `-D`, `--dispersion-method <DISPERSION_METHOD>` — Method for calculating statistical dispersion. Choose between:

   **stddev**: Standard deviation - sensitive to outliers, use for normally distributed data where you want to detect all changes.

   **mad**: Median Absolute Deviation - robust to outliers, use when data has occasional spikes or you want to focus on typical changes.

   If not specified, uses the value from .gitperfconfig file, or defaults to stddev.

  Possible values: `stddev`, `mad`




## `git-perf bump-epoch`

Accept HEAD commit's measurement for audit, even if outside of range. This is allows to accept expected performance changes. This is accomplished by starting a new epoch for the given measurement. The epoch is configured in the git perf config file. A change to the epoch therefore has to be committed and will result in a new HEAD for which new measurements have to be taken

**Usage:** `git-perf bump-epoch --measurement <MEASUREMENT>`

###### **Options:**

* `-m`, `--measurement <MEASUREMENT>`



## `git-perf remove`

Remove all performance measurements for commits that have been committed at or before the specified time (inclusive boundary, uses <=).

Note: Only published measurements (i.e., those that have been pushed to the remote repository) can be removed. Local unpublished measurements are not affected by this operation.

**Usage:** `git-perf remove --older-than <OLDER_THAN>`

###### **Options:**

* `--older-than <OLDER_THAN>`



## `git-perf prune`

Remove all performance measurements for non-existent/unreachable objects. Will refuse to work if run on a shallow clone

**Usage:** `git-perf prune`



## `git-perf list-commits`

List all commits that have performance measurements.

Outputs one commit SHA-1 hash per line. This can be used to identify which commits have measurements stored in the performance notes branch.

Example: git perf list-commits | wc -l  # Count commits with measurements git perf list-commits | head   # Show first few commits

**Usage:** `git-perf list-commits`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
