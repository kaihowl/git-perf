# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/kaihowl/git-perf/compare/git_perf_cli_types-v0.3.0...git_perf_cli_types-v0.4.0) - 2026-03-01

### Added

- add min_absolute_deviation ([#647](https://github.com/kaihowl/git-perf/pull/647))
- *(cli)* add status and reset commands ([#589](https://github.com/kaihowl/git-perf/pull/589))
- *(audit)* show aggregation method in audit output ([#618](https://github.com/kaihowl/git-perf/pull/618))

### Other

- *(deps)* bump the cargo-dependencies group with 4 updates ([#637](https://github.com/kaihowl/git-perf/pull/637))
- *(deps)* bump the cargo-dependencies group with 2 updates ([#628](https://github.com/kaihowl/git-perf/pull/628))

## [0.3.0](https://github.com/kaihowl/git-perf/compare/git_perf_cli_types-v0.2.1...git_perf_cli_types-v0.3.0) - 2025-12-30

### Added

- committish support, ImportOptions, and non-head docs ([#545](https://github.com/kaihowl/git-perf/pull/545))
- rename detect-changes to show-changes ([#549](https://github.com/kaihowl/git-perf/pull/549))
- *(change_point)* add PELT detection with epochs, enrichment, CLI, and visualization ([#474](https://github.com/kaihowl/git-perf/pull/474))
- *(reporting)* add HTML template support for customizable reports ([#502](https://github.com/kaihowl/git-perf/pull/502))

### Fixed

- *(ci)* resolve 500 error and add --dry-run flag ([#501](https://github.com/kaihowl/git-perf/pull/501))

### Other

- *(git_perf)* add zero min_measurements audit tests; fix NaN display in stats ([#538](https://github.com/kaihowl/git-perf/pull/538))
- *(clippy)* drop index_slicing lint; add must_use, pattern matching ([#525](https://github.com/kaihowl/git-perf/pull/525))
- clarify minimum measurement requirement ([#489](https://github.com/kaihowl/git-perf/pull/489))

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
