# Dashboard Templates

Multi-section templates allow creating comprehensive performance reports with multiple independent sections in a single HTML file.

## Quick Start

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

    <h2>Benchmarks</h2>
    {{SECTION[benchmarks]
        measurement-filter: ^bench::
        aggregate-by: median
    }}
</body>
</html>
```

Generate report:
```bash
git perf report --template dashboard.html --output report.html
```

**Note:** CLI arguments (`--filter`, `--aggregate-by`, etc.) are ignored when using multi-section templates.

## Section Syntax

```
{{SECTION[section-id]
    parameter: value
}}
```

### Available Parameters

- **measurement-filter**: Regex pattern (e.g., `^test::`, `memory_.*`)
- **key-value-filter**: Metadata filters (e.g., `os=linux,arch=x64`)
- **separate-by**: Split by metadata keys (e.g., `os,arch`)
- **aggregate-by**: `none`, `min`, `max`, `median`, `mean`
- **depth**: Number of commits (overrides `-n` flag)
- **show-epochs**: Show epoch boundaries (`true`/`false`)
- **show-changes**: Show change points (`true`/`false`)

## Examples

### Cross-Platform Comparison
```html
<h2>Build Times by Platform</h2>
{{SECTION[build-platform]
    measurement-filter: ^build_time$
    separate-by: os,arch
    aggregate-by: median
}}
```

### Aggregated vs Raw Data
```html
<h2>Test Performance (Median)</h2>
{{SECTION[tests-median]
    measurement-filter: ^test::
    aggregate-by: median
}}

<h2>Test Performance (Raw)</h2>
{{SECTION[tests-raw]
    measurement-filter: ^test::
    aggregate-by: none
    depth: 20
}}
```

### Side-by-Side Comparison
```html
<div style="display: grid; grid-template-columns: 1fr 1fr; gap: 20px;">
    <div>
        <h3>Peak Memory</h3>
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

## Global Placeholders

- `{{TITLE}}` - Report title
- `{{PLOTLY_HEAD}}` - Plotly.js library (required in `<head>`)
- `{{CUSTOM_CSS}}` - Custom CSS
- `{{TIMESTAMP}}` - Generation timestamp
- `{{COMMIT_RANGE}}` - Commit range
- `{{DEPTH}}` - Number of commits

## Best Practices

1. **Use meaningful IDs**: `test-median`, `bench-by-platform` (not `section1`, `s2`)
2. **Limit sections**: 3-5 optimal, 10+ may impact load time
3. **Control depth**: Use shorter histories (`depth: 20-50`) for faster loading
4. **Organize logically**: Overview sections first, detailed breakdowns second
5. **Choose aggregation wisely**:
   - `median` - Typical trends
   - `max` - Peak resource usage
   - `min` - Best-case scenarios
   - `none` - Show variability

## Example Templates

See `.git-perf/templates/` directory:
- `performance-overview.html` - Professional dashboard (recommended)
- `simple-dashboard.html` - Basic example
- `dashboard-example.html` - Comprehensive example

Copy and customize:
```bash
cp .git-perf/templates/simple-dashboard.html .git-perf/my-dashboard.html
git perf report --template .git-perf/my-dashboard.html --output report.html
```

## Troubleshooting

**No data in section**: Check `measurement-filter` matches your measurements. Use `git perf list-commits` to verify.

**Duplicate ID error**: Each section must have a unique ID.

**Section appears as text**: Check syntax, ensure closing `}}`, verify `--template` flag.

## See Also

- [GitHub Pages Integration Plan](./plans/github-pages-integration-and-templating.md)
- [Integration Tutorial](./INTEGRATION_TUTORIAL.md)
