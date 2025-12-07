# Git-Perf Report Templates

This directory contains example report templates for git-perf's multi-section dashboard feature.

## Available Templates

- **performance-overview.html** ⭐ - Professional multi-section dashboard with modern styling (recommended)
- **simple-dashboard.html** - Basic multi-section template with aggregated and raw views
- **dashboard-example.html** - Comprehensive example showing various section configurations

## Quick Start

```bash
git perf report --template .git-perf/templates/performance-overview.html --output dashboard.html
```

## Documentation

For complete documentation on template syntax, section parameters, and examples, see:
**[Dashboard Templates Guide](../../docs/dashboard-templates.md)**

## Creating Your Own Template

1. Copy an existing template as a starting point
2. Edit sections to match your measurements (configured in `.gitperfconfig`)
3. Generate reports using `git perf report --template <path> --output <file>`

See the [Dashboard Templates Guide](../../docs/dashboard-templates.md) for full syntax reference and examples.
