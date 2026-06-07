# Benchmark Study Guide

Before adding a new benchmark to your CI performance suite, run a **benchmark study** to measure its cross-runner variability (Coefficient of Variation) and get recommended `.gitperfconfig` settings. This prevents noisy benchmarks from causing false audit failures after merging.

## Why a study?

Performance benchmarks on shared CI infrastructure vary between runner instances due to hardware differences, load, and scheduling. A benchmark with 15% cross-runner CoV will trigger false regression alerts constantly. The study workflow measures this variance and tells you exactly how to configure the benchmark to tolerate it.

## Setup (once per repository)

Add a wrapper workflow to your repo so you can trigger studies on demand:

```yaml
# .github/workflows/benchmark-study.yml
on:
  workflow_dispatch:
    inputs:
      measurement:
        description: Full measurement name (e.g. bench::my_bench/op::min)
        required: true
        type: string
      command:
        description: Command that runs the benchmark
        required: true
        type: string
      instances:
        description: Number of independent runners (≥ 3; default 10)
        required: false
        default: '10'
        type: string
      reps_per_instance:
        description: Repetitions per runner passed to git-perf measure -n
        required: false
        default: '10'
        type: string
      max_cov:
        description: Fail if CoV exceeds this % (leave empty to only report)
        required: false
        default: ''
        type: string

jobs:
  study:
    uses: kaihowl/git-perf/.github/workflows/benchmark-study.yml@VERSION
    with:
      measurement: ${{ inputs.measurement }}
      command: ${{ inputs.command }}
      instances: ${{ inputs.instances }}
      reps_per_instance: ${{ inputs.reps_per_instance }}
      max_cov: ${{ inputs.max_cov }}
    secrets: inherit
```

Replace `VERSION` with a specific git-perf release tag (e.g. `v0.18.0`) for stability.

**Prerequisites**: The reusable workflow installs git-perf automatically using the [install action](../../../.github/actions/install). Your repository must have push access to the git-perf notes ref (`refs/notes/perf-v3`), which requires `contents: write` permission for the workflow.

## When to run a study

- **When adding a new benchmark** to CI before merging the PR
- **When an existing benchmark starts producing noisy audits** (unexplained failures)
- **After changing a benchmark's workload**, setup, or iteration count

## How to trigger it

1. Push your PR branch
2. Go to **Actions → Benchmark Study → Run workflow**
3. Select your PR branch
4. Enter:
   - **measurement**: the exact name git-perf will use (e.g. `bench::my_bench/sorting/1000::median`)
   - **command**: the shell command that runs the benchmark (e.g. `cargo bench --bench my_bench`)
   - **instances**: number of parallel runners (default 10; minimum 3)
   - **reps_per_instance**: repetitions per runner (default 10)
5. Click **Run workflow** and wait for the `study` job to finish

The study output appears in the **Analyze cross-runner variance** job log.

## How it works

The workflow runs in two stages:

1. **Measure** (parallel): Each of the N runner instances runs the benchmark independently and stores its measurements via `git-perf measure --key-value group=<instance>` then pushes them to the remote notes ref.

2. **Study** (sequential): After all measure jobs finish, pulls all measurements and runs `git-perf study -m MEASUREMENT`. The command groups measurements by the `group` key, computes a **per-group minimum** (to reduce within-runner noise), then computes statistics over those N per-group values to produce the **between-runner CoV**.

## Interpreting results

The study output looks like:

```
📊 'bench::my_benchmark/sort/1000::min' — 10 groups × 10 reps (grouped by: group)
  μ: 45.2µs | σ: 3.8µs | MAD: 2.1µs
  Between-group CoV: 8.4% | MAD%: 4.6% | MAD/σ: 0.55

  ✅ CoV < 10%: benchmark is stable.

  Recommended .gitperfconfig:
  [measurement."bench::my_benchmark/sort/1000::min"]
  dispersion_method = "mad"  # MAD/σ = 0.55 — outliers between runners detected
  sigma = 3.5                # tightened threshold for CoV > 5%
  aggregate_by = "min"
  min_measurements = 3
  min_relative_deviation = 13.0  # 1.5× between-group CoV — noise floor
  max_cov = 17.0                 # warn if noise grows to 2× current level
```

**CoV verdict**:

| CoV | Verdict | Action |
|-----|---------|--------|
| < 10% | ✅ Stable | Use the recommended config and merge |
| 10–20% | ⚠️ Moderate | Use the config, monitor with `max_cov` |
| > 20% | ⚠️ Noisy | Improve benchmark isolation before merging |

## Applying the recommendations

1. Copy the `[measurement."..."]` TOML block from the study output
2. Paste it into `.gitperfconfig` in your repository
3. Commit it alongside the new benchmark

The config block tells git-perf's `audit` command how to interpret this benchmark's noise level, preventing false alarms.

## What to do if CoV is too high

If your benchmark shows CoV > 20%, consider:

- **Increase workload size**: more work per iteration reduces relative setup noise
- **Add warmup**: run the benchmark once before measuring (e.g. `--warm-up-time 3s` in Criterion)
- **Isolate I/O**: avoid filesystem, network, or database operations in the timed section
- **Increase `reps_per_instance`**: more reps per runner let `min` aggregation absorb outliers
- **Pin to a fixed-spec runner**: use `runs-on: [self-hosted, perf]` with dedicated hardware

After improving isolation, re-run the study and compare the new CoV.

## Using the study as a CI gate

To prevent noisy benchmarks from being merged, add `max_cov` to the study trigger:

```yaml
# In your PR workflow
- name: Benchmark stability gate
  uses: kaihowl/git-perf/.github/workflows/benchmark-study.yml@VERSION
  with:
    measurement: bench::my_bench/op::min
    command: cargo bench --bench my_bench
    max_cov: '20'   # Fail if CoV exceeds 20%
```

The workflow exits with code 1 if the measured CoV exceeds `max_cov`, blocking the PR merge.

## See also

- [Integration Tutorial](INTEGRATION_TUTORIAL.md) — full CI setup guide
- [`.gitperfconfig` reference](manpage.md) — all available config options
- `git perf study --help` — CLI reference
