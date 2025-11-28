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

- `measurement-filter` - Regex to match measurement names (only matches measurements configured in `.gitperfconfig`)
- `key-value-filter` - Filter by metadata (e.g., `os=linux,arch=x64`)
- `separate-by` - Split by metadata keys (e.g., `os,arch`)
- `aggregate-by` - `none`, `min`, `max`, `median`, `mean`
- `depth` - Number of commits for this section (overrides the `-n` flag and `{{DEPTH}}` placeholder)
- `title` - Section title (for future use)

### Understanding depth vs -n Flag

When generating reports with multi-section templates:

- **`-n` flag** (command line): Sets the default depth for all sections AND the `{{DEPTH}}` placeholder
- **`depth:` parameter** (in section): Overrides the default for that specific section only

**Example:**
```bash
git perf report --template dashboard.html -n 100 --output report.html
```

With this template:
```html
{{DEPTH}}  <!-- Will show "100" -->

{{SECTION[recent]
    measurement-filter: ^test::
    depth: 20
}}  <!-- Shows last 20 commits -->

{{SECTION[historical]
    measurement-filter: ^test::
}}  <!-- Shows last 100 commits (uses -n default) -->
```

**Best Practice:** Use `-n` to set a reasonable default history, then use per-section `depth:` parameters to show shorter/longer histories where needed (e.g., raw data sections with `depth: 20` for faster loading).

## Tips

1. **Use configured measurements** - Only measurements defined in `.gitperfconfig` will appear in reports. Check your config file to see available measurements.
2. **Start simple** - Begin with 2-3 sections and add more as needed
3. **Use meaningful IDs** - Name sections like `test-median` or `bench-by-platform`
4. **Combine aggregations** - Mix raw and aggregated views for different insights
5. **Control depth** - Use shorter histories (depth: 20-50) for faster loading

## Examples

**Note:** These examples use measurements from the repository's `.gitperfconfig`. Adjust the `measurement-filter` patterns to match your own configured measurements.

### Basic Test Dashboard

```html
<h2>Test Performance</h2>
{{SECTION[tests]
    measurement-filter: ^(test-measure2)$
    aggregate-by: median
}}
```

### Benchmark Comparison

```html
<h2>Benchmark Performance</h2>
{{SECTION[benchmarks]
    measurement-filter: ^(report-benchmark|add-benchmark)$
    aggregate-by: median
}}
```

### Size Analysis

```html
<div style="display: grid; grid-template-columns: 1fr 1fr; gap: 20px;">
    <div>
        <h3>Binary Size</h3>
        {{SECTION[binary-size]
            measurement-filter: ^(release-binary-size)$
            aggregate-by: max
        }}
    </div>
    <div>
        <h3>Report Size</h3>
        {{SECTION[report-size]
            measurement-filter: ^(report-size|report-size-benchmark)$
            aggregate-by: mean
        }}
    </div>
</div>
```
