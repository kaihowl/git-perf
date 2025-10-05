# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.0](https://github.com/kaihowl/git-perf/compare/git-perf-v0.17.2...git-perf-v0.18.0) - 2025-10-05

### Added

- *(cli)* add optional remote parameter to push command ([#375](https://github.com/kaihowl/git-perf/pull/375))
- merge prune functionality into remove command ([#370](https://github.com/kaihowl/git-perf/pull/370))
- *(cli)* add list-commits command, cleanup orphaned gh-pages reports with safety confirmations, scheduled cleanup workflow, and comprehensive testing & documentation ([#353](https://github.com/kaihowl/git-perf/pull/353))
- add support for git 2.43.0 by using git symbolic-ref ([#355](https://github.com/kaihowl/git-perf/pull/355))
- [**breaking**] Fix global settings inconsistency ([#292](https://github.com/kaihowl/git-perf/pull/292))
- implement summarize measurement for csv  ([#276](https://github.com/kaihowl/git-perf/pull/276))
- MAD dispersion method ([#261](https://github.com/kaihowl/git-perf/pull/261))

### Fixed

- *(reporting)* improve error message when no commits are found and add test for empty repository ([#348](https://github.com/kaihowl/git-perf/pull/348))
- *(config)* ensure non-inline tables are created in empty config files ([#306](https://github.com/kaihowl/git-perf/pull/306))
- *(audit)* add measurement name header for insufficient data messages and include tests ([#300](https://github.com/kaihowl/git-perf/pull/300))
- only show finite z-scores ([#259](https://github.com/kaihowl/git-perf/pull/259))

### Other

- *(mutation)* add tests for critical missed mutants ([#392](https://github.com/kaihowl/git-perf/pull/392))
- resolve clippy warnings for code quality improvements ([#383](https://github.com/kaihowl/git-perf/pull/383))
- *(git)* move concurrent modification error handling to push retry logic ([#377](https://github.com/kaihowl/git-perf/pull/377))
- remove outdated TODO comment in update_read_branch function ([#376](https://github.com/kaihowl/git-perf/pull/376))
- remove outdated TODO comment in reporting module ([#374](https://github.com/kaihowl/git-perf/pull/374))
- *(filter)* express measurement filters using subset relation semantics ([#372](https://github.com/kaihowl/git-perf/pull/372))
- *(report)* add comprehensive output validation and improved existing tests ([#368](https://github.com/kaihowl/git-perf/pull/368))
- *(git_interop)* improve authorization header verification in test_customheader_pull ([#365](https://github.com/kaihowl/git-perf/pull/365))
- *(reporting,serialization)* add comprehensive unit tests for coverage improvement ([#354](https://github.com/kaihowl/git-perf/pull/354))
- *(git)* move get_repository_root from high-level to low-level module ([#340](https://github.com/kaihowl/git-perf/pull/340))
- *(reporting)* replace TODO with explanatory comment for axis reversal ([#338](https://github.com/kaihowl/git-perf/pull/338))
- *(data)* extract key-value filtering logic into reusable utility method ([#337](https://github.com/kaihowl/git-perf/pull/337))
- replace TODO comments with explanatory NOTE comments ([#336](https://github.com/kaihowl/git-perf/pull/336))
- *(stats)* move sigma threshold check to is_significant method and add boundary tests ([#335](https://github.com/kaihowl/git-perf/pull/335))
- *(audit)* improve test clarity for relative deviation calculation  ([#334](https://github.com/kaihowl/git-perf/pull/334))
- *(audit)* improve test coverage and add core audit logic tests ([#316](https://github.com/kaihowl/git-perf/pull/316))
- *(reporting)* clarify axis reversal limitation in plotly-rs 0.8.3 ([#317](https://github.com/kaihowl/git-perf/pull/317))
- add weekly mutation testing with comprehensive coverage ([#299](https://github.com/kaihowl/git-perf/pull/299))
- *(data)* centralize data structures by moving Commit to data.rs ([#313](https://github.com/kaihowl/git-perf/pull/313))
- *(git_interop)* abstract common git notes operations into execute_notes_operation ([#312](https://github.com/kaihowl/git-perf/pull/312))
- remove outdated TODO #96 comment about no-separator option ([#308](https://github.com/kaihowl/git-perf/pull/308))
- *(git)* update minimum git version to 2.46.0 with explanation ([#307](https://github.com/kaihowl/git-perf/pull/307))
- Remove deprecated TODOs and clean up test scripts ([#295](https://github.com/kaihowl/git-perf/pull/295))
- *(config)* implement hierarchical configuration system ([#287](https://github.com/kaihowl/git-perf/pull/287))
- Simplify manpage versioning ([#291](https://github.com/kaihowl/git-perf/pull/291))
- migrate to nextest ([#280](https://github.com/kaihowl/git-perf/pull/280))
- proper version check for manpage docs ([#286](https://github.com/kaihowl/git-perf/pull/286))
- MAD documentation ([#263](https://github.com/kaihowl/git-perf/pull/263))
- manpage generation with clap_markdown ([#269](https://github.com/kaihowl/git-perf/pull/269))
- *(cli)* clarify that remove only affects published measurements ([#344](https://github.com/kaihowl/git-perf/pull/344))
- *(cli)* clarify that remove only affects published measurements ([#343](https://github.com/kaihowl/git-perf/pull/343))
- Add MAD dispersion method documentation and improve CLI help ([#272](https://github.com/kaihowl/git-perf/pull/272))

## [0.17.2](https://github.com/kaihowl/git-perf/compare/git-perf-v0.17.1...git-perf-v0.17.2) - 2025-08-23

### Added

- allow ignoring relative median differences ([#255](https://github.com/kaihowl/git-perf/pull/255))

### Fixed

- ad missing repository key in git_perf_cli_types ([#251](https://github.com/kaihowl/git-perf/pull/251))

## [0.17.1](https://github.com/kaihowl/git-perf/compare/git-perf-v0.17.0...git-perf-v0.17.1) - 2025-08-18

### Fixed

- retry on bad object ([#250](https://github.com/kaihowl/git-perf/pull/250))

## [0.17.0](https://github.com/kaihowl/git-perf/compare/git-perf-v0.16.0...git-perf-v0.17.0) - 2025-08-15

### Added

- shorten audit output and 100% more emojis ([#240](https://github.com/kaihowl/git-perf/pull/240))
- hack some ascii sparklines in the output ([#233](https://github.com/kaihowl/git-perf/pull/233))
- support auditing multiple metrics ([#229](https://github.com/kaihowl/git-perf/pull/229))
- add z-score and direction decorator ([#226](https://github.com/kaihowl/git-perf/pull/226))
- add new GitError variant for empty or never pushed remote ([#175](https://github.com/kaihowl/git-perf/pull/175))
- generate man page with build ([#156](https://github.com/kaihowl/git-perf/pull/156))

### Fixed

- remove explicit fetch output ([#238](https://github.com/kaihowl/git-perf/pull/238))
- improve locale handling in compatibility tests ([#237](https://github.com/kaihowl/git-perf/pull/237))
- handle equality for comparison of mean correctly ([#239](https://github.com/kaihowl/git-perf/pull/239))
- properly handle concurrent fetches ([#234](https://github.com/kaihowl/git-perf/pull/234))
- typo in context error message ([#230](https://github.com/kaihowl/git-perf/pull/230))
- do not run auto maintenance when invoking git commands ([#228](https://github.com/kaihowl/git-perf/pull/228))
- use temporary read references ([#217](https://github.com/kaihowl/git-perf/pull/217))
- revert "do not use git 2.46's symref-create for broader compatibility ([#207](https://github.com/kaihowl/git-perf/pull/207))" ([#212](https://github.com/kaihowl/git-perf/pull/212))
- min version needed is 2.45.0 for git ([#208](https://github.com/kaihowl/git-perf/pull/208))
- do not use git 2.46's symref-create for broader compatibility ([#207](https://github.com/kaihowl/git-perf/pull/207))
- use proper origin for git-perf ([#203](https://github.com/kaihowl/git-perf/pull/203))
- clean up badly reviewed, cursor-generated PR fallout ([#202](https://github.com/kaihowl/git-perf/pull/202))
- address clippy warnings ([#158](https://github.com/kaihowl/git-perf/pull/158))

### Other

- remove outdated todos ([#221](https://github.com/kaihowl/git-perf/pull/221))
- use consistent lifetime annotations ([#225](https://github.com/kaihowl/git-perf/pull/225))
- replace TODO by documentation ([#222](https://github.com/kaihowl/git-perf/pull/222))
- improve walk_commits implementation ([#219](https://github.com/kaihowl/git-perf/pull/219))
- remove outdated todos ([#218](https://github.com/kaihowl/git-perf/pull/218))
- remove outdated todo ([#216](https://github.com/kaihowl/git-perf/pull/216))
- remove unnecessary emptiness check ([#215](https://github.com/kaihowl/git-perf/pull/215))
- add TODOs for documentation and config improvements ([#204](https://github.com/kaihowl/git-perf/pull/204))
- remove outdated TODOs ([#201](https://github.com/kaihowl/git-perf/pull/201))
- remove outdated TODO comments in git_push_notes_ref function ([#200](https://github.com/kaihowl/git-perf/pull/200))
- remove outdated TODO comment in raw_push function ([#198](https://github.com/kaihowl/git-perf/pull/198))
- improve error messages for commit header retrieval in walk_commits function ([#197](https://github.com/kaihowl/git-perf/pull/197))
- silence noisy test output ([#195](https://github.com/kaihowl/git-perf/pull/195))
- split git module into high and low level ([#194](https://github.com/kaihowl/git-perf/pull/194))
- enhance read_config_from_file function signature ([#193](https://github.com/kaihowl/git-perf/pull/193))
- update comment in raw_push to clarify fetch behavior ([#191](https://github.com/kaihowl/git-perf/pull/191))
- simplify temporary reference creation by introducing helper function ([#190](https://github.com/kaihowl/git-perf/pull/190))
- remove outdated TODO comments in new_symbolic_write_ref ([#189](https://github.com/kaihowl/git-perf/pull/189))
- remove outdated TODO comment in raw_add_note_line_to_head ([#186](https://github.com/kaihowl/git-perf/pull/186))
- remove unnecessary explicit drop ([#187](https://github.com/kaihowl/git-perf/pull/187))
- consolidate temporary reference creation functions ([#183](https://github.com/kaihowl/git-perf/pull/183))
- implement configurable backoff policy for git operations ([#182](https://github.com/kaihowl/git-perf/pull/182))
- rename ReductionFuncIterator to MeasurementReducer for clarity ([#181](https://github.com/kaihowl/git-perf/pull/181))
- clean up and clarify outdated TODOs ([#180](https://github.com/kaihowl/git-perf/pull/180))
- enhance measurement retrieval with epoch filtering ([#179](https://github.com/kaihowl/git-perf/pull/179))
- remove outdated todo ([#178](https://github.com/kaihowl/git-perf/pull/178))
- improve error handling in config file operations ([#177](https://github.com/kaihowl/git-perf/pull/177))
- improve write_config function to return Result ([#176](https://github.com/kaihowl/git-perf/pull/176))
- add documentation for performance measurement constants in git_interop.rs ([#174](https://github.com/kaihowl/git-perf/pull/174))
- update environment variables for git command execution ([#173](https://github.com/kaihowl/git-perf/pull/173))
- remove outdated TODO comment in audit function ([#172](https://github.com/kaihowl/git-perf/pull/172))
- remove outdated TODO comments in measurement storage ([#168](https://github.com/kaihowl/git-perf/pull/168))
- remove outdated TODO comment in audit function ([#169](https://github.com/kaihowl/git-perf/pull/169))
- enhance error handling for system time retrieval in measurement storage ([#166](https://github.com/kaihowl/git-perf/pull/166))
- optimize MeasurementData creation in add_multiple function ([#167](https://github.com/kaihowl/git-perf/pull/167))
- remove TODO comment and add epoch parsing tests ([#160](https://github.com/kaihowl/git-perf/pull/160))
- clean up build script by removing unnecessary print statements ([#162](https://github.com/kaihowl/git-perf/pull/162))
- use std::sync::Once for one-time warning about duplicate keys ([#164](https://github.com/kaihowl/git-perf/pull/164))
- remove commented-out TODO in deserialize_single function ([#163](https://github.com/kaihowl/git-perf/pull/163))
