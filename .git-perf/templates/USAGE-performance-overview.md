# Performance Overview Template Usage

This template creates a professional, multi-section performance dashboard that organizes measurements into logical categories with visual styling and comprehensive analytics.

## Template Features

- **5 distinct sections** organizing measurements by type
- **Modern gradient design** with professional styling
- **Change point detection** enabled on all sections
- **Epoch boundary visualization** showing significant periods
- **Descriptive headers** with icons and context
- **Responsive layout** that works on different screen sizes

## Sections Breakdown

### 1. Fibonacci Benchmarks 🧮
**Filter**: `bench::fibonacci_*::mean,bench::fibonacci_*::median,bench::fibonacci_*::mad`

Shows statistical metrics (mean, median, MAD) for Fibonacci benchmark tests. Groups related statistical measurements together.

### 2. Timing Benchmarks ⏱️
**Filter**: `*-benchmark,report,test-measure2` (excluding size measurements)

Displays execution time measurements in nanoseconds for core operations like add-benchmark, report-benchmark, and general tests.

### 3. Size Measurements 📦
**Filter**: `*-size*`

Tracks binary sizes and output sizes (release-binary-size, report-size, etc.) to monitor build artifact growth.

### 4. Trend Analysis 📈
**Filter**: `*::slope`

Isolates slope measurements showing performance acceleration or deceleration trends.

### 5. Complete Overview 🔍
**Filter**: `*` (all measurements)

Comprehensive view of all metrics on a single graph for high-level monitoring.

## How to Use

### Generate Report with This Template

```bash
# Generate HTML report using the performance-overview template
git perf report --template .git-perf/templates/performance-overview.html \
    --output performance-dashboard.html

# Or specify via config
git perf report --template performance-overview \
    --output dashboard.html
```

### Customization Options

You can modify the template to adjust:

1. **Filters**: Change which measurements appear in each section
   ```
   {{SECTION:name|filter=your-pattern-here|...}}
   ```

2. **Depth**: Adjust how many commits to show (default: 40)
   ```
   {{SECTION:name|...|depth=60|...}}
   ```

3. **Aggregation**: Change from median to mean, min, or max
   ```
   {{SECTION:name|...|aggregate=mean|...}}
   ```

4. **Toggle features**: Enable/disable epochs or change detection per section
   ```
   {{SECTION:name|...|enable_epochs=false|enable_change_detection=false}}
   ```

5. **Styling**: Modify the CSS in the `<style>` block to match your branding

## Section Configuration Parameters

Each `{{SECTION:...}}` placeholder supports:

- **filter**: Glob pattern for measurement names (required)
- **exclude**: Glob pattern to exclude measurements (optional)
- **aggregate**: Aggregation method (median, mean, min, max)
- **depth**: Number of commits to include
- **title**: Custom chart title
- **enable_epochs**: Show epoch boundaries (true/false)
- **enable_change_detection**: Show change points (true/false)

## Example Customizations

### Focus on Recent Performance (Last 20 Commits)

```html
{{SECTION:recent|filter=*|aggregate=median|depth=20|title=Recent Performance (20 commits)|enable_epochs=true|enable_change_detection=true}}
```

### Minimal View Without Analytics

```html
{{SECTION:simple|filter=bench::*|aggregate=median|depth=40|title=Benchmarks Only|enable_epochs=false|enable_change_detection=false}}
```

### Compare Specific Tests

```html
{{SECTION:comparison|filter=test-measure2,add-benchmark|aggregate=median|depth=40|title=Test Comparison|enable_epochs=true|enable_change_detection=true}}
```

## Integration with GitHub Pages

This template works seamlessly with the GitHub Pages integration:

1. Configure your repository with the template
2. The report generation workflow will use the template automatically
3. Published reports will have the professional multi-section layout

## Visual Design

The template uses:
- **Gradient header**: Purple gradient (#667eea to #764ba2)
- **Card-based sections**: Light gray backgrounds with white plot containers
- **Icons**: Emoji-based section icons for quick visual identification
- **Badges**: Color-coded badges showing metric units
- **Shadows**: Subtle shadows for depth and hierarchy

## Change Point Detection & Epochs

All sections have both features enabled by default:

- **Change points**: PELT algorithm detects significant performance shifts
- **Epochs**: Boundaries mark distinct performance periods
- **Configuration**: Uses `.gitperfconfig` settings for detection sensitivity

To adjust sensitivity, modify `.gitperfconfig`:

```toml
[change_point]
penalty = 0.5              # Lower = more sensitive
min_data_points = 10
min_magnitude_pct = 5.0
```

## Troubleshooting

**Issue**: Template not found
**Fix**: Ensure you're using the correct path relative to repo root:
```bash
git perf report --template .git-perf/templates/performance-overview.html
```

**Issue**: Sections appear empty
**Fix**: Check that filter patterns match your measurement names exactly. Use:
```bash
git perf list  # See all measurement names
```

**Issue**: Too many/too few change points
**Fix**: Adjust the `penalty` parameter in `.gitperfconfig` or per-measurement overrides.

## Further Reading

- [Dashboard Templates Documentation](../../docs/dashboard-templates.md)
- [Change Point Detection Guide](../../docs/change-point-detection.md)
- [Phase 2b Implementation Summary](../../docs/phase-2b-implementation-summary.md)
