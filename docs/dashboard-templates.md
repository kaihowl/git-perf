# Dashboard Templates - Multi-Section Reports

**Version:** Phase 2b Implementation
**Status:** Implemented

## Overview

Dashboard templates allow you to create comprehensive performance reports with multiple independent sections in a single HTML file. Each section can have its own filtering, aggregation, and visualization settings.

## Use Cases

- **Aggregated vs Raw**: Show both median-aggregated trends and raw measurement variability
- **Comparison Views**: Display different measurements (CPU vs Memory) side-by-side
- **Multi-Metric Dashboards**: Combine test durations, benchmarks, and build times in one report
- **Split Comparisons**: Show the same measurement split by different metadata keys (OS vs architecture)

## Quick Start

### Basic Example

```html
<!DOCTYPE html>
<html>
<head>
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    <h1>{{TITLE}}</h1>

    <h2>Test Performance</h2>
    {{SECTION[tests]
        measurement-filter: ^test::
        aggregate-by: median
    }}

    <h2>Benchmark Performance</h2>
    {{SECTION[benchmarks]
        measurement-filter: ^bench::
        aggregate-by: median
    }}
</body>
</html>
```

### Usage

```bash
# Generate dashboard report
git perf report --template .git-perf/templates/dashboard.html --output report.html
```

**Important:** When using multi-section templates, CLI arguments like `--filter`, `--measurement`, `--aggregate-by`, `--separate-by`, and `-n` are **ignored**. All configuration comes from the template itself.

## Section Syntax

### Basic Structure

```
{{SECTION[section-id]
    parameter: value
    parameter2: value2
}}
```

- **section-id**: Unique identifier for this section (e.g., `test-overview`, `bench-median`)
- **parameters**: Optional configuration parameters (one per line)

### Available Parameters

#### measurement-filter

Regex pattern for selecting measurements. Uses the same syntax as `--filter` CLI flag.

```
measurement-filter: ^test::
measurement-filter: ^bench::.*::mean$
measurement-filter: memory_.*
```

**Examples:**
- `^test::` - All measurements starting with "test::"
- `^bench::.*::mean$` - Benchmark mean values
- `memory_peak` - Exact match for "memory_peak"

#### key-value-filter

Filter measurements by metadata key-value pairs. Comma-separated list.

```
key-value-filter: os=linux,arch=x64
key-value-filter: test_type=integration,env=ci
```

Only measurements with **all** specified key-value pairs will be included.

#### separate-by

Split measurements into separate traces based on metadata keys. Comma-separated list.

```
separate-by: os,arch
separate-by: platform
```

Creates separate plot lines for each unique combination of the specified keys.

#### aggregate-by

Aggregation function to apply. Options:
- `none` - Raw data (no aggregation)
- `min` - Minimum value
- `max` - Maximum value
- `median` - Median value (default for many use cases)
- `mean` - Average value

```
aggregate-by: median
aggregate-by: none
aggregate-by: max
```

#### depth

Number of commits to include (overrides global `-n` flag).

```
depth: 50
depth: 100
```

#### title

Section-specific title for the chart (currently supported in parsing, display TBD).

```
title: Test Execution Time
title: Peak Memory Usage
```

## Complete Examples

### Example 1: Test Performance Dashboard

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    <style>
        body { font-family: sans-serif; max-width: 1200px; margin: 0 auto; padding: 20px; }
        .section { margin: 30px 0; padding: 20px; background: #f9f9f9; }
        .section h2 { margin-top: 0; border-bottom: 2px solid #007bff; }
        {{CUSTOM_CSS}}
    </style>
</head>
<body>
    <h1>{{TITLE}}</h1>
    <p>Generated: {{TIMESTAMP}} | Commits: {{COMMIT_RANGE}} ({{DEPTH}} total)</p>

    <div class="section">
        <h2>Test Performance Overview (Median, Last 50 Commits)</h2>
        {{SECTION[test-median]
            measurement-filter: ^test::
            aggregate-by: median
            depth: 50
        }}
    </div>

    <div class="section">
        <h2>Integration Tests - Raw Data</h2>
        {{SECTION[integration-raw]
            measurement-filter: ^test::integration::
            aggregate-by: none
            key-value-filter: test_type=integration
        }}
    </div>

    <div class="section">
        <h2>Unit Tests - Raw Data</h2>
        {{SECTION[unit-raw]
            measurement-filter: ^test::unit::
            aggregate-by: none
            key-value-filter: test_type=unit
        }}
    </div>
</body>
</html>
```

### Example 2: Cross-Platform Comparison

```html
<div class="section">
    <h2>Build Times by Platform</h2>
    {{SECTION[build-platform]
        measurement-filter: ^build_time$
        separate-by: os,arch
        aggregate-by: median
    }}
</div>

<div class="section">
    <h2>Test Performance by OS</h2>
    {{SECTION[test-by-os]
        measurement-filter: ^test::
        separate-by: os
        aggregate-by: median
    }}
</div>
```

### Example 3: Memory Usage Analysis

```html
<div style="display: grid; grid-template-columns: 1fr 1fr; gap: 20px;">
    <div>
        <h3>Peak Memory (Max)</h3>
        {{SECTION[memory-max]
            measurement-filter: memory_peak
            aggregate-by: max
        }}
    </div>
    <div>
        <h3>Average Memory</h3>
        {{SECTION[memory-avg]
            measurement-filter: memory_peak
            aggregate-by: mean
        }}
    </div>
</div>
```

### Example 4: Benchmark Comparison

```html
<div class="section">
    <h2>Benchmark Performance - Mean Values</h2>
    {{SECTION[bench-mean]
        measurement-filter: ^bench::.*::mean$
        aggregate-by: median
    }}
</div>

<div class="section">
    <h2>Benchmark Performance - Median Values</h2>
    {{SECTION[bench-median-values]
        measurement-filter: ^bench::.*::median$
        aggregate-by: median
    }}
</div>
```

## Template Placeholders

In addition to `{{SECTION[...]}}`, the following global placeholders are available:

- `{{TITLE}}` - Report title (from `--title` flag or config)
- `{{PLOTLY_HEAD}}` - Plotly.js library script tags (required in `<head>`)
- `{{PLOTLY_BODY}}` - Not used in multi-section templates (sections handle plot rendering)
- `{{CUSTOM_CSS}}` - Custom CSS from `--custom-css` flag or config
- `{{TIMESTAMP}}` - Report generation timestamp
- `{{COMMIT_RANGE}}` - Commit range (e.g., "abc123..def456")
- `{{DEPTH}}` - Number of commits analyzed
- `{{AUDIT_SECTION}}` - Audit results (future enhancement)

## Best Practices

### 1. Organize Sections Logically

Group related measurements together:

```html
<!-- High-level overview first -->
<section>
    <h2>Test Suite Performance (Aggregated)</h2>
    {{SECTION[tests-overview]
        measurement-filter: ^test::
        aggregate-by: median
    }}
</section>

<!-- Detailed breakdowns second -->
<section>
    <h2>Integration Tests (Raw Data)</h2>
    {{SECTION[integration-detail]
        measurement-filter: ^test::integration::
        aggregate-by: none
    }}
</section>
```

### 2. Use Meaningful Section IDs

Choose descriptive IDs that reflect the content:

```
✅ Good: test-median, bench-by-platform, memory-max
❌ Bad: section1, s2, data
```

### 3. Combine Aggregations Strategically

- **Median**: Best for typical performance trends
- **Max**: Useful for peak resource usage
- **Min**: Good for best-case scenarios
- **Mean**: When you want the average including outliers
- **None**: Show raw data to visualize variability

### 4. Limit Sections Per Page

Too many sections can make reports slow to load. Consider:
- **3-5 sections**: Optimal for most dashboards
- **10+ sections**: May impact load time
- Split into multiple templates if needed

### 5. Use depth to Control Data Volume

```
{{SECTION[recent-tests]
    measurement-filter: ^test::
    aggregate-by: median
    depth: 20
}}
```

Shorter histories load faster and are easier to read for trend analysis.

## Example Templates

The git-perf repository includes example templates:

- `.git-perf/templates/simple-dashboard.html` - Basic multi-section example
- `.git-perf/templates/dashboard-example.html` - Comprehensive dashboard with styling

Copy and customize these for your needs:

```bash
cp .git-perf/templates/simple-dashboard.html .git-perf/my-dashboard.html
# Edit .git-perf/my-dashboard.html
git perf report --template .git-perf/my-dashboard.html --output report.html
```

## Troubleshooting

### No Data in Section

If a section shows no data:

1. Check the `measurement-filter` pattern matches your measurement names
2. Verify `key-value-filter` constraints are not too restrictive
3. Confirm measurements exist with: `git perf list-commits`

### Duplicate Section ID Error

```
Error: Duplicate section ID found: test-section
```

Each section must have a unique ID within the template.

### Section Not Rendering

If `{{SECTION[...]}}` appears as text in the output:

1. Check the syntax - ensure no typos
2. Verify closing `}}` is present
3. Ensure template is actually being used (check `--template` flag)

### CLI Arguments Ignored

When using multi-section templates, CLI arguments for filtering and aggregation are intentionally ignored. All configuration must be in the template.

## Advanced Usage

### Conditional Sections

You can use standard HTML/CSS to show/hide sections:

```html
<div class="section" id="benchmarks">
    <h2>Benchmarks (Only on CI)</h2>
    {{SECTION[bench-ci]
        measurement-filter: ^bench::
        key-value-filter: env=ci
        aggregate-by: median
    }}
</div>
```

If no measurements match, the section will still render but the chart may be empty.

### Multiple Dashboards

Create different templates for different purposes:

```bash
# Test-focused dashboard
git perf report --template .git-perf/test-dashboard.html -o reports/tests.html

# Benchmark-focused dashboard
git perf report --template .git-perf/bench-dashboard.html -o reports/benchmarks.html

# Overview dashboard
git perf report --template .git-perf/overview.html -o reports/overview.html
```

## Migration from Simple Templates

If you have an existing simple template without sections:

**Before (simple template):**
```html
<body>
    <h1>{{TITLE}}</h1>
    {{PLOTLY_BODY}}
</body>
```

**After (multi-section):**
```html
<body>
    <h1>{{TITLE}}</h1>
    {{SECTION[main]
        measurement-filter: .*
        aggregate-by: median
    }}
</body>
```

The CLI arguments you previously used can now be moved into the section configuration.

## See Also

- [Report Templating Guide](./report-templating.md) - Basic template customization
- [Importing Measurements](./importing-measurements.md) - How to add measurements
- [Integration Tutorial](./INTEGRATION_TUTORIAL.md) - CI/CD integration
- [GitHub Pages Integration Plan](./plans/github-pages-integration-and-templating.md) - Multi-purpose site deployment
