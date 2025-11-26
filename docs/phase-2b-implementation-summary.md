# Phase 2b Implementation Summary: Multi-Section Dashboard Templates

**Date:** 2025-11-26
**Plan Reference:** [docs/plans/github-pages-integration-and-templating.md](./plans/github-pages-integration-and-templating.md)
**Phase:** 2b - Multi-Configuration Templates

## Overview

Implemented multi-section dashboard template support for git-perf reports, allowing users to create comprehensive performance dashboards with multiple independent report sections in a single HTML file.

## Implementation Status

### ✅ Completed Tasks

1. **Section Placeholder Parser**
   - Implemented regex-based parser for `{{SECTION[id] ...}}` syntax
   - Supports multiline parameter blocks
   - Validates section IDs for uniqueness

2. **SectionConfig Struct**
   - Captures all section-specific parameters
   - Supports: `measurement-filter`, `key-value-filter`, `separate-by`, `aggregate-by`, `depth`, `title`

3. **Multi-Section Report Generation Pipeline**
   - `generate_section_plot()` - Creates individual plot for each section
   - `generate_multi_section_report()` - Orchestrates section generation and template assembly
   - Automatic detection of multi-section templates

4. **Section-Specific Parameters**
   - **measurement-filter**: Regex pattern for measurement selection
   - **key-value-filter**: Metadata filtering (e.g., `os=linux,arch=x64`)
   - **separate-by**: Split traces by metadata keys
   - **aggregate-by**: `none`, `min`, `max`, `median`, `mean`
   - **depth**: Per-section commit count override
   - **title**: Section-specific chart title (parsed, display TBD)

5. **CLI Argument Override Behavior**
   - When template contains sections, CLI args (`--filter`, `--measurement`, `--aggregate-by`, etc.) are ignored
   - Log message warns user about this behavior

6. **Refactored Reporting Module**
   - Extracted plot generation logic for reusability
   - Maintained backward compatibility with simple templates
   - Early return for multi-section templates to avoid duplicate processing

7. **Unit Tests**
   - 15+ tests for section parsing
   - Tests for all parameter types
   - Tests for error conditions (invalid values, duplicate IDs)
   - Tests for template parsing (empty, single, multiple sections)
   - All tests passing ✅

8. **Documentation**
   - Comprehensive guide: `docs/dashboard-templates.md`
   - Template README: `.git-perf/templates/README.md`
   - Inline code documentation

9. **Example Templates**
   - `simple-dashboard.html` - Basic 3-section example
   - `dashboard-example.html` - Comprehensive 5-section dashboard

## Code Changes

### Files Modified

- **git_perf/src/reporting.rs** (~220 lines added)
  - `SectionConfig` struct and parser
  - `parse_template_sections()` function
  - `generate_section_plot()` function
  - `generate_multi_section_report()` function
  - Modified `report()` function to detect and handle multi-section templates
  - 15 new unit tests

### Files Created

- `.git-perf/templates/dashboard-example.html` - Comprehensive example
- `.git-perf/templates/simple-dashboard.html` - Basic example
- `.git-perf/templates/README.md` - Template usage guide
- `docs/dashboard-templates.md` - Complete documentation
- `docs/phase-2b-implementation-summary.md` - This file

## Technical Details

### Section Parsing

Uses regex with DOTALL flag to match multi-line section blocks:
```rust
r"(?s)\{\{SECTION\[([^\]]+)\](.*?)\}\}"
```

Parameters are parsed line-by-line from the content between `{{SECTION[id]` and `}}`.

### Report Generation Flow

1. Load template file
2. Check for section placeholders
3. If sections found:
   - Parse all section configurations
   - For each section:
     - Apply filters and aggregations
     - Generate plot
     - Replace placeholder with plot HTML
   - Apply global placeholders
   - Return complete HTML
4. If no sections, use existing single-section logic

### Backward Compatibility

- Templates without `{{SECTION[...]}}` work as before
- `{{PLOTLY_BODY}}` still works for simple templates
- CLI arguments still apply to simple templates
- No breaking changes to existing functionality

## Usage Examples

### Basic Usage

```bash
git perf report --template .git-perf/templates/simple-dashboard.html --output report.html
```

### With Custom CSS

```bash
git perf report \
  --template .git-perf/templates/dashboard-example.html \
  --custom-css .git-perf/styles.css \
  --title "My Project Performance" \
  --output dashboard.html
```

### Example Section Syntax

```html
{{SECTION[test-median]
    measurement-filter: ^test::
    aggregate-by: median
    depth: 50
}}
```

## Testing

### Unit Tests

```bash
cargo test --lib reporting::tests
```

**Result:** 49/49 tests passing

### Test Coverage

- Section parsing (basic, all parameters, invalid inputs)
- Template parsing (empty, single, multiple, duplicate IDs)
- Parameter validation (aggregate-by, depth, filters)
- Error handling (invalid regex, malformed sections)

## Performance Considerations

- Each section generates independent plot data
- Commit data loaded once, filtered multiple times
- Regex compilation happens once per template load
- No significant performance impact vs multiple report runs

## Security Considerations

Implemented as per plan:
- Templates read from repository (trusted source)
- No external URL fetching
- HTML escaping for metadata values (inherited from template system)
- Regex validation to prevent ReDoS
- Maximum section limit (soft: log warning at 10+)

## Known Limitations

1. **Title parameter** - Parsed but not yet used in plot display
2. **No validation** - Doesn't validate that measurements exist before section generation
3. **Empty sections** - Sections with no matching data render empty plots
4. **No section-level errors** - If one section fails, whole report fails (no graceful degradation)

## Future Enhancements

From the plan document, these items remain for future work:

1. **Section title display** - Use `title` parameter for chart customization
2. **Graceful error handling** - Continue with other sections if one fails
3. **Section validation** - Pre-check that measurements exist
4. **Section templates** - Allow sections to reference sub-templates
5. **Dynamic sections** - Generate sections based on available measurements

## Migration Guide

### From CLI Arguments to Template

**Before:**
```bash
git perf report \
  --filter "^test::" \
  --aggregate-by median \
  -n 50 \
  --output report.html
```

**After:**
```html
{{SECTION[tests]
    measurement-filter: ^test::
    aggregate-by: median
    depth: 50
}}
```

```bash
git perf report --template my-template.html --output report.html
```

## Verification Checklist

- [x] Code compiles without errors
- [x] All unit tests pass
- [x] Code formatted with `cargo fmt`
- [x] Clippy warnings addressed
- [x] Documentation created
- [x] Example templates provided
- [x] Backward compatibility maintained
- [x] Phase 2b checklist items completed

## References

- **Plan Document**: `docs/plans/github-pages-integration-and-templating.md`
- **User Documentation**: `docs/dashboard-templates.md`
- **Example Templates**: `.git-perf/templates/`
- **Implementation**: `git_perf/src/reporting.rs`

## Next Steps (Phase 3)

Following the plan, Phase 3 would implement:
- Index generation for reports
- `git perf generate-index` subcommand
- Integration with GitHub Pages workflows

Phase 2b is now **complete** and ready for review.
