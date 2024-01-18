# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.15.2](https://github.com/kaihowl/git-perf/compare/v0.15.1...v0.15.2) - 2024-01-18

### Other
- release flow with cargo-dist ([#82](https://github.com/kaihowl/git-perf/pull/82))
- use branch name when not running on a PR ([#81](https://github.com/kaihowl/git-perf/pull/81))
- release as draft by default ([#79](https://github.com/kaihowl/git-perf/pull/79))

## [0.15.1](https://github.com/kaihowl/git-perf/compare/v0.15.0...v0.15.1) - 2024-01-14

### Added
- cap retries at total max time of 60 seconds ([#74](https://github.com/kaihowl/git-perf/pull/74))

### Fixed
- ensure that test commits are unique ([#73](https://github.com/kaihowl/git-perf/pull/73))

### Other
- make use of the new app token ([#76](https://github.com/kaihowl/git-perf/pull/76))
- allow release-plz to run on pull_request actions ([#75](https://github.com/kaihowl/git-perf/pull/75))
- *(deps)* bump the github-actions group with 3 updates ([#71](https://github.com/kaihowl/git-perf/pull/71))
- give dependabot group meaningful name ([#70](https://github.com/kaihowl/git-perf/pull/70))
- fixup grouping of github action updates ([#68](https://github.com/kaihowl/git-perf/pull/68))
- group dependency updates ([#64](https://github.com/kaihowl/git-perf/pull/64))
- add pr permission ([#67](https://github.com/kaihowl/git-perf/pull/67))
- set explicit permissions for git-perf operation ([#66](https://github.com/kaihowl/git-perf/pull/66))
- keep actions up to date weekly ([#60](https://github.com/kaihowl/git-perf/pull/60))
- add release-plz action ([#58](https://github.com/kaihowl/git-perf/pull/58))
- add report action ([#57](https://github.com/kaihowl/git-perf/pull/57))
- add git-perf install action ([#54](https://github.com/kaihowl/git-perf/pull/54))
- make version tags start with 'v' ([#52](https://github.com/kaihowl/git-perf/pull/52))
- minimum of 10 measurements needed ([#53](https://github.com/kaihowl/git-perf/pull/53))
