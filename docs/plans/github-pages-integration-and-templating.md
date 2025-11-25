# Plan: GitHub Pages Integration for Multi-Purpose Sites with Templating Support

**Status:** Proposal
**Created:** 2025-11-20

## Overview

Enhance git-perf's GitHub Pages integration to coexist with existing documentation sites and provide templatable HTML report layouts. Currently, the GitHub Actions workflows assume exclusive ownership of GitHub Pages, which prevents repositories with existing documentation from adopting git-perf reports.

## Motivation

### Current Limitations

1. **Exclusive Pages Ownership**: The report and cleanup actions assume they are the only users of the `gh-pages` branch, which conflicts with repositories already hosting documentation on GitHub Pages
2. **Static HTML Generation**: Report layouts are hardcoded in Rust (git_perf/src/reporting.rs:200-209), making it impossible to customize appearance without rebuilding the tool
3. **No Navigation Integration**: Generated reports exist as isolated HTML files without integration into existing site navigation or index pages
4. **Single Source Limitation**: GitHub Pages only allows one publishing source per repository, complicating multi-purpose deployments

### Use Cases Requiring Better Integration

- **Documentation + Performance**: Repositories with MkDocs, Sphinx, or Jekyll documentation that want to add performance tracking
- **Multiple Report Types**: Projects generating both API docs and performance reports
- **Branded Reports**: Organizations wanting reports styled consistently with their documentation theme
- **Versioned Documentation**: Sites hosting multiple documentation versions that want performance data per version
- **Dashboard Integration**: Projects wanting a unified landing page showing both docs and latest performance results

## Goals

1. **Subdirectory Organization**: Deploy reports to a dedicated subdirectory (e.g., `/perf/` or `/reports/`) to avoid conflicts with existing content
2. **Template Support**: Allow customization of HTML report layout, navigation, and styling
3. **Index Generation**: Automatically generate or update an index page listing available reports
4. **Safe Cleanup**: Ensure cleanup actions only remove orphaned reports, not unrelated documentation
5. **Workflow Coordination**: Prevent race conditions when multiple workflows deploy to `gh-pages`
6. **Backward Compatibility**: Maintain support for existing single-purpose deployments

## Non-Goals (Future Work)

- Full static site generator integration (Jekyll themes, Hugo modules, etc.)
- Real-time report regeneration on branch changes
- Automatic versioning of reports by git tag
- Report expiration/archival beyond current cleanup logic
- Cross-repository report aggregation

## Research Findings

### GitHub Pages Constraints

Based on current GitHub Pages capabilities (as of 2025):

1. **Single Publishing Source**: Each repository can only configure one publishing source (branch + optional path)
2. **No Native Multi-Source**: No built-in support for publishing from multiple branches or merging content
3. **Workarounds Required**: Must use GitHub Actions to manually merge content from different sources into the target branch

### Existing Tools and Patterns

1. **peaceiris/actions-gh-pages**:
   - Supports `keep_files: true` to preserve existing files
   - Allows `publish_dir` to specify subdirectory source
   - Used by git-perf report action (.github/actions/report/action.yml:136)

2. **gh-pages-multi**: Community tool for managing multiple subdirectories, demonstrating the common need for this pattern

3. **Common Patterns**:
   - Documentation in `/docs/` or root
   - Reports/dashboards in `/reports/` or `/perf/`
   - Root `index.html` as navigation hub
   - Each subdirectory self-contained with its own assets

### Templating Approaches

Several options for making reports templatable:

1. **Embedded Template Files**: Ship default templates with git-perf, allow override via config
2. **External Template Loading**: Read templates from repository path at report generation time
3. **HTML Wrapper Injection**: Wrap generated Plotly HTML in customizable header/footer
4. **CSS-Only Customization**: Keep structure fixed, allow CSS theme override
5. **Build-Time Generation**: Generate static site with reports during CI build phase

## Proposed Solution

### Phase 1: Subdirectory Organization

#### 1.1 Report Action Modifications

Update `.github/actions/report/action.yml` to support subdirectory deployment:

**New Input Parameters:**
```yaml
inputs:
  reports-subdirectory:
    description: 'Subdirectory within gh-pages for reports (e.g., "perf", "reports"). Empty for root.'
    required: false
    default: ''
  preserve-existing:
    description: 'Preserve existing gh-pages content outside reports subdirectory'
    required: false
    default: 'true'
```

**Implementation:**
- Create reports in `reports/{subdirectory}/` locally
- Use `publish_dir: ./reports/{subdirectory}` when subdirectory is specified
- Set `destination_dir: {subdirectory}` for peaceiris/actions-gh-pages
- Keep `keep_files: true` when `preserve-existing: true`

#### 1.2 Cleanup Action Modifications

Update `.github/actions/cleanup/action.yml` to respect subdirectory boundaries:

**New Input Parameters:**
```yaml
inputs:
  reports-subdirectory:
    description: 'Subdirectory within gh-pages containing reports (must match report action)'
    required: false
    default: ''
```

**Implementation Changes to scripts/cleanup-reports.sh:**
```bash
# Only list reports from the specified subdirectory
if [ -n "$REPORTS_SUBDIR" ]; then
  git ls-tree --name-only gh-pages "$REPORTS_SUBDIR" | \
    sed "s|^$REPORTS_SUBDIR/||" | \
    grep -E '^[0-9a-f]{40}\.html$' | \
    sed 's/\.html$//' | \
    sort > /tmp/commits_with_reports.txt
else
  # Existing behavior for root-level reports
  git ls-tree --name-only gh-pages | \
    grep -E '^[0-9a-f]{40}\.html$' | \
    sed 's/\.html$//' | \
    sort > /tmp/commits_with_reports.txt
fi

# Deletion path adjustment
for commit in $ORPHANED_REPORTS; do
  if [ -n "$REPORTS_SUBDIR" ]; then
    git rm "$REPORTS_SUBDIR/${commit}.html" 2>/dev/null
  else
    git rm "${commit}.html" 2>/dev/null
  fi
done
```

#### 1.3 Workflow Concurrency Management

Add or document concurrency controls to prevent race conditions:

**Best Practice Documentation:**
```yaml
# In user's workflow file
concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false  # Queue deployments, don't cancel
```

**Alternative - Per-Subdirectory Concurrency:**
```yaml
concurrency:
  group: gh-pages-reports-${{ github.ref }}
  cancel-in-progress: false
```

### Phase 2: Report Templating

#### 2.1 Template Architecture

**Template Components:**
1. **Outer HTML**: Full page structure (header, navigation, footer)
2. **Plotly Container**: Div where Plotly chart renders
3. **Metadata Section**: Area for commit info, timestamp, audit results
4. **CSS Customization**: Custom styles and themes
5. **Multi-Configuration Sections**: Multiple report sections with different measurement selections and aggregations

**Template Loading Strategy:**
- Git-perf reads template from `.git-perf/report-template.html` in repository root
- Falls back to built-in default template if not found
- Template uses placeholder syntax for dynamic content
- Supports section-based configuration for multiple report views in a single template

#### 2.2 Template Syntax

Use a simple, safe placeholder system:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    {{CUSTOM_CSS}}
</head>
<body>
    <header>
        <nav>
            <a href="/index.html">Home</a>
            <a href="/docs/">Documentation</a>
            <a href="/perf/">Performance Reports</a>
        </nav>
        <h1>{{TITLE}}</h1>
    </header>

    <main>
        <div class="metadata">
            <p>Generated: {{TIMESTAMP}}</p>
            <p>Commit Range: {{COMMIT_RANGE}}</p>
            <p>Depth: {{DEPTH}}</p>
        </div>

        <div id="plotly-container">
            {{PLOTLY_BODY}}
        </div>

        {{AUDIT_SECTION}}
    </main>

    <footer>
        <p>Generated by <a href="https://github.com/kaihowl/git-perf">git-perf</a></p>
    </footer>
</body>
</html>
```

**Placeholders:**
- `{{TITLE}}`: Report title (configurable via CLI)
- `{{PLOTLY_HEAD}}`: Plotly JavaScript/CSS dependencies
- `{{PLOTLY_BODY}}`: Plotly chart HTML
- `{{CUSTOM_CSS}}`: Inline CSS from config or separate file
- `{{TIMESTAMP}}`: Generation timestamp
- `{{COMMIT_RANGE}}`: First..Last commit short hashes
- `{{DEPTH}}`: Number of commits in report
- `{{AUDIT_SECTION}}`: Optional audit results HTML

#### 2.3 Implementation in reporting.rs

Modify `PlotlyReporter::as_bytes()` method:

```rust
impl PlotlyReporter {
    fn as_bytes(&self, template: Option<&str>, metadata: &ReportMetadata) -> Vec<u8> {
        let plotly_html = if let Some(y_axis) = self.compute_y_axis() {
            let mut plot_with_y_axis = self.plot.clone();
            let mut layout = plot_with_y_axis.layout().clone();
            layout = layout.y_axis(y_axis);
            plot_with_y_axis.set_layout(layout);
            plot_with_y_axis.to_html()
        } else {
            self.plot.to_html()
        };

        if let Some(template_str) = template {
            apply_template(template_str, &plotly_html, metadata)
        } else {
            // Existing behavior - return Plotly's HTML directly
            plotly_html.as_bytes().to_vec()
        }
    }
}

fn apply_template(template: &str, plotly_html: &str, metadata: &ReportMetadata) -> Vec<u8> {
    // Parse plotly_html to extract <head> and <body> content
    let (plotly_head, plotly_body) = extract_plotly_parts(plotly_html);

    let output = template
        .replace("{{TITLE}}", &metadata.title)
        .replace("{{PLOTLY_HEAD}}", &plotly_head)
        .replace("{{PLOTLY_BODY}}", &plotly_body)
        .replace("{{CUSTOM_CSS}}", &metadata.custom_css)
        .replace("{{TIMESTAMP}}", &metadata.timestamp)
        .replace("{{COMMIT_RANGE}}", &metadata.commit_range)
        .replace("{{DEPTH}}", &metadata.depth.to_string())
        .replace("{{AUDIT_SECTION}}", &metadata.audit_html);

    output.as_bytes().to_vec()
}

struct ReportMetadata {
    title: String,
    custom_css: String,
    timestamp: String,
    commit_range: String,
    depth: usize,
    audit_html: String,
}
```

#### 2.4 Configuration Support

Add to `.gitperfconfig`:

```toml
[report]
template_path = ".git-perf/report-template.html"
custom_css_path = ".git-perf/report-styles.css"
title = "Performance Report - {{PROJECT_NAME}}"
include_audit_in_report = true

[report.navigation]
home_url = "/index.html"
docs_url = "/docs/"
reports_index_url = "/perf/index.html"
```

CLI flag override:
```bash
git perf report --template .git-perf/custom-template.html \
                --custom-css .git-perf/branded.css \
                --title "My Custom Report"
```

#### 2.5 Multi-Configuration Templates (Dashboard Support)

**Motivation:**
Allow a single template to render multiple independent report views with different measurement selections and aggregation options, creating dashboard-style reports without running `git perf report` multiple times.

**Use Cases:**
- **Aggregated vs Raw**: Show both raw measurements and median-aggregated trends side-by-side
- **Comparison Views**: Display different measurements (e.g., CPU time vs Memory usage) in separate sections
- **Multi-Metric Dashboards**: Combine test durations, benchmark results, and build times in one report
- **Split Comparisons**: Show same measurement split by different metadata keys (e.g., by OS vs by architecture)

**Configuration Syntax:**

Templates can define multiple report sections using special placeholder blocks:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
    {{CUSTOM_CSS}}
</head>
<body>
    <header>
        <h1>Performance Dashboard</h1>
        <p>Generated: {{TIMESTAMP}}</p>
    </header>

    <main>
        <!-- Section 1: Raw test durations -->
        <section class="report-section">
            <h2>Test Duration Trends (Raw Data)</h2>
            {{SECTION[test-durations-raw]
                measurement-filter: ^test::
                aggregate-by: none
            }}
        </section>

        <!-- Section 2: Aggregated benchmark results -->
        <section class="report-section">
            <h2>Benchmark Performance (Median)</h2>
            {{SECTION[bench-median]
                measurement-filter: ^bench::
                aggregate-by: median
            }}
        </section>

        <!-- Section 3: Build times split by platform -->
        <section class="report-section">
            <h2>Build Times by Platform</h2>
            {{SECTION[build-platform]
                measurement-filter: ^build_time$
                separate-by: os,arch
                aggregate-by: median
            }}
        </section>

        <!-- Section 4: Memory usage comparison -->
        <section class="report-section">
            <h2>Memory Usage (Min vs Max)</h2>
            <div class="side-by-side">
                <div>
                    <h3>Minimum</h3>
                    {{SECTION[memory-min]
                        measurement-filter: memory_.*
                        aggregate-by: min
                    }}
                </div>
                <div>
                    <h3>Maximum</h3>
                    {{SECTION[memory-max]
                        measurement-filter: memory_.*
                        aggregate-by: max
                    }}
                </div>
            </div>
        </section>

        {{AUDIT_SECTION}}
    </main>

    <footer>
        <p>Generated by git-perf</p>
    </footer>
</body>
</html>
```

**Section Configuration Parameters:**

Each `{{SECTION[section-id] ... }}` block supports:

- `measurement-filter`: Regex pattern for selecting measurements (equivalent to `--filter` or `--measurement`)
- `key-value-filter`: Key-value pairs to match (e.g., `os=linux,arch=x64`)
- `separate-by`: Comma-separated list of metadata keys to split traces by (equivalent to `--separate-by`)
- `aggregate-by`: Aggregation function: `none`, `min`, `max`, `median`, `mean` (equivalent to `--aggregate-by`)
- `depth`: Number of commits (overrides global `-n`, optional)
- `title`: Section-specific title for the chart (optional, defaults to section-id)

**Implementation Approach:**

1. **Template Parsing**: During template loading, parse and extract all `{{SECTION[...]}}` blocks
2. **Section Processing**: For each section:
   - Extract configuration parameters
   - Generate a separate PlotlyReporter with section-specific filters
   - Render the Plotly chart HTML
   - Replace the `{{SECTION[...]}}` placeholder with the generated chart
3. **Resource Sharing**: All sections share the same commit data (loaded once) but apply different filters and aggregations
4. **Plotly Configuration**: Each section gets its own Plotly instance with independent configuration

**Rust Implementation Sketch:**

```rust
struct SectionConfig {
    id: String,
    measurement_filter: Option<String>,
    key_value_filter: Vec<(String, String)>,
    separate_by: Vec<String>,
    aggregate_by: Option<ReductionFunc>,
    depth: Option<usize>,
    title: Option<String>,
}

fn parse_template_sections(template: &str) -> Vec<SectionConfig> {
    // Regex to extract {{SECTION[id] param: value ... }}
    // Returns list of SectionConfig structs
}

fn generate_multi_section_report(
    template: &str,
    commits: &[Commit],
    global_metadata: &ReportMetadata,
) -> Vec<u8> {
    let sections = parse_template_sections(template);
    let mut output = template.to_string();

    for section in sections {
        // Create a reporter for this section
        let mut reporter = PlotlyReporter::new();
        reporter.add_commits(commits);

        // Apply section-specific filtering and aggregation
        let filtered_measurements = apply_section_filters(
            commits,
            &section.measurement_filter,
            &section.key_value_filter,
        );

        // Generate traces with section-specific aggregation
        for measurement_name in get_unique_measurement_names(&filtered_measurements) {
            let trace_data = prepare_trace_data(
                &filtered_measurements,
                measurement_name,
                &section.separate_by,
                section.aggregate_by,
            );

            if let Some(agg) = section.aggregate_by {
                reporter.add_summarized_trace(trace_data, measurement_name, &section.separate_by);
            } else {
                reporter.add_trace(trace_data, measurement_name, &section.separate_by);
            }
        }

        // Render section HTML
        let section_html = reporter.as_bytes();
        let section_html_str = String::from_utf8_lossy(&section_html);

        // Replace {{SECTION[id] ... }} with generated HTML
        output = replace_section_placeholder(&output, &section.id, &section_html_str);
    }

    // Apply global placeholders (TITLE, TIMESTAMP, etc.)
    output = apply_global_placeholders(&output, global_metadata);

    output.as_bytes().to_vec()
}
```

**CLI Behavior with Multi-Section Templates:**

When a multi-section template is provided:

```bash
git perf report --template .git-perf/dashboard-template.html \
                --output reports/main.html
```

The CLI arguments (`--filter`, `--measurement`, `--aggregate-by`, etc.) are **ignored** when the template contains `{{SECTION[...]}}` blocks. Instead, all configuration comes from the template itself.

For backward compatibility, if the template contains **no** `{{SECTION[...]}}` blocks, the existing behavior applies (single report with CLI-specified filters).

**Using Multiple Dashboard Templates:**

Simply keep different dashboard templates in your repository and reference them directly:

```bash
# Generate test-focused dashboard
git perf report --template .git-perf/test-dashboard.html \
                --output reports/tests.html

# Generate benchmark-focused dashboard
git perf report --template .git-perf/bench-dashboard.html \
                --output reports/benchmarks.html

# Generate comprehensive overview
git perf report --template .git-perf/overview-dashboard.html \
                --output reports/overview.html
```

The template files themselves contain all the configuration needed for each dashboard view.

**Example: Complete Dashboard Template**

`.git-perf/dashboard-template.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}} - Performance Dashboard</title>
    {{PLOTLY_HEAD}}
    <style>
        body {
            font-family: system-ui, -apple-system, sans-serif;
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
        .report-section {
            background: white;
            padding: 20px;
            margin-bottom: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .report-section h2 {
            margin-top: 0;
            color: #333;
            border-bottom: 2px solid #007bff;
            padding-bottom: 10px;
        }
        .side-by-side {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 20px;
        }
        .metadata {
            background: #f8f9fa;
            padding: 15px;
            border-radius: 4px;
            margin-bottom: 20px;
        }
        .metadata p {
            margin: 5px 0;
            color: #666;
        }
        nav {
            display: flex;
            gap: 20px;
            margin-bottom: 15px;
        }
        nav a {
            text-decoration: none;
            color: #007bff;
            font-weight: 500;
        }
        nav a:hover {
            text-decoration: underline;
        }
        footer {
            text-align: center;
            padding: 20px;
            color: #666;
            font-size: 0.9em;
        }
        {{CUSTOM_CSS}}
    </style>
</head>
<body>
    <header>
        <nav>
            <a href="/index.html">Home</a>
            <a href="/docs/">Documentation</a>
            <a href="/perf/">All Reports</a>
        </nav>
        <h1>{{TITLE}}</h1>
        <div class="metadata">
            <p><strong>Generated:</strong> {{TIMESTAMP}}</p>
            <p><strong>Commit Range:</strong> {{COMMIT_RANGE}}</p>
            <p><strong>Commits Analyzed:</strong> {{DEPTH}}</p>
        </div>
    </header>

    <main>
        <!-- Quick Stats Overview -->
        <section class="report-section">
            <h2>📊 Test Suite Performance (Last 50 Commits)</h2>
            {{SECTION[test-overview]
                measurement-filter: ^test::
                aggregate-by: median
                depth: 50
                title: Test Execution Time
            }}
        </section>

        <!-- Benchmark Trends -->
        <section class="report-section">
            <h2>🚀 Benchmark Results</h2>
            <p>Median benchmark performance across all platforms</p>
            {{SECTION[benchmark-median]
                measurement-filter: ^bench::.*::mean$
                aggregate-by: median
                title: Benchmark Mean Values
            }}
        </section>

        <!-- Build Time Analysis -->
        <section class="report-section">
            <h2>⚙️ Build Times by Platform</h2>
            {{SECTION[build-by-platform]
                measurement-filter: ^build_time$
                separate-by: os,arch
                aggregate-by: median
                title: Build Duration
            }}
        </section>

        <!-- Memory Comparison -->
        <section class="report-section">
            <h2>💾 Memory Usage Trends</h2>
            <div class="side-by-side">
                <div>
                    <h3>Peak Memory (Max)</h3>
                    {{SECTION[memory-max]
                        measurement-filter: memory_peak
                        aggregate-by: max
                        title: Peak Memory Usage
                    }}
                </div>
                <div>
                    <h3>Average Memory</h3>
                    {{SECTION[memory-avg]
                        measurement-filter: memory_peak
                        aggregate-by: mean
                        title: Average Memory Usage
                    }}
                </div>
            </div>
        </section>

        <!-- Raw Data Detail View -->
        <section class="report-section">
            <h2>📈 Detailed Raw Measurements (Integration Tests)</h2>
            <p>Individual test runs without aggregation - shows variability</p>
            {{SECTION[integration-raw]
                measurement-filter: ^test::integration::
                aggregate-by: none
                key-value-filter: test_type=integration
                title: Integration Test Raw Data
            }}
        </section>

        <!-- Audit Results -->
        {{AUDIT_SECTION}}
    </main>

    <footer>
        <p>Generated by <a href="https://github.com/kaihowl/git-perf">git-perf</a> |
           <a href="https://github.com/user/repo">View Source</a></p>
    </footer>
</body>
</html>
```

**Benefits:**

1. **Single Command**: Generate comprehensive dashboard with one `git perf report` invocation
2. **Consistency**: All sections use the same commit data and timestamp, ensuring coherent comparison
3. **Performance**: Measurements loaded once and filtered multiple times (vs running git-perf N times)
4. **Flexibility**: Mix and match raw data, aggregations, and different measurement sets
5. **Maintainability**: Template defines all report logic in one place
6. **Version Control**: Dashboard configuration is tracked alongside code

### Phase 3: Index Generation

#### 3.1 Reports Index Page

Generate an index page listing all available reports:

**New Subcommand:**
```bash
git perf generate-index --output perf/index.html \
                        --title "Performance Reports" \
                        --template .git-perf/index-template.html
```

**Index Template (.git-perf/index-template.html):**
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    {{CUSTOM_CSS}}
</head>
<body>
    <header>
        <nav>
            <a href="/index.html">Home</a>
            <a href="/docs/">Documentation</a>
            <a href="/perf/">Performance Reports</a>
        </nav>
        <h1>{{TITLE}}</h1>
    </header>

    <main>
        <p>Latest performance reports for this project.</p>

        <h2>Branch Reports</h2>
        <ul>
            {{BRANCH_REPORTS}}
        </ul>

        <h2>Recent Commit Reports</h2>
        <table>
            <thead>
                <tr>
                    <th>Commit</th>
                    <th>Date</th>
                    <th>Author</th>
                    <th>Report</th>
                </tr>
            </thead>
            <tbody>
                {{COMMIT_REPORTS}}
            </tbody>
        </table>
    </main>

    <footer>
        <p>Generated by <a href="https://github.com/kaihowl/git-perf">git-perf</a></p>
    </footer>
</body>
</html>
```

#### 3.2 Index Generation Logic

**Implementation Approach:**
1. Scan `gh-pages` branch (or specified subdirectory) for `.html` files
2. Categorize reports:
   - Branch reports (named after branches, e.g., `main.html`, `develop.html`)
   - Commit reports (40-character hex SHA, e.g., `a1b2c3d4...html`)
   - Custom reports (other names)
3. Gather metadata for each report (commit date, author from git log)
4. Generate index HTML using template
5. Output to specified path

**Action Integration:**
```yaml
# .github/actions/report/action.yml
- name: Generate reports index
  if: ${{ inputs.generate-index == 'true' }}
  shell: bash
  run: |
    # After publishing individual report
    git perf generate-index \
      --output reports/index.html \
      --subdirectory "${{ inputs.reports-subdirectory }}" \
      --template .git-perf/index-template.html
```

#### 3.3 Root Navigation Integration

For repositories with existing documentation, provide guidance for integrating the reports index:

**Option 1: Manual Integration**
Users add a link to their existing docs site:
```markdown
<!-- In existing docs/index.md -->
- [Performance Reports](../perf/index.html)
```

**Option 2: Automated Index Update**
If the root `index.html` follows a pattern, git-perf could inject a link:
```yaml
inputs:
  update-root-index:
    description: 'Add link to reports in root index.html'
    required: false
    default: 'false'
  root-index-selector:
    description: 'CSS selector where to inject reports link (e.g., "nav ul")'
    required: false
    default: ''
```

This would be complex and error-prone, so **recommend manual integration** with clear documentation.

## Integration Examples

### Example 1: MkDocs Documentation + Performance Reports

**Repository Structure:**
```
repo/
├── docs/               # MkDocs source
│   ├── index.md
│   ├── api.md
│   └── mkdocs.yml
├── .git-perf/
│   ├── report-template.html
│   └── index-template.html
└── .github/
    └── workflows/
        ├── docs.yml    # Build MkDocs → gh-pages/
        └── perf.yml    # Build reports → gh-pages/perf/
```

**Docs Workflow (docs.yml):**
```yaml
name: Deploy Documentation
on:
  push:
    branches: [main]
    paths: ['docs/**']

permissions:
  contents: write
  pages: write

concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false

jobs:
  deploy-docs:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-python@v5
        with:
          python-version: 3.x
      - run: pip install mkdocs-material
      - run: mkdocs build

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./site
          keep_files: true  # Preserve perf/ reports
```

**Performance Workflow (perf.yml):**
```yaml
name: Performance Reports
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: write
  pages: write
  pull-requests: write

concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false

jobs:
  report:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 100

      - uses: kaihowl/git-perf/.github/actions/report@master
        with:
          reports-subdirectory: 'perf'
          preserve-existing: 'true'
          generate-index: 'true'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Result:**
- Docs at: `https://user.github.io/repo/`
- Reports at: `https://user.github.io/repo/perf/`
- Reports index: `https://user.github.io/repo/perf/index.html`

### Example 2: Jekyll Site with Performance Dashboard

**Root Index (_layouts/default.html in Jekyll):**
```html
<nav>
  <a href="{{ site.baseurl }}/">Home</a>
  <a href="{{ site.baseurl }}/docs/">Docs</a>
  <a href="{{ site.baseurl }}/perf/">Performance</a>
</nav>
```

**Jekyll Workflow:**
```yaml
- name: Build Jekyll
  run: bundle exec jekyll build

- name: Deploy Jekyll + Reports
  uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./_site
    keep_files: true  # Reports added separately by perf workflow
```

### Example 3: Simple Static Site with Dashboard

Create a minimal root index manually:

**index.html (committed to gh-pages):**
```html
<!DOCTYPE html>
<html>
<head>
    <title>Project Name</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body { font-family: system-ui; max-width: 800px; margin: 50px auto; }
        nav { display: flex; gap: 20px; margin-bottom: 30px; }
        nav a { text-decoration: none; color: #0366d6; }
        .section { margin: 30px 0; }
    </style>
</head>
<body>
    <nav>
        <a href="/">Home</a>
        <a href="/perf/">Performance Reports</a>
        <a href="https://github.com/user/repo">GitHub</a>
    </nav>

    <h1>Project Name</h1>
    <p>Welcome to the project homepage.</p>

    <div class="section">
        <h2>Quick Links</h2>
        <ul>
            <li><a href="/perf/main.html">Latest Performance (main branch)</a></li>
            <li><a href="/perf/index.html">All Performance Reports</a></li>
        </ul>
    </div>
</body>
</html>
```

This index is preserved by `keep_files: true` when reports are deployed to `/perf/`.

## Implementation Phases

### Phase 1: Subdirectory Support (Weeks 1-2)
- [ ] Add `reports-subdirectory` input to report action
- [ ] Add `destination_dir` support for peaceiris action
- [ ] Update cleanup script to respect subdirectory
- [ ] Add concurrency documentation
- [ ] Test with existing single-purpose deployments (backward compat)
- [ ] Test with multi-workflow scenario
- [ ] Update action READMEs with examples

### Phase 2: Basic Templating (Weeks 3-5)
- [ ] Design template placeholder syntax
- [ ] Implement template loading in git-perf CLI
- [ ] Refactor `PlotlyReporter::as_bytes()` for template support
- [ ] Create default template matching current output
- [ ] Add CLI flags: `--template`, `--custom-css`, `--title`
- [ ] Add config file support for template paths
- [ ] Write tests for template substitution
- [ ] Document template creation guide

#### Phase 2b: Multi-Configuration Templates (Weeks 6-7)
- [ ] Design and implement `{{SECTION[...]}}` placeholder parser
- [ ] Create `SectionConfig` struct and parsing logic
- [ ] Implement multi-section report generation pipeline
- [ ] Add support for section-specific parameters (filter, aggregate-by, separate-by, depth, title)
- [ ] Implement CLI argument override behavior (ignore CLI when sections present)
- [ ] Add `key-value-filter` parameter parsing
- [ ] Refactor reporting.rs to support reusable report generation for sections
- [ ] Write unit tests for section parsing and configuration
- [ ] Write integration tests for multi-section reports
- [ ] Document dashboard template syntax and usage
- [ ] Create example dashboard templates (test overview, benchmark comparison, memory analysis)
- [ ] Add examples showing multiple dashboard templates in one repository

### Phase 3: Index Generation (Weeks 5-6)
- [ ] Implement `generate-index` subcommand
- [ ] Create default index template
- [ ] Add report scanning and categorization logic
- [ ] Integrate index generation into report action
- [ ] Add `generate-index` input to action
- [ ] Create example index templates for common scenarios
- [ ] Document index customization

### Phase 4: Documentation & Examples (Week 7)
- [ ] Create integration guide for MkDocs
- [ ] Create integration guide for Jekyll
- [ ] Create integration guide for static sites
- [ ] Add troubleshooting section
- [ ] Create migration guide for existing users
- [ ] Update INTEGRATION_TUTORIAL.md
- [ ] Add example templates to repository

## Testing Strategy

### Unit Tests
- Template placeholder replacement
- Plotly HTML parsing (head/body extraction)
- Report metadata collection
- Index generation logic
- Section placeholder parsing and extraction
- Section configuration parameter parsing
- Multi-section report generation with different filters
- Section-specific aggregation (none, min, max, median, mean)
- Key-value filter parsing and application

### Integration Tests
- Deploy to subdirectory preserves root files
- Multiple workflows don't conflict
- Cleanup only removes reports in subdirectory
- Custom templates render correctly
- Index lists all reports

### Manual Testing Scenarios
1. Fresh repository with no existing gh-pages
2. Repository with Jekyll documentation
3. Repository with MkDocs documentation
4. Repository with custom static site
5. PR workflow + main workflow concurrency
6. Template with all placeholders
7. Template with minimal placeholders
8. Large number of reports (100+) in index
9. Multi-section template with 2-5 sections
10. Dashboard template with mixed aggregation (raw + median + max)
11. Dashboard template with separate-by in different sections
12. Dashboard template with different depth per section
13. Backward compatibility: simple template without SECTION blocks
14. CLI arguments ignored when SECTION blocks present
15. Dashboard with key-value-filter constraints

## Documentation Requirements

### User Documentation
1. **Integration Guide** (new: `docs/github-pages-integration.md`):
   - Overview of subdirectory approach
   - Step-by-step for common static site generators
   - Troubleshooting concurrent workflows
   - Migration from root-level reports

2. **Template Guide** (new: `docs/report-templating.md`):
   - Template syntax reference
   - Available placeholders
   - CSS customization
   - Example templates gallery

3. **Action README Updates**:
   - Document new inputs
   - Update examples for subdirectory usage
   - Add multi-workflow coordination examples

### Developer Documentation
1. **Architecture Decision Record** (new: `docs/adr/0001-github-pages-subdirectories.md`):
   - Why subdirectory approach chosen
   - Alternatives considered
   - Trade-offs

2. **Template Implementation** (comments in `reporting.rs`):
   - Template parsing approach
   - Security considerations
   - Performance implications

## Security Considerations

### Template Injection
- **Risk**: If templates can include user-controlled content, XSS attacks possible
- **Mitigation**:
  - Templates read from repository (trusted source)
  - No external URL fetching
  - HTML-escape all metadata values before substitution
  - Document that templates should be reviewed before use

### Multi-Section Template Security
- **Risk**: Malformed section configurations could cause parsing errors or infinite loops
- **Mitigation**:
  - Strict regex validation for section placeholder syntax
  - Maximum section limit (e.g., 20 sections per template)
  - Timeout protection for section parsing
  - Validate all parameter values before processing
  - Sanitize regex patterns in measurement-filter to prevent ReDoS attacks

### Section Configuration Validation
- **Risk**: Invalid aggregation methods, filters, or parameters could cause crashes
- **Mitigation**:
  - Whitelist valid aggregation methods (none, min, max, median, mean)
  - Validate depth values (must be positive integers)
  - Pre-compile and validate all regex patterns
  - Provide clear error messages for invalid configurations
  - Continue processing other sections if one fails (with warning)

### Path Traversal
- **Risk**: `reports-subdirectory` input could include `../` to escape intended directory
- **Mitigation**:
  - Validate input: reject paths containing `..`, absolute paths, or special characters
  - Use path normalization before use
  - Document allowed characters: `[a-zA-Z0-9_-]`

### Cleanup Script Safety
- **Risk**: Subdirectory misconfiguration could delete unrelated files
- **Mitigation**:
  - Dry-run mode by default in docs
  - Validate that commits in reports have 40-char hex SHA pattern
  - Only delete files matching `{SHA}.html` pattern
  - Log all deletions before confirming

## Migration Path for Existing Users

### Scenario 1: Existing Root-Level Reports

**Current Setup:**
```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

**No Change Required**: Continue working as before.

**Optional Migration to Subdirectory:**
```yaml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

Then manually move old reports:
```bash
git checkout gh-pages
mkdir -p perf
git mv *.html perf/ || true  # Move SHA-named reports
git commit -m "refactor: move reports to /perf subdirectory"
git push origin gh-pages
```

### Scenario 2: Adding Reports to Existing Documentation Site

**Step 1**: Add concurrency control to docs workflow:
```yaml
concurrency:
  group: gh-pages-deploy
  cancel-in-progress: false
```

**Step 2**: Add `keep_files: true` to docs deployment:
```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./docs
    keep_files: true  # NEW
```

**Step 3**: Add performance workflow:
```yaml
# .github/workflows/performance.yml
- uses: kaihowl/git-perf/.github/actions/report@master
  with:
    reports-subdirectory: 'perf'
    preserve-existing: 'true'
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

**Step 4**: Add link to reports in documentation navigation (manual).

## Alternatives Considered

### Alternative 1: Separate Repository for Reports
**Approach**: Deploy reports to a separate `repo-perf` repository.

**Pros:**
- Complete isolation from documentation
- No workflow coordination needed
- Simpler implementation

**Cons:**
- Split project context across repositories
- Harder to discover reports
- Additional repository maintenance
- Less cohesive project presence

**Decision**: Rejected - Most users want unified project presence.

### Alternative 2: Full Jekyll Theme Integration
**Approach**: Create git-perf as a Jekyll plugin/theme.

**Pros:**
- Native Jekyll site integration
- Leverage Jekyll's templating (Liquid)
- Automatic navigation inclusion

**Cons:**
- Jekyll-specific (excludes MkDocs, Hugo, etc.)
- Complex implementation
- Jekyll knowledge required from users
- Overhead for simple static sites

**Decision**: Rejected - Too specific to one generator. Subdirectory + templating is generator-agnostic.

### Alternative 3: Client-Side Report Generation
**Approach**: Commit measurement JSON, generate charts in browser.

**Pros:**
- Extremely flexible templates (any web framework)
- No server-side generation needed
- Interactive filtering/sorting

**Cons:**
- Large JSON files for long histories
- Requires JavaScript
- Slower initial load
- More complex troubleshooting

**Decision**: Rejected for now - Could be future enhancement, but Plotly's server-side generation is proven.

### Alternative 4: Embed Reports in Documentation Pages
**Approach**: Generate Plotly divs that can be embedded in Markdown/HTML.

**Pros:**
- Reports inline with relevant documentation
- No separate navigation needed

**Cons:**
- Documentation pages become large
- Build process more complex
- Less flexibility for standalone reports
- Different integration for each doc generator

**Decision**: Rejected - Subdirectory approach provides this optionally while keeping core simple.

## Success Criteria

### Adoption Metrics
- 5+ repositories successfully integrating reports with existing docs within 3 months
- No reported conflicts/data loss from multi-workflow deployments
- Positive feedback on template customization

### Technical Metrics
- Zero test failures for subdirectory isolation
- Backward compatibility maintained (no breaking changes)
- Template rendering adds <100ms to report generation

### Documentation Metrics
- Integration guide used by 80%+ of new multi-purpose adopters
- Template guide referenced in customization issues
- <5% of issues related to configuration confusion

## Open Questions

1. **Default subdirectory name**: Should there be a recommended default (`perf`, `reports`, `performance`)? Or require explicit configuration?
   - **Recommendation**: Default to empty (root) for backward compat, document `perf` as recommended subdirectory in integration guide.

2. **Template distribution**: Should example templates be in main repo, separate repo, or both?
   - **Recommendation**: Include 2-3 basic templates in main repo under `.git-perf/templates/`, point to community templates in docs.

3. **Index update frequency**: Should index be regenerated on every report, or separate manual step?
   - **Recommendation**: Opt-in via `generate-index: true` input, allowing users to control frequency.

4. **Backward compatibility period**: How long to maintain dual support for root and subdirectory?
   - **Recommendation**: Indefinite - root-level is still valid for single-purpose repos.

5. **Template validation**: Should git-perf validate templates before use?
   - **Recommendation**: Basic validation (check all required placeholders present) with warnings, not errors.

## References

### External Documentation
- [GitHub Pages Documentation](https://docs.github.com/en/pages)
- [peaceiris/actions-gh-pages](https://github.com/peaceiris/actions-gh-pages)
- [Jekyll Documentation](https://jekyllrb.com/docs/)
- [MkDocs Documentation](https://www.mkdocs.org/)

### Related Issues/Discussions
- [GitHub Community: Multiple pages from same repo](https://github.com/orgs/community/discussions/21582)
- [gh-pages-multi tool](https://github.com/koumoul-dev/gh-pages-multi)

### Internal References
- `.github/actions/report/action.yml` - Current report action
- `.github/actions/cleanup/action.yml` - Current cleanup action
- `scripts/cleanup-reports.sh` - Cleanup script
- `git_perf/src/reporting.rs` - Report generation implementation

## Changelog

### 2025-11-20 - Initial Proposal
- Defined problem statement and motivation
- Researched GitHub Pages constraints and community patterns
- Designed three-phase implementation approach
- Documented subdirectory organization strategy
- Designed template placeholder syntax
- Created index generation proposal
- Provided integration examples for common scenarios
