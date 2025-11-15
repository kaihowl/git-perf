# Change Point Detection - Visual Guide

A visual explanation of change point detection for git-perf performance measurement tracking.

---

## What Problem Does This Solve?

### Current State: Z-Score Testing

```
Time →
Commit: A    B    C    D    E    F    G    H    I    J    [HEAD]
Value:  10   10   11   10   15   15   14   15   16   15    15

                                  ┌─── What caused this?
                                  │
Audit compares HEAD vs history: ──┘
  HEAD: 15ms
  Tail avg: 11.8ms
  Z-score: 2.5 → REGRESSION DETECTED ⚠️

Question: "Is HEAD significantly different?"
Answer: YES

BUT... We don't know WHEN the change happened!
```

### Proposed: Change Point Detection

```
Time →
Commit: A    B    C    D    E    F    G    H    I    J    [HEAD]
Value:  10   10   11   10   15   15   14   15   16   15    15
                          │
        Regime 1          │       Regime 2
        (μ=10.25)         │       (μ=15.0)
                          │
                  Change Point Detected! ↑ +46% at commit E

Question: "WHERE in history did performance change?"
Answer: Commit E caused a +46% regression

Now we can: git show E
           git blame <files changed in E>
           Understand root cause!
```

---

## Visual Algorithm Comparison

### PELT (Recommended)

```
Dynamic Programming with Pruning

Step 1: Try all possible segmentations
   A B C D E F G H I J
   └─────┘ └─────────┘  Cost = 5.2 + penalty(1 change point)
   └───┘ └─┘ └───────┘  Cost = 3.1 + penalty(2 change points)
   └─────────────────┘  Cost = 15.8 + penalty(0 change points)

Step 2: Prune impossible solutions
   ✅ Keep: Low cost segmentations
   ❌ Prune: Can never be optimal

Step 3: Find minimum cost
   Optimal: A-D | E-J (1 change point at E)

Time: O(n) with pruning
Accuracy: EXACT (mathematically optimal)
```

### E-Divisive (Netflix Approach)

```
Hierarchical Divisive with Energy Statistics

Step 1: Test for ANY change point
   A B C D E F G H I J
   Is there a split that maximizes divergence?
   → YES at E (energy statistic = 12.5)

Step 2: Recursively split each segment
   Segment 1 (A-D): Any change? → NO
   Segment 2 (E-J): Any change? → NO

Result: 1 change point at E

Time: O(n²) to O(n³)
Accuracy: Statistical (significance testing)
Advantage: Non-parametric (no distribution assumptions)
```

### Binary Segmentation (Simple)

```
Greedy Approach

Iteration 1: Find best single split
   A B C D E F G H I J
   Try splits at: B, C, D, E, F, G, H, I
   Best split: E (cost reduction = 10.6)
   Split! → [A-D] | [E-J]

Iteration 2: Find next best split in either segment
   [A-D]: Best split cost reduction = 0.3 (below threshold)
   [E-J]: Best split cost reduction = 0.5 (below threshold)
   STOP

Result: 1 change point at E

Time: O(n²)
Accuracy: Approximate (locally optimal)
```

---

## Real-World Example Visualization

### Scenario: Build Time Performance Over 30 Commits

```
Build Time (ms)
   25 │                                        █ █ █
      │                                      █ █ █ █
   20 │                                    █ █ █ █ █ █
      │                              █ █ █ █ █ █ █ █ █
   15 │                            █ █ █ █ █ █ █ █ █ █
      │      ▓ ▓ ▓               █ █ █ █ █ █ █ █ █ █
   10 │    ▓ ▓ ▓ ▓ ▓           █ █ █ █ █ █ █ █ █ █ █
      │  ▓ ▓ ▓ ▓ ▓ ▓         █ █ █ █ █ █ █ █ █ █ █
    5 │▓ ▓ ▓ ▓ ▓ ▓ ▓       █ █ █ █ █ █ █ █ █ █ █
      │▓ ▓ ▓ ▓ ▓ ▓ ▓     █ █ █ █ █ █ █ █ █ █ █
    0 └┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴
      A B C D E F G H I J K L M N O P Q R S T U V W X Y Z 1 2 3 4

      ▓ = Regime 1 (μ=8.2ms, σ=0.5)
      Gap at H-I: 🔴 Change Point #1 (+22% regression)

      █ = Regime 2 (μ=10.0ms, σ=0.3)
      Gap at N-O: 🔴 Change Point #2 (+100% regression)

      █ = Regime 3 (μ=20.0ms, σ=0.7)

PELT Output:
  Change Point 1: Commit I (+22%, 99% confidence)
  Change Point 2: Commit O (+100%, 99% confidence)

Root Cause Investigation:
  git show I  → Added integration tests
  git show O  → Switched to debug build by mistake
```

---

## Data Flow Diagram

```
┌───────────────────────────────────────────────────────────────┐
│ User: git perf audit -m build_time --detect-changes          │
└────────────────────────────┬──────────────────────────────────┘
                             │
                             ▼
        ┌────────────────────────────────────────────┐
        │ audit_with_commits()                       │
        │ • max_count = 40 (configurable)            │
        │ • measurement_name = "build_time"          │
        │ • selectors = []                           │
        └──────────────────┬─────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────┐
        │ measurement_retrieval::summarize()         │
        │ • Walk commits HEAD backwards              │
        │ • Filter by measurement name               │
        │ • Aggregate by reduction function (Mean)   │
        │ • Stop at epoch boundary                   │
        └──────────────────┬─────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────┐
        │ Collect time series data                   │
        │                                            │
        │ measurements: [15.0, 15.2, 15.1, 10.3, …] │
        │ commits: [HEAD, HEAD~1, HEAD~2, HEAD~3, …]│
        │                                            │
        │ (Newest first, will reverse for analysis) │
        └──────────────────┬─────────────────────────┘
                           │
              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼
┌──────────────────────┐    ┌───────────────────────────┐
│ Z-Score Test         │    │ Change Point Detection    │
│ (existing)           │    │ (NEW)                     │
│                      │    │                           │
│ Input:               │    │ Input:                    │
│  • head = [15.0]     │    │  • measurements (Vec<f64>)│
│  • tail = [15.2, …]  │    │  • commits (Vec<SHA>)     │
│                      │    │  • config (penalty, etc.) │
│ Process:             │    │                           │
│  • Calc stats        │    │ Process:                  │
│  • Compute z-score   │    │  • Reverse (oldest first) │
│  • Compare threshold │    │  • Run PELT algorithm     │
│                      │    │  • Calculate magnitude    │
│ Output:              │    │  • Compute confidence     │
│  • ✅ or ⚠️         │    │  • Filter by thresholds   │
│  • z-score value     │    │                           │
│  • Stats summary     │    │ Output:                   │
│                      │    │  • Vec<ChangePoint>       │
└──────────────────────┘    └───────────────────────────┘
              │                         │
              └────────────┬────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────┐
        │ Format output                              │
        │                                            │
        │ ✅ 'build_time'                           │
        │ z-score (stddev): ↑ 2.34                  │
        │ Head: μ: 15.12 ms …                       │
        │ Tail: μ: 10.45 ms …                       │
        │                                            │
        │ Change Points Detected (PELT, n=25):      │
        │   ↑ Commit a1b2c3d: +44.7% (99%)          │
        │   ↓ Commit d4e5f6g: -3.2% (87%)           │
        └────────────────────────────────────────────┘
```

---

## PELT Algorithm Walkthrough

### Example: 10 measurements with 1 obvious change point

```
Data: [10, 10, 11, 10, 10, 20, 20, 19, 20, 20]
       └──── Regime 1 ─────┘ └──── Regime 2 ────┘
                            ↑
                     Change point at index 5

Penalty (β) = 5.0 (moderate sensitivity)
```

### Step-by-Step Execution

```
Initialization:
  F[0] = -β = -5.0
  R = {0}  (active set contains only start)
  cp[0] = 0

Iteration t=1 (measurement: 10):
  For τ=0: cost = F[0] + C(0:1) + β = -5.0 + 0.0 + 5.0 = 0.0
  F[1] = 0.0, cp[1] = 0
  R = {0, 1}

Iteration t=2 (measurement: 10):
  For τ=0: cost = F[0] + C(0:2) + β = -5.0 + 0.0 + 5.0 = 0.0
  For τ=1: cost = F[1] + C(1:2) + β = 0.0 + 0.0 + 5.0 = 5.0
  F[2] = 0.0, cp[2] = 0 (no split yet cheaper)
  R = {0, 2}  (τ=1 pruned)

[… iterations 3-5 similar, no split cheaper than no-split …]

Iteration t=6 (measurement: 20 - first point in new regime):
  For τ=0: cost = F[0] + C(0:6) + β = -5.0 + 166.7 + 5.0 = 166.7
            (high cost: mixing two regimes)
  For τ=5: cost = F[5] + C(5:6) + β = 0.0 + 0.0 + 5.0 = 5.0
            (low cost: split at regime boundary!)
  F[6] = 5.0, cp[6] = 5 ← SPLIT PREFERRED
  R = {5, 6}

Iteration t=7 (measurement: 20):
  For τ=5: cost = F[5] + C(5:7) + β = 0.0 + 0.5 + 5.0 = 5.5
  For τ=6: cost = F[6] + C(6:7) + β = 5.0 + 0.0 + 5.0 = 10.0
  F[7] = 5.5, cp[7] = 5
  R = {5, 7}

[… iterations 8-10 similar …]

Final: F[10] = 6.7, change point at index 5

Backtracking:
  cp[10] = 5 → cp[5] = 0 → DONE
  Change points: [5]

Result: 1 change point at index 5 (Commit F)
```

---

## Configuration Sensitivity Analysis

### Effect of Penalty Parameter

```
Data: [10, 10, 10, 11, 15, 15, 15, 16, 20, 20, 20, 21]
      └─ Stable ─┘ └─ Small ─┘ └─ Large change ─┘

Penalty = 10.0 (Conservative):
  Only detects large, obvious changes
  Result: 1 change point at index 8

  [10, 10, 10, 11, 15, 15, 15, 16] [20, 20, 20, 21]
                                    ↑
                            Only this detected

Penalty = 3.0 (Balanced - DEFAULT):
  Detects moderate changes
  Result: 2 change points at indices 4, 8

  [10, 10, 10, 11] [15, 15, 15, 16] [20, 20, 20, 21]
                   ↑                 ↑
                   Both detected

Penalty = 1.0 (Aggressive):
  Detects even small fluctuations (risk of false positives)
  Result: 3+ change points

  [10, 10] [10, 11] [15, 15, 15, 16] [20, 20, 20, 21]
           ↑        ↑                 ↑
           Noise    Real              Real
```

**Recommendation**: Start with penalty = 3.0, tune based on false positive rate.

---

## Confidence Calculation

### How Confidence is Computed

```
Confidence = f(magnitude, segment_stability, sample_size)

Example:

Change Point: Index 5, magnitude +100% (10 → 20)
  Before segment: [10.0, 10.1, 9.9, 10.0, 10.2] (n=5)
  After segment:  [20.0, 19.8, 20.1, 20.2, 19.9] (n=5)

Step 1: Calculate segment statistics
  Before: μ₁=10.04, σ₁=0.12
  After:  μ₂=19.98, σ₂=0.16

Step 2: Compute t-statistic (Welch's t-test)
  t = (μ₂ - μ₁) / √(σ₁²/n₁ + σ₂²/n₂)
  t = (19.98 - 10.04) / √(0.12²/5 + 0.16²/5)
  t = 9.94 / 0.074 = 134.3

Step 3: Convert to p-value and confidence
  p-value ≈ 0.0001 → Confidence = 99.99%

Interpretation: Extremely confident this is a real change, not noise.
```

---

## Output Format Mockups

### Terminal Output (Colorized in Practice)

```
$ git perf audit -m build_time --detect-changes

✅ 'build_time'
z-score (stddev): ↑ 2.34
Head: μ: 15.12 ms σ: 0.23 MAD: 0.15 n: 1
Tail: μ: 10.45 ms σ: 0.52 MAD: 0.38 n: 25
 [+39.2% – +51.8%] ▃▃▃▄▄▅▅██

⚠️  Performance regression detected at HEAD

╭─────────────────────────────────────────────────────────╮
│ Change Points Detected (PELT algorithm, n=25)           │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ↑ Commit a1b2c3d (5 commits ago)                      │
│     • Magnitude: +44.7% (10.0ms → 14.5ms)              │
│     • Confidence: 99%                                   │
│     • Date: Jan 25, 2024                                │
│     • Message: "Add comprehensive integration tests"   │
│                                                         │
│  ↓ Commit d4e5f6g (12 commits ago)                     │
│     • Magnitude: -3.2% (15.0ms → 14.5ms)               │
│     • Confidence: 87%                                   │
│     • Date: Jan 18, 2024                                │
│     • Message: "Optimize database queries"             │
│                                                         │
│  ℹ  Commit x7y8z9w (20 commits ago)                    │
│     • Magnitude: +2.1% (within noise margin)           │
│     • Confidence: 73% (below threshold)                │
│     • Status: Not significant                          │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ Summary: 1 regression, 1 improvement                    │
│ Net change: +40.8% since 25 commits ago                │
│                                                         │
│ Recommendation: Investigate commit a1b2c3d              │
│   git show a1b2c3d                                      │
│   git diff a1b2c3d^..a1b2c3d                           │
╰─────────────────────────────────────────────────────────╯
```

### HTML Report (Plotly Graph)

```html
<!DOCTYPE html>
<html>
<head>
    <title>Build Time Performance Analysis</title>
    <script src="plotly-latest.min.js"></script>
</head>
<body>
    <div id="chart"></div>
    <script>
    var trace = {
        x: commits,
        y: measurements,
        type: 'scatter',
        mode: 'lines+markers',
        name: 'build_time',
        line: { color: 'blue' }
    };

    // Change point vertical lines
    var shapes = [
        {
            type: 'line',
            x0: 5, x1: 5,
            y0: 0, y1: 1,
            yref: 'paper',
            line: { color: 'red', width: 3, dash: 'dash' }
        },
        {
            type: 'line',
            x0: 12, x1: 12,
            y0: 0, y1: 1,
            yref: 'paper',
            line: { color: 'green', width: 3, dash: 'dash' }
        }
    ];

    // Change point annotations
    var annotations = [
        {
            x: 5,
            y: measurements[5],
            text: '🔴 Regression<br>+44.7%<br>a1b2c3d',
            showarrow: true,
            arrowhead: 2,
            bgcolor: 'rgba(255,0,0,0.8)',
            font: { color: 'white' }
        },
        {
            x: 12,
            y: measurements[12],
            text: '🟢 Improvement<br>-3.2%<br>d4e5f6g',
            showarrow: true,
            arrowhead: 2,
            bgcolor: 'rgba(0,255,0,0.8)',
            font: { color: 'white' }
        }
    ];

    var layout = {
        title: 'Build Time - Change Point Analysis',
        xaxis: { title: 'Commit' },
        yaxis: { title: 'Time (ms)' },
        shapes: shapes,
        annotations: annotations
    };

    Plotly.newPlot('chart', [trace], layout);
    </script>
</body>
</html>
```

**Visual Result**:
```
    ms
    20 │                  ●
       │                 ● ●
    15 │    ┊           ●   ●
       │    ┊    ●     ●     ●
    10 │ ●  ┊   ● ●   ●       ●
       │● ● ┊  ●   ● ●
     5 │    ┊
       └────┴─────────────────────→ commits
            ↑
      Change point marker
      (red dashed line)
      Hover shows: "+44.7%, commit a1b2c3d"
```

---

## CSV Export Enhancement

### Current CSV Format
```csv
commit,timestamp,measurement,value,unit
abc123,2024-01-25T10:00:00Z,build_time,15.2,ms
def456,2024-01-24T09:30:00Z,build_time,10.1,ms
```

### Enhanced with Change Point Data
```csv
commit,timestamp,measurement,value,unit,segment_id,change_point,magnitude_pct,confidence
abc123,2024-01-25T10:00:00Z,build_time,15.2,ms,2,false,,,
def456,2024-01-24T09:30:00Z,build_time,15.0,ms,2,true,44.7,0.99
ghi789,2024-01-23T08:00:00Z,build_time,10.3,ms,1,false,,,
jkl012,2024-01-22T07:00:00Z,build_time,10.0,ms,1,false,,,
```

**New Columns**:
- `segment_id`: Which regime (1, 2, 3, etc.)
- `change_point`: Is this commit a detected change point?
- `magnitude_pct`: Percentage change (if change point)
- `confidence`: Statistical confidence (if change point)

---

## Implementation Timeline Gantt Chart

```
Week 1-2: MVP (PELT)
├─ Day 1-3:   PELT algorithm + tests     ████████
├─ Day 4-5:   Integration into audit     ███
├─ Day 6-7:   Configuration support      ███
└─ Day 8-10:  Testing + documentation    █████

Week 3-4: Multiple Algorithms
├─ Week 3:    E-Divisive implementation  ████████
└─ Week 4:    Binary Seg + benchmarks    ████████

Week 5-8: Production Ready
├─ Week 5-6:  HTML/CSV integration       ████████████
├─ Week 7:    CI/CD examples             ████████
└─ Week 8:    Optimization + polish      ████████
```

---

## Decision Tree: Which Algorithm to Use?

```
                    START
                      │
                      ▼
            ┌─────────────────────┐
            │ How many data points│
            │ do you have?        │
            └──────┬──────────────┘
                   │
         ┌─────────┴─────────┐
         │                   │
       < 50              50-1000
         │                   │
         ▼                   ▼
    ┌─────────┐        ┌──────────┐
    │ Is data │        │ Need     │
    │ noisy?  │        │ speed?   │
    └────┬────┘        └────┬─────┘
         │                  │
    ┌────┴────┐        ┌────┴────┐
    │         │        │         │
   YES       NO       YES       NO
    │         │        │         │
    ▼         ▼        ▼         ▼
┌────────┐ ┌──────┐ ┌──────┐ ┌──────────┐
│E-Div.  │ │BinSeg│ │PELT  │ │E-Div or  │
│        │ │      │ │      │ │PELT      │
│Robust  │ │Simple│ │Fast  │ │Your      │
│        │ │      │ │      │ │choice    │
└────────┘ └──────┘ └──────┘ └──────────┘

Recommended default: PELT
```

---

## Summary: Benefits at a Glance

| Aspect | Before (Z-Score Only) | After (+ Change Points) |
|--------|-----------------------|-------------------------|
| **Question Answered** | "Is HEAD different?" | "When did it change?" |
| **Output** | Pass/Fail, z-score | List of change commits |
| **Root Cause** | Manual bisect needed | Direct commit identification |
| **Historical View** | Only HEAD vs tail | Full timeline analysis |
| **Multiple Changes** | Only detects latest | Detects all changes |
| **Debugging** | "Something changed" | "Commit X caused +45% regression" |
| **Confidence** | Statistical | Statistical + magnitude |

---

## Next Steps

1. ✅ Read this visual guide
2. ✅ Review CHANGE_POINT_DETECTION_PROPOSAL.md (detailed spec)
3. ✅ Review QUICK_START_CPD.md (implementation checklist)
4. ⏭️  Create feature branch: `feature/change-point-detection`
5. ⏭️  Begin Phase 1: Implement PELT algorithm
6. ⏭️  Weekly check-ins and progress reviews

---

**Document Version**: 1.0
**Created**: November 12, 2025
**Purpose**: Visual aid for understanding change point detection implementation
