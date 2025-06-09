# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.17.0](https://github.com/kaihowl/git-perf/compare/git-perf-v0.16.0...git-perf-v0.17.0) - 2025-06-09

### Added

- generate man page with build ([#156](https://github.com/kaihowl/git-perf/pull/156))

### Fixed

- address clippy warnings ([#158](https://github.com/kaihowl/git-perf/pull/158))

### Other

- remove outdated TODO comment in audit function ([#169](https://github.com/kaihowl/git-perf/pull/169))
- enhance error handling for system time retrieval in measurement storage ([#166](https://github.com/kaihowl/git-perf/pull/166))
- optimize MeasurementData creation in add_multiple function ([#167](https://github.com/kaihowl/git-perf/pull/167))
- remove TODO comment and add epoch parsing tests ([#160](https://github.com/kaihowl/git-perf/pull/160))
- clean up build script by removing unnecessary print statements ([#162](https://github.com/kaihowl/git-perf/pull/162))
- use std::sync::Once for one-time warning about duplicate keys ([#164](https://github.com/kaihowl/git-perf/pull/164))
- remove commented-out TODO in deserialize_single function ([#163](https://github.com/kaihowl/git-perf/pull/163))
