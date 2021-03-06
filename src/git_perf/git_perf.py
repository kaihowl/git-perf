#!/usr/bin/env python3

# TODO(kaihowl) allow specifying an annotation that allows a deviation for the current commit
# TODO(kaihowl) show mean/stddev in report

import argparse
import time
import subprocess
from typing import List, Tuple
import sys
from icecream import ic  # type: ignore
from enum import Enum, auto
ic.configureOutput(outputFunction=print)
ic.disable()

PERF_REF = 'refs/notes/perf'
MAX_PUSHES = 3
DEFAULT_MAX_COUNT = 40
ACCEPT_PERF_TRAILER = 'accept-perf'
GIT_NOTES_BASE_COMMAND = ['git', 'notes', '--ref', PERF_REF]
GIT_LOG_BASE_COMMAND = ['git',
                        '--no-pager',
                        'log',
                        '--no-color',
                        '--ignore-missing',  # Support empty repo
                        ]


class MissingValuePolicy(Enum):
    SILENT = auto()
    WARN = auto()
    FAIL = auto()


KeyValueList = List[Tuple[str, str]]

parser = argparse.ArgumentParser(description="""
        Track performance measurements in git using git-notes.
""")
sub_parser = parser.add_subparsers(dest='subcommand', required=True)


def isnumeric(val: str):
    try:
        float(val)
    except ValueError:
        return False

    return True


def numerictype(val):
    if not isnumeric(val):
        raise argparse.ArgumentTypeError(f"'{val}' is not numeric")

    # Keep as string, this is parsed in Pandas to the most appropriate type
    return val


def add_measurement_option(parser, required=True):
    parser.add_argument(
        '--measurement', '-m', help='Name of measurement', required=required, default=None,
        dest='measurement')


def keyvalue(val) -> Tuple[str, str]:
    if len(val.split()) != 1:
        raise argparse.ArgumentTypeError(f"Key value pair '{val}' contains whitespace")

    split = val.strip().split('=')
    if len(split) != 2:
        raise argparse.ArgumentTypeError(
            f"Key value pair '{val}' does not follow 'key=value' format")

    return split


def add_kv_option(parser):
    parser.add_argument(
        '--key-value',
        '-kv',
        action='append',
        type=keyvalue,
        help='Key-value pair separated by "=" with no whitespaces',
        default=[])


# subcommand: measure
measure_parser = sub_parser.add_parser(
    "measure", help='Measure the supplied commands runtime')
add_kv_option(measure_parser)
add_measurement_option(measure_parser)
measure_parser.add_argument(
    '--number', '-n', help='Number of measurements', type=int, default=1)
measure_parser.add_argument(
    'command', nargs='+', help='Command to measure')

# subcommand: add
add_parser = sub_parser.add_parser('add', help='Add single measurement')
add_kv_option(add_parser)
add_measurement_option(add_parser)
add_parser.add_argument('value', help='Measured value to be added (numeric)', type=numerictype)

# subcommand: push
push_parser = sub_parser.add_parser(
    'push', help='Publish performance results to remote')

# subcommand: push
pull_parser = sub_parser.add_parser(
    'pull', help='Pull performance results from remote')

# subcommand: report
report_parser = sub_parser.add_parser(
    'report', help='Create an HTML performance report')
add_measurement_option(report_parser, required=False)
report_parser.add_argument('-o', '--output', help='HTML output file', default='result.html')
report_parser.add_argument('-s', '--separate-by', help='Create individual plots '
                           'by grouping with the value of this selector')
# TODO(kaihowl) repeated with below
report_parser.add_argument(
    '-n',
    '--max-count',
    help='Limit to the number of previous commits considered',
    type=int,
    default=DEFAULT_MAX_COUNT)
report_parser.add_argument(
    '-g',
    '--group-by',
    help='What to group the measurements by',
    default='commit')

# subcommand: audit
audit_parser = sub_parser.add_parser(
    'audit', help="""
    For a given measurement, check perfomance deviations of the HEAD commit
    against <n> previous commits. Group previous results and aggregate their
    results before comparison.
    """)
add_measurement_option(audit_parser)
audit_parser.add_argument(
    '--selector',
    '-s',
    action='append',
    type=keyvalue,
    help='Key-value pair separated by "=" with no whitespaces to subselect measurements',
    default=[])
audit_parser.add_argument(
    '-n',
    '--max-count',
    help='Limit to the number of commits considered',
    type=int,
    default=DEFAULT_MAX_COUNT)
audit_parser.add_argument(
    '-g',
    '--group-by',
    help='What to group the measurements by',
    default='commit')
audit_parser.add_argument(
    '-agg',
    '--aggregate-by',
    help='What to aggregate the measurements in each group with',
    default='min')
audit_parser.add_argument(
    '-d', '--sigma',
    help='Multiple of the stddev after which a outlier is detected. '
    'If the HEAD measurement is within [mean-<d>*sigma; mean+<d>*sigma], '
    'it is considered acceptable.',
    type=float,
    default=4)

# subcomand: good
good_parser = sub_parser.add_parser('good', help="""
        Accept HEAD commit's measurement for audit, even if outside of range.
        This is allows to accept expected performance changes.
        It will copy the current HEAD's measurements to the amended HEAD commit.
        """)
add_measurement_option(good_parser, required=True)
add_kv_option(good_parser)


# subcommand: prune
prune_parser = sub_parser.add_parser('prune', help="""
        Remove all performance measurements for non-existent/unreachable objects.
        Will refuse to work if run on a shallow clone.
        """)


def measure(measurement: str, number: int, key_value: KeyValueList, command: List[str]):
    for i in range(number):
        start = time.time_ns()
        subprocess.check_call(command)
        end = time.time_ns()
        value = end - start
        add(measurement, key_value, str(value))


def get_formatted_kvs(key_value: KeyValueList) -> str:
    return ' '.join([f"{key}={value}" for (key, value) in key_value])


def add(measurement: str, key_value: KeyValueList, value: str):
    formatted_measurement = f"{measurement} {time.time()} {value} {get_formatted_kvs(key_value)}"
    ic(formatted_measurement)

    notes_call = GIT_NOTES_BASE_COMMAND + ['append', '-m', formatted_measurement]
    subprocess.check_call(notes_call)


def pull():
    fetch()
    reconcile()


def push():
    counter = 0
    while counter < MAX_PUSHES and push_to_origin() != 0:
        pull()
        counter += 1


def push_to_origin() -> int:
    return subprocess.call(['git', 'push', 'origin', PERF_REF])


def fetch():
    subprocess.check_call(['git', 'fetch', 'origin', PERF_REF])


def reconcile():
    command = GIT_NOTES_BASE_COMMAND + [
        'merge',
        '-s',
        'cat_sort_uniq',
        'FETCH_HEAD']
    subprocess.check_call(command)


def get_raw_notes(max_count_commits: int) -> str:
    command = GIT_LOG_BASE_COMMAND + [
        '-n', str(max_count_commits),
        '--first-parent',  # only show the main branch history
        '--pretty=--,%H,%D%n%N',
        '--decorate=full',
        f"--notes={PERF_REF}",
        "HEAD"]  # pass in revision to allow using --ignore-missing for empty repos
    ic(command)
    return ic(subprocess.check_output(command, text=True))


def get_raw_pending_trailers():
    """
    Get all performance trailers in commit messages that are not merged, yet.
    This will take the total of all trailers found in 'HEAD^..'.
    If HEAD is a normal commit, only this commit is considered.
    If HEAD is a merge commit, all commits not merged into the first parent are considered.
    """
    command = GIT_LOG_BASE_COMMAND + [
        f'--format=%(trailers:key={ACCEPT_PERF_TRAILER},valueonly=true)',
        'HEAD^..',
    ]
    ic(command)
    return ic(subprocess.check_output(command, text=True))


def get_trailer_df():
    import pandas as pd  # type: ignore
    records = []
    for line in get_raw_pending_trailers().splitlines():
        if len(line.strip()) == 0:
            continue
        items = line.split()
        name = items[0]
        kvs = items[1:]
        records.append({
            'name': name,
            'kvs': kvs,
        })

    df = pd.DataFrame(records)
    df = expand_kvs(df)
    return df


def expand_kvs(df):
    df.index.name = 'num'
    if 'kvs' in df.columns:
        expanded = df.kvs.explode().str.split('=', expand=True)
        # Only continue if there are indeed kv pairs and not a kvs column of empty arrays
        # TODO(kaihowl) appears when adding only non-kv measurements, maybe handle earlier?
        if len(expanded.columns) == 2:
            # Explicitly rename columns. Using integer column names exposes
            # broken pivot_table in Pandas 1.4.2
            expanded.columns = ['key', 'value']
            pivoted = expanded.pivot_table(columns='key', index='num',
                                           values='value', aggfunc='last')
            df = df.drop(['kvs'], axis=1).join(pivoted)
    return df


def get_df(max_count_commits: int):
    """
    Retrieve a pandas dataframe containing the raw measurements.
    Returns a tuple (df, list_with_parsed_commits_hashes_backwards_order)
    """
    import pandas as pd  # type: ignore
    records = []
    commit = None
    commit_is_grafted = False
    commits_parsed = []

    for line in get_raw_notes(max_count_commits).splitlines():
        if line.startswith('--'):
            _, commit, *decorations = line.strip().split(",")
            commit_is_grafted = 'grafted' in decorations
            commits_parsed.append(commit)
            continue
        if len(line.strip()) == 0:
            continue
        data = line.strip()
        items = data.split()
        if commit is None:
            print(
                f"Already have input but commit is unknown: '{data}'", file=sys.stderr)
            assert(False)
            continue
        if len(items) < 3:
            print(
                f"Too few items for commit in input line: '{data}'", file=sys.stderr)
            assert(False)
            continue
        name = items[0]
        time = items[1]
        val = items[2]
        if not isnumeric(val):
            print(
                f"Found non-numeric value '{val}' as measurement "
                f"for commit {commit} in line: '{data}'",
                file=sys.stderr)
            assert(False)
            continue
        kvs = items[3:]
        records.append({
            "commit": commit,
            "nr_commit": len(commits_parsed),
            "name": name,
            "time": pd.to_datetime(time, unit='s'),
            "val": float(val),
            "kvs": kvs})
    number_commits = len(commits_parsed)
    if number_commits < max_count_commits and commit_is_grafted:
        print(f"Found {number_commits} "
              f"commit{'s' if number_commits > 1 else ''} instead of "
              f"the desired {max_count_commits} and "
              "we seem to have hit the boundary of a shallow clone",
              file=sys.stderr)
        sys.exit(1)
    df = pd.DataFrame(records)

    df = expand_kvs(df)

    return (df, commits_parsed)


def report(measurement: str,
           max_count: int,
           output: str,
           group_by: str,
           separate_by: str = None):

    import plotly.express as px  # type: ignore

    df, commits_parsed = get_df(max_count)

    # TODO(kaihowl) move check into get_df
    # TODO(kaihowl) check if perf notes is a valid ref
    if (len(df) == 0):
        print("No performance measurements found", file=sys.stderr)
        sys.exit(1)

    if measurement:
        df = filter_df(df, [('name', measurement)])

    # TODO(kaihowl) add test for reverse ordering
    df = df[::-1]
    df = df.fillna("n/a")
    ic(df.to_markdown())

    if group_by not in df.columns:
        print(f"Argument for --group_by invalid: {group_by}",
              file=sys.stderr)
        sys.exit(1)

    args = {
        'x': group_by,
        'y': 'val',
        'points': 'all',
        'hover_data': df.columns,
    }

    is_group_by_commit = group_by == 'commit'

    if is_group_by_commit:
        args['x'] = 'nr_commit'

    if separate_by:
        if separate_by not in df.columns:
            print(f"Argument for --separate-by invalid: {separate_by} "
                  "not found in columns", file=sys.stderr)
            sys.exit(1)
        args['color'] = separate_by
        args['category_orders'] = {separate_by: df[separate_by].unique()}

    if (len(df) == 0):
        print("No performance measurements after filtering found", file=sys.stderr)
        sys.exit(1)

    with open(output, 'w') as f:
        f.write('<h1>Performance Measurements</h1>')
        for name in df.name.unique():
            fig = px.box(df[df.name == name], **args)
            fig.update_yaxes(matches=None)

            if is_group_by_commit:
                fig.update_xaxes(
                    tickvals=list(range(1, len(commits_parsed)-1)),
                    ticktext=commits_parsed,
                    title='',
                    # TODO(kaihowl) this is a double reverse with ::-1 above
                    autorange='reversed')

            f.write(f'<h2>{name}</h2>')
            # TODO(kaihowl) make include_plotlyjs configurable
            f.write(fig.to_html(include_plotlyjs='cdn', full_html=False))


def filter_df(df,
              selector: KeyValueList,
              missing: MissingValuePolicy = MissingValuePolicy.FAIL):
    for (key, value) in selector:
        if key not in df.columns:
            message = f"Selector '{key}' does not exist"
            if missing == MissingValuePolicy.SILENT:
                continue
            elif missing == MissingValuePolicy.WARN:
                print(message, file=sys.stderr)
            else:
                raise ValueError(message)
        df = df[df[key] == value]
    return df


def summarize(df, group_by: str, aggregate_by: str) -> Tuple[float, float]:
    import numpy as np
    if (len(df) == 0):
        return (np.nan, np.nan)
    group = df.groupby(group_by).val.agg(aggregate_by)
    return (group.mean(), group.std())


def audit(measurement: str,
          max_count: int,
          group_by: str,
          aggregate_by: str,
          selector: KeyValueList,
          sigma: float):
    import numpy as np

    df, commits = get_df(max_count)

    # TODO(kaihowl) test missing
    if len(df) == 0:
        print("No performance measurements found", file=sys.stderr)
        sys.exit(1)

    ic(df.to_markdown())

    df_head = df[df['commit'] == commits[0]]
    df_tail = df[df['commit'] != commits[0]]

    if len(df_head) == 0:
        print("No performance measurements on HEAD commit", file=sys.stderr)
        sys.exit(1)

    trailers = get_trailer_df()

    selector.append(('name', measurement))
    df_head = filter_df(df_head, selector, missing=MissingValuePolicy.FAIL)
    df_tail = filter_df(df_tail, selector, missing=MissingValuePolicy.WARN)
    trailers = filter_df(trailers, selector, missing=MissingValuePolicy.SILENT)

    accept_regression = len(trailers) > 0

    ic(df_head.to_markdown())
    ic(df_tail.to_markdown())

    tail_mean, tail_std = summarize(df_tail, group_by, aggregate_by)
    print(f"mean: {tail_mean}")
    print(f"std: {tail_std}")

    head_mean, _ = summarize(df_head, group_by, aggregate_by)
    print(f"head_mean: {head_mean}")
    print(f"sigma: {sigma}")

    if np.isnan(head_mean):
        print("No matching measurements in HEAD commit", file=sys.stderr)
        sys.exit(1)

    if np.isnan(tail_std):
        print("Not enough historical data available", file=sys.stderr)
        return

    # If the historical measurements are perfectly stable, stddev == 0,
    # then z should be positive infinity to always trigger a deviation.
    # This uses np.float64 objects which have
    # 0/0 = np.nan(0)
    # z/0 = np.nan(+inf) if z > 0
    with np.errstate(divide='ignore'):
        z = abs(head_mean - tail_mean) / tail_std
    z = np.nan_to_num(z)
    print(f"z-score: {z}")
    is_regular = z <= sigma
    print(f"is regular: {z} <= {sigma} == {is_regular}")
    print(f"accept regression: {accept_regression}")
    accepted = is_regular or accept_regression
    sys.exit(not accepted)


def mark_as_good(measurement: str, key_value: KeyValueList):
    command = ['git',
               '--no-pager',
               'commit',
               '--amend',
               '--no-edit',
               '--trailer', f"{ACCEPT_PERF_TRAILER}: {measurement} {get_formatted_kvs(key_value)}",
               ]
    ic(command)
    return ic(subprocess.check_output(command, text=True))


def copy_measurements_from_prev_head():
    command = GIT_NOTES_BASE_COMMAND + [
        'list',
        'HEAD@{1}',
    ]
    ic(command)
    has_notes = ic(subprocess.call(command, text=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)) == 0
    if not has_notes:
        return

    command = GIT_NOTES_BASE_COMMAND + [
        'copy',
        '-f',
        'HEAD@{1}',
        'HEAD',
    ]
    ic(command)
    return ic(subprocess.check_output(command))


def good(measurement: str, key_value: KeyValueList):
    mark_as_good(measurement, key_value)
    copy_measurements_from_prev_head()


def is_shallow_repo() -> bool:
    command = ['git', 'rev-parse', '--is-shallow-repository']
    ic(command)
    text = ic(subprocess.check_output(command, text=True))
    return text.strip() == 'true'


def prune():
    if is_shallow_repo():
        print("Refusing to prune in a shallow clone", file=sys.stderr)
        sys.exit(1)

    command = GIT_NOTES_BASE_COMMAND + ['prune']
    ic(command)
    ic(subprocess.check_call(command))


def main():
    args = parser.parse_args()
    subcommand = args.subcommand
    del args.subcommand
    globals()[subcommand](**vars(args))


if __name__ == '__main__':
    main()
