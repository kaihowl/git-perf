# Release Process

This document outlines the automated release process for the git-perf project. Our release workflow is fully automated using GitHub Actions, release-plz, and cargo-dist to ensure consistent, reliable releases.

## Overview

The release process consists of two main phases:
1. **Continuous Release Tracking** - A continuously pending PR tracks changes for the next release
2. **Release Creation** - When ready, the PR is merged, triggering automated artifact building and release creation

## Continuous Release Tracking

We maintain a continuously pending pull request (PR) that tracks all changes slated for the upcoming release. This PR is automatically updated by release-plz and serves as a central point for review and discussion.

### How It Works

- **Automated Updates**: release-plz runs on every push to the master branch
- **Version Bumping**: Automatically increments version numbers based on conventional commits
- **Changelog Generation**: Updates CHANGELOG.md with new changes
- **Draft PR**: Creates or updates a draft PR with all pending changes

### Benefits

- **Transparency**: All stakeholders can see what's coming in the next release
- **Review Process**: Changes can be reviewed before the actual release
- **Automation**: No manual version management required
- **Consistency**: Ensures all releases follow the same process

## Release Creation Process

When the release PR is ready and approved, the following automated process occurs:

### 1. Merge the Release PR

Once all changes are approved, merge the release PR into the master branch. This triggers the release workflow.

### 2. Tag Creation

release-plz automatically creates a git tag following semantic versioning (e.g., `v0.16.0`).

### 3. Artifact Building

The release workflow builds artifacts for multiple platforms:

- **x86_64-unknown-linux-gnu** - 64-bit Linux (GNU)
- **aarch64-apple-darwin** - Apple Silicon macOS
- **x86_64-apple-darwin** - Intel macOS
- **x86_64-unknown-linux-musl** - 64-bit Linux (musl)

### 4. Release Creation

- Creates a GitHub release with the generated changelog
- Uploads all built artifacts to the release
- Sets the 'latest' tag for non-prerelease versions

## Artifact Building and Distribution

We use cargo-dist to build and distribute release artifacts across multiple platforms.

### Build Process

1. **Planning Phase**: Determines what needs to be built based on the workspace configuration
2. **Local Artifacts**: Builds platform-specific binaries and installers
3. **Global Artifacts**: Creates platform-agnostic artifacts (checksums, universal installers)
4. **Upload**: All artifacts are uploaded to the GitHub release

### Artifact Types

- **Binaries**: Platform-specific executables
- **Installers**: Shell installers for easy installation
- **Checksums**: SHA256 checksums for verification
- **Archives**: Compressed archives for manual installation

### Supported Platforms

| Platform | Target | Description |
|----------|--------|-------------|
| Linux (GNU) | `x86_64-unknown-linux-gnu` | Standard 64-bit Linux with GNU libc |
| Linux (musl) | `x86_64-unknown-linux-musl` | 64-bit Linux with musl libc (Alpine) |
| macOS (Intel) | `x86_64-apple-darwin` | Intel-based macOS |
| macOS (Apple Silicon) | `aarch64-apple-darwin` | Apple Silicon (M1/M2) macOS |

## Draft Releases

Draft releases are unpublished releases that allow for final reviews and adjustments before public availability.

### Purpose

- **Quality Assurance**: Final verification of artifacts and release notes
- **Manual Review**: Allows maintainers to review before publishing
- **Rollback Capability**: Can be easily modified or deleted if issues are found

### Configuration

Draft releases are enabled by default in `.release-plz.toml`:
```toml
[workspace]
git_release_draft=true
```

### Publishing

Once verified, draft releases can be published manually through the GitHub web interface.

## Tools and Configuration

### release-plz

- **Purpose**: Automates version management and changelog generation
- **Configuration**: `.release-plz.toml`
- **Workflow**: `.github/workflows/release-plz.yml`

### cargo-dist

- **Purpose**: Builds and distributes artifacts across platforms
- **Configuration**: `Cargo.toml` workspace metadata
- **Workflow**: `.github/workflows/release.yml`

### GitHub Actions

- **release-plz.yml**: Handles continuous release tracking
- **release.yml**: Handles artifact building and release creation
- **ci.yml**: Ensures code quality before release

## Troubleshooting

### Common Issues

#### Release PR Not Created

**Symptoms**: No draft PR appears after pushing to master

**Possible Causes**:
- Missing GitHub App token configuration
- release-plz workflow failed
- No conventional commits detected

**Solutions**:
1. Check GitHub Actions for workflow failures
2. Verify APP_ID and APP_PRIVATE_KEY secrets are configured
3. Ensure commits follow conventional commit format

#### Artifact Build Failures

**Symptoms**: Release workflow fails during artifact building

**Possible Causes**:
- Platform-specific build issues
- Dependency problems
- Resource constraints

**Solutions**:
1. Check build logs for specific error messages
2. Verify all dependencies are available
3. Ensure sufficient GitHub Actions minutes

#### Release Not Created

**Symptoms**: Tag created but no GitHub release appears

**Possible Causes**:
- Draft release configuration issues
- Permission problems
- Workflow failures

**Solutions**:
1. Check release workflow logs
2. Verify GITHUB_TOKEN permissions
3. Manually create release if needed

#### Artifact Upload Failures

**Symptoms**: Release created but artifacts missing

**Possible Causes**:
- Network issues during upload
- File size limits
- Permission problems

**Solutions**:
1. Check upload step logs
2. Verify artifact files exist
3. Retry the workflow if needed

### Debugging Steps

1. **Check Workflow Logs**: Review GitHub Actions logs for specific error messages
2. **Verify Configuration**: Ensure all configuration files are correct
3. **Test Locally**: Use cargo-dist locally to test builds
4. **Check Permissions**: Verify GitHub token permissions

### Manual Recovery

If automated release fails:

1. **Manual Tag**: Create git tag manually if needed
2. **Manual Release**: Create GitHub release manually
3. **Manual Artifacts**: Build and upload artifacts manually
4. **Update Changelog**: Ensure CHANGELOG.md is up to date

## Best Practices

### Before Release

- Review the release PR thoroughly
- Ensure all tests pass
- Verify changelog accuracy
- Check for any breaking changes

### During Release

- Monitor the release workflow
- Verify artifacts are uploaded correctly
- Test installation on target platforms
- Review the final release notes

### After Release

- Verify the 'latest' tag is set correctly
- Test the release on different platforms
- Monitor for any issues reported by users
- Update documentation if needed

## Additional Resources

- [release-plz Documentation](https://github.com/MarcoIeni/release-plz)
- [cargo-dist Documentation](https://github.com/axodotdev/cargo-dist)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Conventional Commits](https://www.conventionalcommits.org/)
