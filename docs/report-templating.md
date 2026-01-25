# Report Templating Guide

This guide explains how to customize git-perf HTML reports using templates.

## Table of Contents

- [Overview](#overview)
- [Simple Templates](#simple-templates)
- [Dashboard Templates](#dashboard-templates)
- [Template Placeholders](#template-placeholders)
- [Custom CSS](#custom-css)
- [Configuration](#configuration)
- [Examples](#examples)

## Overview

Git-perf supports two types of templates:

1. **Simple Templates**: Single-section reports with custom layout and styling
2. **Dashboard Templates**: Multi-section reports with different visualizations per section

Templates use a simple placeholder syntax:
- `{{TITLE}}` - Replaced with the report title
- `{{PLOTLY_HEAD}}` - Plotly.js library includes
- `{{PLOTLY_BODY}}` - The actual plot HTML
- `{{CUSTOM_CSS}}` - Custom CSS styles
- `{{TIMESTAMP}}` - Generation timestamp
- `{{COMMIT_RANGE}}` - Commit range covered (e.g., `abc1234..def5678`)
- `{{DEPTH}}` - Number of commits analyzed

## Simple Templates

Simple templates wrap a single Plotly visualization with custom HTML structure.

### Basic Example

Create `.git-perf/templates/basic-template.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    <style>
        body {
            font-family: system-ui, sans-serif;
            max-width: 1400px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        header {
            background: white;
            padding: 20px;
            margin-bottom: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1 {
            margin: 0;
            color: #333;
        }
        .metadata {
            margin-top: 10px;
            color: #666;
            font-size: 0.9em;
        }
        main {
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        {{CUSTOM_CSS}}
    </style>
</head>
<body>
    <header>
        <h1>{{TITLE}}</h1>
        <div class="metadata">
            <p>Generated: {{TIMESTAMP}}</p>
            <p>Commits: {{COMMIT_RANGE}} ({{DEPTH}} commits)</p>
        </div>
    </header>

    <main>
        {{PLOTLY_BODY}}
    </main>
</body>
</html>
```

### Using Simple Templates

**Command line:**
```bash
git perf report -n 50 -o report.html --template .git-perf/templates/basic-template.html
```

**GitHub Actions:**
```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    template: '.git-perf/templates/basic-template.html'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Adding Navigation

Include navigation links to your documentation:

```html
<header>
    <nav>
        <a href="/index.html">Home</a>
        <a href="/docs/">Documentation</a>
        <a href="/perf/">All Reports</a>
    </nav>
    <h1>{{TITLE}}</h1>
</header>
```

## Dashboard Templates

Dashboard templates create multi-section reports with independent visualizations. See [Dashboard Templates Guide](./dashboard-templates.md) for complete documentation.

### Quick Example

```html
<main>
    <!-- Test Performance Overview -->
    <section>
        <h2>Test Performance (Last 50 Commits)</h2>
        {{SECTION[test-overview]
            measurement-filter: ^test::
            aggregate-by: median
            depth: 50
        }}
    </section>

    <!-- Benchmark Results -->
    <section>
        <h2>Benchmark Results</h2>
        {{SECTION[benchmarks]
            measurement-filter: ^bench::.*::mean$
            aggregate-by: median
        }}
    </section>
</main>
```

Each `{{SECTION[...]}}` block:
- Has a unique ID (`test-overview`, `benchmarks`)
- Can filter measurements independently
- Can use different aggregation methods
- Can customize visualization settings

## Template Placeholders

### Available Placeholders

| Placeholder | Description | Example Value |
|-------------|-------------|---------------|
| `{{TITLE}}` | Report title | "Performance Measurements" |
| `{{PLOTLY_HEAD}}` | Plotly.js library includes | `<script src="...plotly.min.js"></script>` |
| `{{PLOTLY_BODY}}` | Plot HTML (div + script) | `<div id="plot">...</div><script>...</script>` |
| `{{CUSTOM_CSS}}` | Custom CSS content | `.my-class { color: red; }` |
| `{{TIMESTAMP}}` | Generation timestamp | "2025-01-15 10:30:00 UTC" |
| `{{COMMIT_RANGE}}` | Commit range | "abc1234..def5678" |
| `{{DEPTH}}` | Number of commits | "50" |
| `{{ALL_REPORTS_URL}}` | URL to the reports index page | "/perf/index.html" |
| `{{AUDIT_SECTION}}` | Audit results (future) | "" (not yet implemented) |

### Conditional Sections

For index templates, use conditional sections:

```html
{{#BRANCH_REPORTS}}
<div class="section">
    <h2>Branch Reports</h2>
    <ul>
        {{BRANCH_REPORTS}}
    </ul>
</div>
{{/BRANCH_REPORTS}}
```

If `BRANCH_REPORTS` is empty, the entire section (including the wrapping div and heading) is removed.

## Custom CSS

### Inline CSS

Add CSS directly in the template:

```html
<style>
    body { background: #1a1a1a; color: #fff; }
    .plotly { background: #2a2a2a; }
    {{CUSTOM_CSS}}
</style>
```

### External CSS File

Create `.git-perf/styles/custom.css`:

```css
:root {
    --primary-color: #007bff;
    --background: #f5f5f5;
}

body {
    background: var(--background);
    font-family: 'Inter', system-ui, sans-serif;
}

header {
    background: white;
    border-bottom: 3px solid var(--primary-color);
}
```

Use with command line:

```bash
git perf report -o report.html \
  --template .git-perf/templates/branded.html \
  --custom-css .git-perf/styles/custom.css
```

Or in GitHub Actions:

```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    template: '.git-perf/templates/branded.html'
    additional-args: '--custom-css .git-perf/styles/custom.css'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

### Dark Mode Example

```css
@media (prefers-color-scheme: dark) {
    body {
        background: #1a1a1a;
        color: #e0e0e0;
    }

    header, main {
        background: #2a2a2a;
        box-shadow: 0 2px 4px rgba(0,0,0,0.5);
    }

    h1, h2 {
        color: #fff;
    }

    a {
        color: #4db8ff;
    }
}
```

## Configuration

### Via .gitperfconfig

Add template configuration to `.gitperfconfig`:

```toml
[report]
template_path = ".git-perf/templates/default.html"
custom_css_path = ".git-perf/styles/theme.css"
title = "Performance Report - {{PROJECT_NAME}}"
```

### Precedence

Template/CSS resolution follows this order (highest to lowest):

1. **CLI flags** (`--template`, `--custom-css`, `--title`)
2. **Config file** (`.gitperfconfig`)
3. **Built-in defaults**

Example:
```bash
# Override config with CLI flags
git perf report -o report.html \
  --template different-template.html \
  --title "Custom Title"
```

## Examples

### Example 1: Branded Corporate Template

Create a template matching your company's style guide:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}} - Acme Corp</title>
    {{PLOTLY_HEAD}}
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Roboto:wght@400;700&display=swap');

        :root {
            --acme-blue: #0066cc;
            --acme-gray: #f4f4f4;
        }

        body {
            font-family: 'Roboto', sans-serif;
            margin: 0;
            background: var(--acme-gray);
        }

        header {
            background: var(--acme-blue);
            color: white;
            padding: 30px;
        }

        header h1 {
            margin: 0;
            font-weight: 700;
        }

        .logo {
            width: 150px;
            margin-bottom: 20px;
        }

        main {
            max-width: 1400px;
            margin: 30px auto;
            background: white;
            padding: 30px;
            border-radius: 8px;
        }

        footer {
            text-align: center;
            padding: 20px;
            color: #666;
        }
    </style>
</head>
<body>
    <header>
        <img src="/assets/logo.png" alt="Acme Corp" class="logo">
        <h1>{{TITLE}}</h1>
        <p>Generated: {{TIMESTAMP}}</p>
    </header>

    <main>
        {{PLOTLY_BODY}}
    </main>

    <footer>
        <p>&copy; 2025 Acme Corp. Performance tracked with git-perf.</p>
    </footer>
</body>
</html>
```

### Example 2: Minimal Template

Bare-bones template for embedding in larger sites:

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    {{PLOTLY_BODY}}
</body>
</html>
```

### Example 3: Multi-Project Dashboard

Template for repositories with multiple components:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    <style>
        body { font-family: system-ui; max-width: 1600px; margin: 0 auto; padding: 20px; }
        .component { margin-bottom: 40px; padding: 20px; background: #f9f9f9; border-radius: 8px; }
        .component h2 { margin-top: 0; color: #333; border-bottom: 2px solid #007bff; padding-bottom: 10px; }
    </style>
</head>
<body>
    <header>
        <h1>{{TITLE}}</h1>
        <p>Tracking performance across all components | {{TIMESTAMP}}</p>
    </header>

    <main>
        <div class="component">
            <h2>🎨 Frontend (React Components)</h2>
            {{SECTION[frontend]
                measurement-filter: ^test::frontend::
                aggregate-by: median
            }}
        </div>

        <div class="component">
            <h2>⚙️ Backend (API Endpoints)</h2>
            {{SECTION[backend]
                measurement-filter: ^test::backend::
                aggregate-by: median
            }}
        </div>

        <div class="component">
            <h2>💾 Database Queries</h2>
            {{SECTION[database]
                measurement-filter: ^bench::db::
                aggregate-by: median
            }}
        </div>
    </main>
</body>
</html>
```

### Example 4: Documentation Integration

Template with navigation to fit into MkDocs/Sphinx sites:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    <link rel="stylesheet" href="/docs/assets/style.css">
    <style>
        .report-container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 20px;
        }
        {{CUSTOM_CSS}}
    </style>
</head>
<body>
    <nav class="docs-nav">
        <a href="/docs/">Documentation</a>
        <a href="/docs/api/">API Reference</a>
        <a href="/perf/">Performance</a>
        <a href="/perf/index.html">All Reports</a>
    </nav>

    <div class="report-container">
        <h1>{{TITLE}}</h1>
        <p class="metadata">{{COMMIT_RANGE}} ({{DEPTH}} commits) | Generated: {{TIMESTAMP}}</p>

        {{PLOTLY_BODY}}

        <div class="report-footer">
            <p><a href="/perf/index.html">← Back to all reports</a></p>
        </div>
    </div>
</body>
</html>
```

## Best Practices

1. **Start with the default template**: Copy `DEFAULT_HTML_TEMPLATE` from `git_perf/src/reporting.rs` and customize
2. **Test templates locally**: Generate reports with `git perf report` before using in CI
3. **Use relative paths**: For links to docs, use relative paths like `../docs/` for portability
4. **Include all placeholders**: At minimum, include `{{PLOTLY_HEAD}}` and `{{PLOTLY_BODY}}`
5. **Validate HTML**: Run generated reports through an HTML validator
6. **Keep CSS inline or in template**: External CSS files won't be bundled into the HTML
7. **Version control templates**: Commit templates to `.git-perf/templates/` for team sharing
8. **Document template usage**: Add README in `.git-perf/templates/` explaining each template

## Troubleshooting

### Template Not Found

**Error**: `Template file not found: .git-perf/template.html`

**Solution**: Ensure the path is correct and relative to repository root:
```bash
# Check if file exists
ls -la .git-perf/templates/

# Use correct path
git perf report --template .git-perf/templates/my-template.html
```

### Plotly Not Rendering

**Symptom**: Blank page or "Loading..." message.

**Cause**: Missing `{{PLOTLY_HEAD}}` or `{{PLOTLY_BODY}}`.

**Solution**: Ensure template includes both placeholders:
```html
<head>
    {{PLOTLY_HEAD}}  <!-- Required: Loads Plotly.js -->
</head>
<body>
    {{PLOTLY_BODY}}  <!-- Required: Plot content -->
</body>
```

### Custom CSS Not Applied

**Symptom**: Template uses default styles.

**Cause**: `{{CUSTOM_CSS}}` placeholder missing or CSS file not loaded.

**Solution**:
```html
<style>
    /* Your inline styles */
    {{CUSTOM_CSS}}  <!-- Placeholder for --custom-css file -->
</style>
```

Then use:
```bash
git perf report --custom-css path/to/styles.css --template template.html
```

### Dashboard Section Not Rendering

**Symptom**: `{{SECTION[...]}}` appears literally in output.

**Cause**: Invalid section syntax or missing measurements.

**Solution**: Check section syntax:
```html
<!-- Correct -->
{{SECTION[section-id]
    measurement-filter: ^test::
    aggregate-by: median
}}

<!-- Incorrect (missing parameters) -->
{{SECTION[section-id]}}
```

## See Also

- [Dashboard Templates Guide](./dashboard-templates.md) - Multi-section report templates
- [GitHub Pages Integration](./github-pages-integration.md) - Deploying templates to GitHub Pages
- [Integration Tutorial](./INTEGRATION_TUTORIAL.md) - Using templates in CI/CD
