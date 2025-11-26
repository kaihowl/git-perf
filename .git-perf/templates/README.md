# Git-Perf Report Templates

This directory contains example report templates for git-perf's multi-section dashboard feature.

## Available Templates

### simple-dashboard.html

A basic multi-section template showing:
- Test performance (aggregated)
- Benchmark performance (aggregated)
- Raw test data (last 20 commits)

**Usage:**
```bash
git perf report --template .git-perf/templates/simple-dashboard.html --output report.html
```

### dashboard-example.html

A comprehensive dashboard template with:
- Test suite performance overview
- Benchmark results
- Build times by platform
- Memory usage comparison (side-by-side max and average)
- Raw integration test data

**Usage:**
```bash
git perf report --template .git-perf/templates/dashboard-example.html --output dashboard.html
```

## Creating Your Own Template

1. Copy an existing template:
   ```bash
   cp .git-perf/templates/simple-dashboard.html .git-perf/my-template.html
   ```

2. Edit the template to customize sections

3. Generate a report:
   ```bash
   git perf report --template .git-perf/my-template.html --output my-report.html
   ```

## Documentation

For complete documentation on the template syntax and available parameters, see:
- [Dashboard Templates Guide](../../docs/dashboard-templates.md)
- [Plan Document](../../docs/plans/github-pages-integration-and-templating.md)

## Section Syntax

```html
{{SECTION[section-id]
    measurement-filter: ^test::
    aggregate-by: median
    depth: 50
}}
```

### Available Parameters

- `measurement-filter` - Regex to match measurement names
- `key-value-filter` - Filter by metadata (e.g., `os=linux,arch=x64`)
- `separate-by` - Split by metadata keys (e.g., `os,arch`)
- `aggregate-by` - `none`, `min`, `max`, `median`, `mean`
- `depth` - Number of commits (overrides `-n` flag)
- `title` - Section title (for future use)

## Tips

1. **Start simple** - Begin with 2-3 sections and add more as needed
2. **Use meaningful IDs** - Name sections like `test-median` or `bench-by-platform`
3. **Combine aggregations** - Mix raw and aggregated views for different insights
4. **Control depth** - Use shorter histories (depth: 20-50) for faster loading

## Examples

### Basic Test Dashboard

```html
<h2>Test Performance</h2>
{{SECTION[tests]
    measurement-filter: ^test::
    aggregate-by: median
}}
```

### Platform Comparison

```html
<h2>Build Times by OS</h2>
{{SECTION[builds]
    measurement-filter: ^build_time$
    separate-by: os,arch
    aggregate-by: median
}}
```

### Memory Analysis

```html
<div style="display: grid; grid-template-columns: 1fr 1fr; gap: 20px;">
    <div>
        <h3>Peak Memory</h3>
        {{SECTION[mem-max]
            measurement-filter: memory_peak
            aggregate-by: max
        }}
    </div>
    <div>
        <h3>Average Memory</h3>
        {{SECTION[mem-avg]
            measurement-filter: memory_peak
            aggregate-by: mean
        }}
    </div>
</div>
```
