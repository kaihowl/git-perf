# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/kaihowl/git-perf/compare/git_perf_cli_types-v0.2.0...git_perf_cli_types-v0.2.1) - 2025-11-15

### Added

- *(config)* add git-perf config command (list, json, validate) ([#451](https://github.com/kaihowl/git-perf/pull/451))
- *(reporting)* support multiple split keys for grouping measurements ([#461](https://github.com/kaihowl/git-perf/pull/461))
- *(cli)* add regex filter for audit and report (OR, anchored) ([#445](https://github.com/kaihowl/git-perf/pull/445))
- *(cli)* size subcommand (phases 1-3 complete) ([#446](https://github.com/kaihowl/git-perf/pull/446))
- *(cli)* allow multiple -m flags for bump-epoch command ([#441](https://github.com/kaihowl/git-perf/pull/441))
- *(cli)* add import command for JUnit XML and Criterion JSON ([#437](https://github.com/kaihowl/git-perf/pull/437))
- *(audit)* unify per-measurement config resolution and CLI precedence ([#405](https://github.com/kaihowl/git-perf/pull/405))

## [0.2.0](https://github.com/kaihowl/git-perf/compare/git_perf_cli_types-v0.1.1...git_perf_cli_types-v0.2.0) - 2025-10-05

### Added

- *(cli)* add optional remote parameter to push command ([#375](https://github.com/kaihowl/git-perf/pull/375))
- merge prune functionality into remove command ([#370](https://github.com/kaihowl/git-perf/pull/370))
- *(cli)* add list-commits command, cleanup orphaned gh-pages reports with safety confirmations, scheduled cleanup workflow, and comprehensive testing & documentation ([#353](https://github.com/kaihowl/git-perf/pull/353))
- [**breaking**] Fix global settings inconsistency ([#292](https://github.com/kaihowl/git-perf/pull/292))
- MAD dispersion method ([#261](https://github.com/kaihowl/git-perf/pull/261))

### Other

- *(cli)* clarify that remove only affects published measurements ([#344](https://github.com/kaihowl/git-perf/pull/344))
- *(cli)* clarify that remove only affects published measurements ([#343](https://github.com/kaihowl/git-perf/pull/343))
- Simplify manpage versioning ([#291](https://github.com/kaihowl/git-perf/pull/291))
- Add MAD dispersion method documentation and improve CLI help ([#272](https://github.com/kaihowl/git-perf/pull/272))

## [0.1.1](https://github.com/kaihowl/git-perf/compare/git_perf_cli_types-v0.1.0...git_perf_cli_types-v0.1.1) - 2025-08-23

### Added

- allow ignoring relative median differences ([#255](https://github.com/kaihowl/git-perf/pull/255))

### Fixed

- ad missing repository key in git_perf_cli_types ([#251](https://github.com/kaihowl/git-perf/pull/251))

## [0.1.0](https://github.com/kaihowl/git-perf/releases/tag/cli_types-v0.1.0) - 2025-08-15

### Added

- support auditing multiple metrics ([#229](https://github.com/kaihowl/git-perf/pull/229))
- generate man page with build ([#156](https://github.com/kaihowl/git-perf/pull/156))

### Fixed

- adapt min_measurements range to current implementation ([#159](https://github.com/kaihowl/git-perf/pull/159))
