#!/bin/bash

set -e
set -x

git --no-pager log --no-color --pretty='-- %H%n%N' --notes=refs/notes/perf > results.txt

python3 <<EOF
import pandas as pd

records = []
f = open('results.txt')
commit = None
for line in f.readlines():
  if line.startswith('--'):
    commit = line.split(" ")[1].strip()
    continue
  if len(line.strip()) == 0:
    continue
  if commit == None:
    assert(False)
  data = line.strip()
  items = data.split(" ")
  if len(items) < 3:
    assert(False)
  name = items[0]
  time = items[1]
  val = items[2]
  kvs = items[3:]
  records.append({"commit": commit, "name": name, "time": pd.to_datetime(time, unit='s'), "val": val, "kvs": kvs})
df = pd.DataFrame(records)
df = df.join(df.kvs.explode().str.split('=', expand=True).pivot(columns=0)[1])
print(df.to_markdown())
EOF
