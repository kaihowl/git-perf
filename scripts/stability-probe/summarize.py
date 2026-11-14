#!/usr/bin/env python3
"""Summarize probe CSVs: how much does each metric vary within a runner,
across runners, and across CPU models? Usage: summarize.py <csv>..."""
import collections, statistics as st, sys

rows = collections.defaultdict(list)  # (workload, metric) -> [(runner, cpu, value)]
for path in sys.argv[1:]:
    for line in open(path):
        runner, cpu, w, _rep, m, v = line.rstrip("\n").split(",", 5)
        if v not in ("NA", ""):
            rows[(w, m)].append((runner, cpu, float(v)))

def cov(xs):
    return 100 * st.pstdev(xs) / st.mean(xs) if len(xs) > 1 and st.mean(xs) else 0.0

print("| workload | metric | runners | CPUs | within-runner CoV % | across-runner CoV % | max/min of CPU medians | worst same-CPU CoV % |")
print("|---|---|---|---|---|---|---|---|")
for (w, m), vals in sorted(rows.items()):
    if w.startswith("report-aa"):
        continue
    per_runner = collections.defaultdict(list); cpu_of = {}
    for r, c, v in vals:
        per_runner[r].append(v); cpu_of[r] = c
    med = {r: st.median(v) for r, v in per_runner.items()}
    within = st.median([cov(v) for v in per_runner.values()])
    across = cov(list(med.values()))
    by_cpu = collections.defaultdict(list)
    for r, v in med.items():
        by_cpu[cpu_of[r]].append(v)
    cpu_meds = [st.median(v) for v in by_cpu.values()]
    ratio = max(cpu_meds) / min(cpu_meds) if min(cpu_meds) > 0 else float("nan")
    same_cpu = max([cov(v) for v in by_cpu.values() if len(v) > 1], default=float("nan"))
    print(f"| {w} | {m} | {len(med)} | {len(by_cpu)} | {within:.2f} | {across:.2f} | {ratio:.3f} | {same_cpu:.2f} |")

# A/A: identical binaries interleaved on the same runner
aa = collections.defaultdict(lambda: collections.defaultdict(list)); cpu_of = {}
for w in ("report-aa-a", "report-aa-b"):
    for r, c, v in rows.get((w, "wall_ns"), []):
        aa[r][w].append(v); cpu_of[r] = c
if aa:
    print("\n### A/A test: same binary, interleaved on one runner (report, wall time)\n")
    print("| runner | cpu | median(a)/median(b) - 1 (%) |\n|---|---|---|")
    diffs = []
    for r, d in sorted(aa.items()):
        x = 100 * (st.median(d["report-aa-a"]) / st.median(d["report-aa-b"]) - 1); diffs.append(x)
        print(f"| {r} | {cpu_of[r]} | {x:+.2f} |")
    print(f"\nA/A spread across runners: stdev {st.pstdev(diffs):.2f} pp, max |diff| {max(map(abs, diffs)):.2f} %")
