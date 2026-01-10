# Issue: Optimize status command to reuse git log

**Status:** Pending GitHub issue creation
**Source:** PR #589 open comment
**Priority:** Performance enhancement
**Labels:** enhancement, performance

## Context

In PR #589 (feat: add status and reset commands), there's a noted inefficiency in `status.rs` regarding individual commit processing. This was identified during code review on January 3, 2026.

## Current Implementation

The current implementation in `git_perf/src/status.rs` creates a new process for each commit when processing status information. This approach works correctly but has unnecessary overhead, especially when dealing with repositories that have many commits with pending measurements.

## Proposed Improvement

Instead of creating a new process for each commit, the implementation should reuse the git log command to batch-process commits more efficiently. This would involve:

1. Using a single git log invocation to retrieve information for multiple commits
2. Parsing the output to extract the necessary data for each commit
3. Reducing the number of subprocess spawns significantly

## Expected Benefits

- **Reduced process creation overhead**: Fewer fork/exec system calls
- **Better performance**: Noticeable improvement when dealing with many commits
- **More efficient resource utilization**: Lower CPU and memory usage
- **Improved scalability**: Better handling of repositories with extensive pending measurements

## Implementation Considerations

- Ensure the batch processing maintains the same accuracy as the current per-commit approach
- Consider edge cases where commits might have unusual metadata
- Maintain compatibility with the existing `--detailed` flag functionality
- Add performance benchmarks to validate the improvement

## Reference

- **Source PR**: #589 (feat: add status and reset commands)
- **Comment Date**: January 3, 2026
- **Related File**: `git_perf/src/status.rs`
- **Reviewer Comment**: "Instead of creating a new process for each commit, we should...reuse the git log command"

## GitHub Issue Creation Command

```bash
gh issue create \
  --repo kaihowl/git-perf \
  --title "perf(status): reuse git log command instead of creating process per commit" \
  --label "enhancement" \
  --label "performance" \
  --body "$(cat <<'EOF'
## Context

In PR #589, there's a noted inefficiency in `status.rs` regarding individual commit processing.

## Current Implementation

The current implementation creates a new process for each commit when processing status information.

## Proposed Improvement

Instead of creating a new process for each commit, we should reuse the git log command to improve performance and reduce overhead.

## Benefits

- Reduced process creation overhead
- Better performance when dealing with many commits
- More efficient resource utilization

## Reference

- Original comment from PR #589 (Jan 3, 2026)
- Related file: `git_perf/src/status.rs`
EOF
)"
```

## Next Steps

1. Create the GitHub issue using the command above or through the GitHub web interface
2. Analyze the current implementation in `git_perf/src/status.rs` to identify specific optimization points
3. Design the batch processing approach
4. Implement the changes
5. Add performance tests to validate improvements
6. Update PR #589 or create a new PR with the optimization
