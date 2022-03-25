import pandas as pd
import plotly.express as px
import sys

records = []
commit = None
for line in sys.stdin.readlines():
    if line.startswith('--'):
        commit = line.split(" ")[1].strip()
        continue
    if len(line.strip()) == 0:
        continue
    if commit is None:
        print(
            f"Already have input but commit is unknown: '{line.rstrip()}'", file=sys.stderr)
        assert(False)
    data = line.strip()
    items = data.split(" ")
    if len(items) < 3:
        print(
            f"Too few items for commit {commit} in input line: '{line.rstrip()}'", file=sys.stderr)
        assert(False)
    name = items[0]
    time = items[1]
    val = items[2]
    kvs = items[3:]
    records.append({"commit": commit, "name": name, "time": pd.to_datetime(
        time, unit='s'), "val": val, "kvs": kvs})
df = pd.DataFrame(records)
df = df.join(df.kvs.explode().str.split(
    '=', expand=True).pivot_table(columns=0, aggfunc='last'))
df = df.drop(['kvs'], axis=1)
if 'os' in df.columns and 'runner_os' in df.columns:
    df['os'] = df['os'].fillna(df['runner_os'])
df = df.fillna("n/a")
print(df.to_markdown())
args = {
    'x': 'time',
    'y': 'val',
    'color': 'name',
    'hover_data': df.columns,
}
if 'os' in df.columns:
    args['symbol'] = 'os'
px.scatter(df, **args).write_html('result.html')
