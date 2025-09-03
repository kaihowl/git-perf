git-perf
================
# NAME

git-perf

# SYNOPSIS

**git-perf** \[**-v**|**--verbose**\]... \[**-h**|**--help**\]
\[**-V**|**--version**\] \<*subcommands*\>

# DESCRIPTION

# OPTIONS

  - **-v**, **--verbose**  
    Increase verbosity level (can be specified multiple times.) The
    first level sets level "info", second sets level "debug", and third
    sets level "trace" for the logger

  - **-h**, **--help**  
    Print help

  - **-V**, **--version**  
    Print version

# SUBCOMMANDS

  - git-perf-measure(1)  
    Measure the runtime of the supplied command (in nanoseconds)

  - git-perf-add(1)  
    Add single measurement

  - git-perf-push(1)  
    Publish performance results to remote

  - git-perf-pull(1)  
    Pull performance results from remote

  - git-perf-report(1)  
    Create an HTML performance report

  - git-perf-audit(1)  
    For given measurements, check perfomance deviations of the HEAD
    commit against \`\<n\>\` previous commits. Group previous results
    and aggregate their results before comparison

  - git-perf-bump-epoch(1)  
    Accept HEAD commits measurement for audit, even if outside of range.
    This is allows to accept expected performance changes. This is
    accomplished by starting a new epoch for the given measurement. The
    epoch is configured in the git perf config file. A change to the
    epoch therefore has to be committed and will result in a new HEAD
    for which new measurements have to be taken

  - git-perf-remove(1)  
    Remove all performance measurements for commits that have been
    committed before the specified time period

  - git-perf-prune(1)  
    Remove all performance measurements for non-existent/unreachable
    objects. Will refuse to work if run on a shallow clone

  - git-perf-help(1)  
    Print this message or the help of the given subcommand(s)

# VERSION

v0.0.0



git-perf-add
================
# NAME

add - Add single measurement

# SYNOPSIS

**add** \<**-m**|**--measurement**\> \[**-k**|**--key-value**\]
\[**-h**|**--help**\] \<*VALUE*\>

# DESCRIPTION

Add single measurement

# OPTIONS

  - **-m**, **--measurement**=*NAME*  
    Name of the measurement

  - **-k**, **--key-value**=*KEY\_VALUE*  
    Key-value pairs separated by =

  - **-h**, **--help**  
    Print help

  - \<*VALUE*\>  
    Measured value to be added



git-perf-audit
================
# NAME

audit - For given measurements, check perfomance deviations of the HEAD
commit against \`\<n\>\` previous commits. Group previous results and
aggregate their results before comparison

# SYNOPSIS

**audit** \<**-m**|**--measurement**\> \[**-n**|**--max-count**\]
\[**-s**|**--selectors**\] \[**--min-measurements**\]
\[**-a**|**--aggregate-by**\] \[**-d**|**--sigma**\]
\[**-D**|**--dispersion-method**\] \[**-h**|**--help**\]

# DESCRIPTION

For given measurements, check perfomance deviations of the HEAD commit
against \`\<n\>\` previous commits. Group previous results and aggregate
their results before comparison.

The audit can be configured to ignore statistically significant
deviations if they are below a minimum relative deviation threshold.
This helps filter out noise while still catching meaningful performance
changes.

Configuration is done via the \`.gitperfconfig\` file: -
Measurement-specific:
\`\[audit.measurement."name"\].min\_relative\_deviation = 10.0\` -
Global: \`\[audit.global\].min\_relative\_deviation = 5.0\`

When the relative deviation is below the threshold, the audit passes
even if the z-score exceeds the sigma threshold. The relative deviation
is calculated as: \`|(head\_value / tail\_median - 1.0) \* 100%|\` where
tail\_median is the median of historical measurements (excluding HEAD).

The sparkline visualization shows the range of measurements relative to
the tail median (historical measurements only).

# OPTIONS

  - **-m**, **--measurement**=*MEASUREMENT*  

<!-- end list -->

  - **-n**, **--max-count**=*MAX\_COUNT* \[default: 40\]  
    Limit the number of previous commits considered. HEAD is included in
    this count

  - **-s**, **--selectors**=*SELECTORS*  
    Key-value pair separated by "=" with no whitespaces to subselect
    measurements

  - **--min-measurements**=*MIN\_MEASUREMENTS* \[default: 2\]  
    Minimum number of measurements needed. If less, pass test and assume
    more measurements are needed. A minimum of two historic measurements
    are needed for proper evaluation of standard deviation

  - **-a**, **--aggregate-by**=*AGGREGATE\_BY* \[default: min\]  
    What to aggregate the measurements in each group with  

  
\[*possible values: *min, max, median, mean\]

  - **-d**, **--sigma**=*SIGMA* \[default: 4.0\]  
    Multiple of the stddev after which a outlier is detected. If the
    HEAD measurement is within \`\[mean-\<d\>\*sigma;
    mean+\<d\>\*sigma\]\`, it is considered acceptable

  - **-D**, **--dispersion-method**=*DISPERSION\_METHOD*  
    Method for calculating statistical dispersion (stddev or mad). If
    not specified, uses the value from .gitperfconfig file, or defaults
    to stddev  

  
\[*possible values: *stddev, mad\]

  - **-h**, **--help**  
    Print help (see a summary with -h)



git-perf-bump-epoch
================
# NAME

bump-epoch - Accept HEAD commits measurement for audit, even if outside
of range. This is allows to accept expected performance changes. This is
accomplished by starting a new epoch for the given measurement. The
epoch is configured in the git perf config file. A change to the epoch
therefore has to be committed and will result in a new HEAD for which
new measurements have to be taken

# SYNOPSIS

**bump-epoch** \<**-m**|**--measurement**\> \[**-h**|**--help**\]

# DESCRIPTION

Accept HEAD commits measurement for audit, even if outside of range.
This is allows to accept expected performance changes. This is
accomplished by starting a new epoch for the given measurement. The
epoch is configured in the git perf config file. A change to the epoch
therefore has to be committed and will result in a new HEAD for which
new measurements have to be taken

# OPTIONS

  - **-m**, **--measurement**=*MEASUREMENT*  

<!-- end list -->

  - **-h**, **--help**  
    Print help



git-perf-measure
================
# NAME

measure - Measure the runtime of the supplied command (in nanoseconds)

# SYNOPSIS

**measure** \[**-n**|**--repetitions**\] \<**-m**|**--measurement**\>
\[**-k**|**--key-value**\] \[**-h**|**--help**\] \<*COMMAND*\>

# DESCRIPTION

Measure the runtime of the supplied command (in nanoseconds)

# OPTIONS

  - **-n**, **--repetitions**=*REPETITIONS* \[default: 1\]  
    Repetitions

  - **-m**, **--measurement**=*NAME*  
    Name of the measurement

  - **-k**, **--key-value**=*KEY\_VALUE*  
    Key-value pairs separated by =

  - **-h**, **--help**  
    Print help

  - \<*COMMAND*\>  
    Command to measure



git-perf-prune
================
# NAME

prune - Remove all performance measurements for non-existent/unreachable
objects. Will refuse to work if run on a shallow clone

# SYNOPSIS

**prune** \[**-h**|**--help**\]

# DESCRIPTION

Remove all performance measurements for non-existent/unreachable
objects. Will refuse to work if run on a shallow clone

# OPTIONS

  - **-h**, **--help**  
    Print help



git-perf-pull
================
# NAME

pull - Pull performance results from remote

# SYNOPSIS

**pull** \[**-h**|**--help**\]

# DESCRIPTION

Pull performance results from remote

# OPTIONS

  - **-h**, **--help**  
    Print help



git-perf-push
================
# NAME

push - Publish performance results to remote

# SYNOPSIS

**push** \[**-h**|**--help**\]

# DESCRIPTION

Publish performance results to remote

# OPTIONS

  - **-h**, **--help**  
    Print help



git-perf-remove
================
# NAME

remove - Remove all performance measurements for commits that have been
committed before the specified time period

# SYNOPSIS

**remove** \<**--older-than**\> \[**-h**|**--help**\]

# DESCRIPTION

Remove all performance measurements for commits that have been committed
before the specified time period

# OPTIONS

  - **--older-than**=*OLDER\_THAN*  

<!-- end list -->

  - **-h**, **--help**  
    Print help



git-perf-report
================
# NAME

report - Create an HTML performance report

# SYNOPSIS

**report** \[**-o**|**--output**\] \[**-n**|**--max-count**\]
\[**-m**|**--measurement**\] \[**-k**|**--key-value**\]
\[**-s**|**--separate-by**\] \[**-a**|**--aggregate-by**\]
\[**-h**|**--help**\]

# DESCRIPTION

Create an HTML performance report

# OPTIONS

  - **-o**, **--output**=*OUTPUT* \[default: output.html\]  
    HTML output file

  - **-n**, **--max-count**=*MAX\_COUNT* \[default: 40\]  
    Limit the number of previous commits considered. HEAD is included in
    this count

  - **-m**, **--measurement**=*MEASUREMENT*  
    Select an individual measurements instead of all

  - **-k**, **--key-value**=*KEY\_VALUE*  
    Key-value pairs separated by =, select only matching measurements

  - **-s**, **--separate-by**=*SEPARATE\_BY*  
    Create individual traces in the graph by grouping with the value of
    this selector

  - **-a**, **--aggregate-by**=*AGGREGATE\_BY*  
    What to aggregate the measurements in each group with  

  
\[*possible values: *min, max, median, mean\]

  - **-h**, **--help**  
    Print help



