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
        assert(False)
    data = line.strip()
    items = data.split(" ")
    if len(items) < 3:
        assert(False)
    name = items[0]
    time = items[1]
    val = items[2]
    kvs = items[3:]
    records.append({"commit": commit, "name": name, "time": pd.to_datetime(
        time, unit='s'), "val": val, "kvs": kvs})
df = pd.DataFrame(records)
df = df.join(df.kvs.explode().str.split('=', expand=True).pivot(columns=0)[1])
df = df.drop(['kvs'], axis=1)
df['os'] = df['os'].fillna(df['runner_os'])
df = df.fillna("n/a")
# print(df.to_markdown())
px.scatter(df, x='time', y='val', color='name', symbol='os',
           hover_data=df.columns).write_html('result.html')
