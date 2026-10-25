# Verdict Table

Based on best (min) inter-runner CoV across aggregation methods.


| OS | Workload | Best CoV (%) | Best Agg | Verdict |
|---|---|---|---|---|
| macos-latest | sha256 | 2.97% | min | GREEN |
| macos-latest | sort | 2.98% | min | GREEN |
| macos-latest | matrix | 2.75% | min | GREEN |
| macos-latest | noop | 14.17% | mean | YELLOW |
| ubuntu-22.04 | sha256 | 10.33% | max | YELLOW |
| ubuntu-22.04 | sort | 9.62% | max | GREEN |
| ubuntu-22.04 | matrix | 22.91% | mean | RED |
| ubuntu-22.04 | noop | 11.05% | max | YELLOW |