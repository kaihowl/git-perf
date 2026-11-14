# Stabilizing CI Benchmarks Across Runner CPU Models

## Problem

GitHub's standard `ubuntu-*` runners are drawn from a pool of Azure (and,
for `ubuntu-24.04`, AWS) VM types with different CPUs. The same git-perf
workload takes up to **2.8× longer** on one model than on another. CI now
audits timings per CPU model (`separate_by = ["os", "cpu"]`). That removed
the false failures, but it split the history: a regression is caught only
when a commit happens to run on a CPU model that already has
`min_measurements` of history, and only EPYC 7763 is trusted to gate.

This document looks at ways to remove the CPU-model dependence and checks
each one on real GitHub runners.

## Experiment

`.github/workflows/stability-probe.yml` runs `scripts/stability-probe/probe.sh`
on many runners in parallel. Each runner builds git-perf and measures the
CI benchmark workloads on a fixture with fixed data and dates:
`git perf add` and `git perf report -n 10`. These are the same commands the
`add-benchmark` and `report-benchmark` measurements use. For each one the
probe records:

- **wall time**
- **rusage**: page faults, max RSS, block I/O, context switches
- **syscall count** (`strace -c -f`)
- **hardware counters** (`perf stat`, following child `git` processes)
- **Valgrind/Cachegrind**: instruction count and *simulated* cache misses,
  with a fixed cache geometry so the result does not depend on the host
  CPU's caches
- **A/A test**: two copies of the same binary run alternately on the same
  runner; the ratio of their medians shows the noise of same-runner
  relative timing

There were three runs:

- [run 43581513383](https://github.com/kaihowl/git-perf/actions/runs/43581513383): 16 runners
- [run 43581903411](https://github.com/kaihowl/git-perf/actions/runs/43581903411): 23 runners
- [run 43582275808](https://github.com/kaihowl/git-perf/actions/runs/43582275808): 13 runners, with `perf` fixed to run as the runner user

### Runner pool observed (52 x86/ARM jobs)

| CPU | Azure size / host | PMU (hardware counters) | Seen on |
|---|---|---|---|
| AMD EPYC 7763 | Standard_D4ads_v5 | ❌ not exposed | 22.04, 24.04 |
| AMD EPYC 9V74 | Standard_D4ads_v6 | ❌ | 22.04, 24.04 |
| AMD EPYC 9V45 | Standard_D4ads_v7 | ❌ | 22.04, 24.04 |
| Intel Xeon Platinum 8573C | Standard_D4ds_v5 | ❌ | 22.04, 24.04 |
| Intel Xeon 6973P-C | Standard_D4ds_v7 | ❌ | 22.04 |
| Intel Xeon 6975P-C | **AWS m8id.xlarge** | ✅ core events only (no cache events) | 24.04 only |
| Neoverse-N2 (Cobalt 100) | Standard_D4pds_v6 | ✅ incl. cache events | 24.04-arm (8/8 jobs) |

Other observations:

- The Azure instance metadata service is reachable from jobs, so the VM size
  can be read with `curl -H Metadata:true 169.254.169.254/...`.
- `sudo sysctl kernel.perf_event_paranoid=-1` works on all runners. On the
  Azure x86 VMs, though, there is no `cpu` PMU, so every hardware event
  reads `<not supported>`.

### Results: `report` workload, per-runner medians, ubuntu-22.04 (3 CPU models)

| CPU | wall time (ms) | Valgrind instructions | simulated LL misses | syscalls | minor faults |
|---|---|---|---|---|---|
| EPYC 9V45 | 12.6 – 13.3 | 29,478,651 – 29,483,260 | 219,075 – 219,132 | 6,555 – 6,557 | 3,433 – 3,441 |
| EPYC 9V74 | 28.5 – 29.7 | 29,479,168 – 29,481,502 | 219,086 – 219,111 | 6,557 | 3,435 – 3,446 |
| EPYC 7763 | 34.7 – 35.1 | 29,479,810 – 29,480,882 | 219,083 – 219,097 | 6,557 | 3,415 – 3,420 |

### Results: variation summary (CoV in %; ratio = slowest/fastest CPU median)

| metric | within-runner CoV | across-runner CoV (same image) | CPU max/min | notes |
|---|---|---|---|---|
| **wall time** | 1.3 – 3.4 | **20 – 38** | **1.6 – 2.7×** | same-CPU cross-runner CoV 2 – 6 % |
| task-clock / utime | 2 – 24 | 12 – 25 | 1.4 – 1.8× | CPU time is speed-dependent too |
| **Valgrind instructions** | ≤ 0.01 | **0.00 – 0.11** | 1.000 – 1.003 | deterministic; ~0.17 % Intel-vs-AMD offset from CPUID-based dispatch |
| Valgrind simulated D1/LL misses | ≤ 0.02 | 0.01 – 0.28 | ≤ 1.005 | memory-access proxy |
| **perf `instructions:u`** (AWS x86, ARM) | ≤ 0.02 | **0.01 – 0.05** | n/a | only on runners with a PMU |
| perf `cycles:u` (AWS, ARM) | 1 – 7 | 1 – 3 | n/a | speed-dependent, but these pools are homogeneous |
| perf `cache-misses` (ARM) | 0.6 – 0.9 | ~1.1 | n/a | not available on AWS x86 |
| syscalls | ≤ 0.04 | ≤ 0.04 | 1.000 | I/O-work proxy |
| minor page faults | 0.15 – 0.23 | 0.26 – 0.33 | ≤ 1.010 | memory-work proxy |
| max RSS | 0.4 – 0.7 | 0.5 – 0.7 | ≤ 1.010 | |
| ARM wall time (Neoverse-N2 only) | 1.7 – 5 | **0.2 – 3** | 1 model | single CPU model so far |
| **A/A same-runner relative timing** | — | stdev **0.5 – 0.75 pp**, max 2.2 % (x86) | — | 1.5 pp stdev on ARM (3 runners) |

The ubuntu-22.04 and ubuntu-24.04 images give different counts: 29.48M
instructions and 6,557 syscalls on 22.04, against 29.33M and 4,286 on 24.04.
The cause is a different `git` and `glibc`. Count-based metrics therefore
depend on the software image, not on the CPU. Pooling across both images
(run 1) inflated the across-runner CoV of syscalls to 12 % and of max RSS
to 7 %.

## Options

### 1. Measure work instead of time (speed-independent metrics)

#### 1a. Valgrind instruction counts and simulated cache misses — feasible now ✅

- **Feasibility:** works on every standard runner (`apt-get install valgrind`).
  With `--trace-children=yes` it also counts the `git` subprocesses.
- **Stability:** across 5 CPU models and 2 vendors it is 100–1000× more
  stable than wall time (CoV ≤ 0.11 % vs 20–38 %). History can be pooled
  across all runners, so `separate_by = ["os"]` is enough.
- **Cost:** about 30–50× slowdown. That is fine for the current
  10–30 ms workloads: the whole probe, including 6 Valgrind runs, takes
  about 1 minute. Criterion benches should be run with a fixed iteration
  count, or through [gungraun](https://github.com/gungraun/gungraun)
  (formerly iai-callgrind), which is built for this.
- **What it catches:** extra CPU work (instructions), and memory-access
  regressions through simulated L1/LL misses. CodSpeed's "CPU simulation"
  mode combines these into an estimated-cycles figure
  (`Ir + 10·L1m + 100·LLm`), which git-perf could store as one measurement.
- **What it misses:** time spent waiting on I/O, locks, sleeps or
  timeouts; parallelism and threading changes (Valgrind serializes
  threads); branch mispredictions; real memory bandwidth effects.
- **Caveats:**
  - A runner image update that bumps `git` or `glibc` shifts the counts.
    Record `git --version` as metadata, add it to `separate_by`, or set an
    `epoch` when the image changes.
  - CPUID-dependent code paths cause a ~0.2 % offset between Intel and AMD,
    far below any useful threshold.

#### 1b. Hardware performance counters (`perf stat`) — not feasible on x86, feasible on ARM ⚠️

- **x86:** not feasible on the Azure x86 runners, which are most
  `ubuntu-22.04` jobs and about half of `ubuntu-24.04`. They expose no PMU,
  as earlier reports also found
  ([actions/runner-images#4974](https://github.com/actions/runner-images/issues/4974)).
- **AWS-hosted 24.04 runners:** `instructions:u`, `cycles:u` and
  `branches:u` work, but cache events read 0. Which host a job gets cannot
  be controlled.
- **ARM:** `ubuntu-24.04-arm` (Cobalt 100) exposes the full ARMv8 PMU.
  `instructions:u` repeats to within 0.01–0.05 % and runs at native speed,
  with no Valgrind slowdown.

#### 1c. OS-level resource counters (rusage, syscalls) — feasible, complementary ✅

- **Syscall count:** deterministic (CoV ≤ 0.04 %) and a good proxy for
  "storage work", for example extra `git` invocations or extra
  reads/writes. Cheap: `strace -c -f` costs less than Valgrind. A
  `perf stat -e raw_syscalls:sys_enter` tracepoint also works as root.
- **Minor page faults and max RSS:** stable to about 0.3–0.7 %. They catch
  memory-footprint regressions.
- **Block I/O counters:** always 0 or constant, because everything comes
  from the page cache. Not useful.

### 2. Pin the hardware

#### 2a. Pin a GitHub-hosted runner to a CPU model — not possible ❌

- **Standard runners:** labels choose the image, not the VM size. The CPU
  model can only be detected (from `/proc/cpuinfo` or the Azure IMDS
  `vmSize`), not selected.
- **Rejection sampling** (launch k jobs, keep the one that lands on the
  target CPU): EPYC 7763 appeared in 7 of 44 x86 jobs (16 %), so k = 5
  gives only about 58 % odds of a hit. Wasteful and unreliable.
- **Larger runners:** these fix the core count, not the CPU model. They
  also require a Team or Enterprise organization plan, which a personal
  repository does not have.

#### 2b. Move timing benchmarks to `ubuntu-24.04-arm` — feasible, promising ✅

- All 8 ARM jobs in the probe ran on the same CPU (Neoverse-N2, D4pds_v6).
- Cross-runner wall-time CoV was 0.2–3 %, against 20–38 % on x86 and
  2–6 % even within a single x86 CPU model.
- These runners are free for public repositories and expose hardware
  counters (see 1b).
- **Risk:** GitHub may add another ARM generation (for example Cobalt 200)
  later. Keep the `cpu` tag on the measurements so that would be visible.
- This option is "pinning" without managing infrastructure. Sample more
  jobs before relying on the pool staying homogeneous.

#### 2c. Self-hosted or third-party dedicated runners — feasible, but costs money or maintenance ⚠️

- **Self-hosted runner on a dedicated or bare-metal machine:** best for
  wall time and has full PMU access.
  - Needs upkeep.
  - In a public repository it must never run fork PR code. Restrict it to
    `push` on `master`, or use ephemeral, isolated runners.
- **Managed services:**
  - CodSpeed macro runners (bare-metal, with a free monthly quota) and its
    Valgrind-based CPU simulation
  - Bencher bare-metal runners
  - Runner providers that let you choose a fixed instance type
    (RunsOn, Ubicloud, WarpBuild); a bare-metal AWS type also gives a PMU
- Each of these adds an external dependency.

### 3. Measure relative to a reference on the same runner

#### 3a. Paired or interleaved A/B on one runner — feasible ✅

- **Noise:** the A/A test shows same-runner relative timing noise of about
  0.5–0.75 pp stdev (max 2.2 %), against 20–38 % across runners. Within a
  runner, every CPU model gives the same answer.
- **Variant with git-perf:** build the merge-base (or `HEAD~1`) and `HEAD`,
  alternate their runs, and store the ratio as the measurement.
  [tango-bench](https://github.com/bazhenov/tango) does the same in-process
  for Rust benches.
- **Drawback of comparing to the parent:** each commit is compared only to
  its parent, so a slow drift spread over many commits goes unnoticed.

#### 3b. Normalize against a fixed baseline binary — feasible, recommended over 3a ✅

- **Method:** keep a pinned reference build, for example the last release,
  cached across runs. Each CI job times `HEAD` and the reference
  alternately on the same runner and stores
  `t(HEAD) / t(reference)` as the measurement.
- **Why it works:** this is a per-workload "CPU speed calibration".
  - Most of the CPU-model factor cancels, while the history stays one
    continuous series, so drift is still visible.
  - A generic calibration loop (such as SHA-256) would not do this as well,
    because the CPU speed ratio differs per workload. The
    [variance experiment](ci-variance-experiment/README.md), for example,
    found `matrix` far noisier across runners than `sort` or `sha256`.
- **Cost:**
  - The benchmark time doubles.
  - Bumping the reference needs an `epoch`.
  - Any CPU-model dependence left over (micro-architecture-sensitive code)
    still shows up and should be checked with a probe run.

### 4. Statistical mitigations within git-perf

These keep wall time but get more out of the split history.

- **Pool the history after normalizing each CPU model.** Divide every
  measurement by its CPU group's median (or z-score it within the group),
  then audit `HEAD` against the pooled, normalized tail. A new or rare CPU
  model then benefits from everyone's history once it has a few points.
  This would be a new option such as `normalize_by = ["cpu"]`.
- **Fall back to the closest model:** when `HEAD`'s CPU group is too short,
  audit against the group with the most history, scaled by the groups'
  median ratio.
- **Run more jobs per commit:** for example 3–4 timing jobs, so that one of
  them likely hits a CPU model with history.
- **Weaker but cheap:** keep per-CPU history and accept less coverage. This
  is the current state.

## Recommendation

1. **Gate on deterministic work metrics on x86, pooled across all CPU
   models.** Add Valgrind instruction counts, simulated LL misses, syscall
   counts and minor page faults for the `add` and `report` workloads, with
   `separate_by = ["os"]` (plus the git version) and tight thresholds
   (e.g. `min_relative_deviation = 1–2 %`). Every CI run then adds to one
   history, and the "no history on this CPU" gap disappears. Keep in mind
   that these metrics do not see waiting time.
2. **Keep a wall-time signal, but make it comparable.** Do one or both:
   - (a) move the timing benchmarks to `ubuntu-24.04-arm`, a single CPU
     model so far with 0.2–3 % cross-runner CoV, which also allows
     `perf stat` counters at native speed;
   - (b) time `HEAD` alternately against a cached reference build on the
     same runner, and audit the ratio.
3. **Keep the `cpu` tag on everything** so changes in the runner pool stay
   visible, and re-run the probe workflow whenever the pool looks
   different.

## Reproducing

```bash
# Locally (needs valgrind, strace; perf optional)
cargo build --release -p git-perf
scripts/stability-probe/probe.sh target/release/git-perf out.csv local
python3 scripts/stability-probe/summarize.py out.csv
```

On GitHub, trigger **Actions → Benchmark Stability Probe → Run workflow**. The
`summarize` job prints the tables and the raw CSV.
