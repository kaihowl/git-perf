# Release Process

This document outlines the automated release process for the git-perf project. Our release workflow is fully automated using GitHub Actions, release-plz, and cargo-dist.

## Overview

The release process consists of two main phases:
1. **Continuous Release Tracking** - A continuously pending PR tracks changes for the next release
2. **Release Creation** - When ready, the PR is merged, triggering automated artifact building and release creation

## Release Process Flow

```plantuml
@startuml
!theme plain
skinparam backgroundColor transparent
skinparam sequenceArrowThickness 2
skinparam roundcorner 20

actor Developer
participant "GitHub" as GH
participant "release-plz.yml" as RP
participant "release.yml" as R
participant "cargo-dist" as CD

== Continuous Release Tracking ==

Developer -> GH: Push to master branch
GH -> RP: Trigger release-plz workflow
activate RP

RP -> RP: Generate GitHub token
RP -> RP: Checkout repository
RP -> RP: Install Rust toolchain
RP -> RP: Run release-plz

alt Changes detected
    RP -> GH: Create/update draft PR
    RP -> RP: Update version numbers
    RP -> RP: Generate changelog
    RP -> RP: Update CHANGELOG.md
end

deactivate RP

== Release Creation ==

Developer -> GH: Merge release PR
GH -> RP: Trigger release-plz workflow
activate RP

RP -> RP: Generate GitHub token
RP -> RP: Checkout repository
RP -> RP: Install Rust toolchain
RP -> RP: Run release-plz

RP -> GH: Create git tag (vX.Y.Z)
deactivate RP

GH -> R: Trigger release workflow (on tag)
activate R

R -> R: Plan artifacts (cargo-dist)
R -> R: Build local artifacts\n(platform-specific binaries)
R -> R: Build global artifacts\n(checksums, installers)
R -> R: Host artifacts
R -> GH: Create GitHub release
R -> GH: Upload all artifacts
R -> GH: Set 'latest' tag (if not prerelease)

deactivate R

note right of Developer
  Supported platforms:
  • x86_64-unknown-linux-gnu
  • aarch64-apple-darwin  
  • x86_64-apple-darwin
  • x86_64-unknown-linux-musl
end note

@enduml
```

## Key Components

- **release-plz.yml**: Runs on master branch pushes, creates draft PRs, and generates tags
- **release.yml**: Runs on tag creation, builds artifacts using cargo-dist, and creates GitHub releases
- **cargo-dist**: Handles cross-platform artifact building and distribution

## Tools and Configuration

- **release-plz**: Version management and changelog generation (`.release-plz.toml`)
- **cargo-dist**: Artifact building and distribution (`Cargo.toml` workspace metadata)
- **GitHub Actions**: Automated workflow execution

## Additional Resources

- [release-plz Documentation](https://github.com/MarcoIeni/release-plz)
- [cargo-dist Documentation](https://github.com/axodotdev/cargo-dist)
- [Conventional Commits](https://www.conventionalcommits.org/)
