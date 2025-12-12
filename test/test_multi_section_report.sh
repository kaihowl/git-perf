#!/bin/bash

set -e
set -x

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

echo "Multi-section template report generation"
cd_temp_repo

# Add some test measurements
git perf add -m test-measure1 100.0
git perf add -m test-measure2 200.0
git perf add -m bench-measure1 150.0
git perf add -m bench-measure2 250.0

# Create a multi-section template
cat > multi-section-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>{{TITLE}}</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    <h1>{{TITLE}}</h1>
    <p>Generated: {{TIMESTAMP}}</p>
    <p>Commits: {{COMMIT_RANGE}} ({{DEPTH}} total)</p>

    <h2>Test Measurements</h2>
    {{SECTION[test-section]
        measurement-filter: ^test-
        aggregate-by: median
    }}

    <h2>Benchmark Measurements</h2>
    {{SECTION[bench-section]
        measurement-filter: ^bench-
        aggregate-by: mean
    }}
</body>
</html>
EOF

# Generate multi-section report
git perf report --template multi-section-template.html -o multi-section-report.html

# Verify report was created
if [[ ! -f multi-section-report.html ]]; then
  echo "Multi-section report file not created"
  exit 1
fi

# Read the report content
report_content=$(cat multi-section-report.html)

# Verify basic structure
assert_output_contains "$report_content" "<!DOCTYPE html>" "Missing DOCTYPE"
assert_output_contains "$report_content" "<html>" "Missing html tag"
assert_output_contains "$report_content" "Test Measurements" "Missing test section heading"
assert_output_contains "$report_content" "Benchmark Measurements" "Missing bench section heading"

# Verify Plotly content is present (multiple plots for multiple sections)
assert_output_contains "$report_content" "Plotly.newPlot" "Missing Plotly plot"

# Verify template placeholders were replaced
assert_output_not_contains "$report_content" "{{TITLE}}" "Title placeholder not replaced"
assert_output_not_contains "$report_content" "{{TIMESTAMP}}" "Timestamp placeholder not replaced"
assert_output_not_contains "$report_content" "{{SECTION[test-section]" "Section placeholder not replaced"
assert_output_not_contains "$report_content" "{{SECTION[bench-section]" "Section placeholder not replaced"

# Count Plotly.newPlot occurrences to verify we have 2 sections
plot_count=$(grep -o "Plotly.newPlot" multi-section-report.html | wc -l)
if [[ $plot_count -lt 2 ]]; then
  echo "Expected at least 2 plots (one per section), found $plot_count"
  exit 1
fi

echo "Multi-section with depth override"
cd_temp_repo

# Add more commits and measurements
create_commit
git perf add -m test-depth 300.0
create_commit
git perf add -m test-depth 400.0

# Create template with depth override
cat > depth-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Depth Test</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    <h1>Depth Override Test</h1>

    <h2>Last 2 Commits</h2>
    {{SECTION[recent]
        measurement-filter: ^test-depth$
        aggregate-by: none
        depth: 2
    }}
</body>
</html>
EOF

git perf report --template depth-template.html -n 10 -o depth-report.html

# Verify report was created
if [[ ! -f depth-report.html ]]; then
  echo "Depth report file not created"
  exit 1
fi

echo "Multi-section with separate-by parameter"
cd_temp_repo

# Add measurements with metadata
git perf add -m platform-test 100.0 --key-value os=linux --key-value arch=x64
git perf add -m platform-test 120.0 --key-value os=linux --key-value arch=arm64
git perf add -m platform-test 110.0 --key-value os=macos --key-value arch=x64

# Create template with separate-by
cat > separate-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Platform Comparison</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    <h1>Platform Comparison</h1>

    <h2>By OS and Architecture</h2>
    {{SECTION[platform-split]
        measurement-filter: ^platform-test$
        separate-by: os,arch
        aggregate-by: median
    }}
</body>
</html>
EOF

git perf report --template separate-template.html -o separate-report.html

# Verify report was created
if [[ ! -f separate-report.html ]]; then
  echo "Separate-by report file not created"
  exit 1
fi

# Verify multiple traces (one per os/arch combination)
separate_content=$(cat separate-report.html)
assert_output_contains "$separate_content" "Plotly.newPlot" "Missing Plotly plot in separate-by report"

echo "Duplicate section ID should fail"
cd_temp_repo

# Create template with duplicate section IDs
cat > duplicate-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Duplicate Test</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    {{SECTION[dup]
        measurement-filter: ^test
    }}

    {{SECTION[dup]
        measurement-filter: ^bench
    }}
</body>
</html>
EOF

# This should fail with duplicate section ID error
output=$(git perf report --template duplicate-template.html -o dup-report.html 2>&1) && exit 1
assert_output_contains "$output" "Duplicate section ID" "Missing duplicate section ID error"

echo "Invalid section parameter should be warned"
cd_temp_repo

# Create template with unknown parameter (should warn but not fail)
cat > warning-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Warning Test</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    {{SECTION[test]
        measurement-filter: ^test
        unknown-param: value
    }}
</body>
</html>
EOF

# Should succeed but may log warning
git perf report --template warning-template.html -o warning-report.html

if [[ ! -f warning-report.html ]]; then
  echo "Warning template report should have been created despite unknown parameter"
  exit 1
fi

echo "CLI arguments ignored with multi-section template"
cd_temp_repo

# Add measurement
git perf add -m cli-test 100.0

# Create multi-section template
cat > cli-ignore-template.html <<'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>CLI Ignore Test</title>
    {{PLOTLY_HEAD}}
</head>
<body>
    {{SECTION[section1]
        measurement-filter: ^cli-test$
    }}
</body>
</html>
EOF

# Use --filter on command line (should be ignored)
git perf report --template cli-ignore-template.html --filter "^other" -o cli-ignore-report.html

# Verify report was created (filter was ignored, template filter used)
if [[ ! -f cli-ignore-report.html ]]; then
  echo "CLI ignore report file not created"
  exit 1
fi

cli_content=$(cat cli-ignore-report.html)
# Should contain the plot since template filter matches cli-test
assert_output_contains "$cli_content" "Plotly.newPlot" "Template should use its own filter, not CLI filter"

exit 0
