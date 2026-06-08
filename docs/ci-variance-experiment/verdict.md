# Verdict Table

Based on best (min) inter-runner CoV across aggregation methods.


| OS | Workload | Best CoV (%) | Best Agg | Verdict |
|---|---|---|---|---|
| macos-latest | sha256 | 5.93% | min | GREEN |
| macos-latest | sort | 3.45% | min | GREEN |
| macos-latest | matrix | 3.57% | min | GREEN |
| macos-latest | noop | 18.91% | mean | YELLOW |
| ubuntu-22.04 | sha256 | 7.26% | median | GREEN |
| ubuntu-22.04 | sort | 5.41% | mean | GREEN |
| ubuntu-22.04 | matrix | 20.66% | median | RED |
| ubuntu-22.04 | noop | 12.07% | max | YELLOW |