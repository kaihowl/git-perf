# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.17.0](https://github.com/kaihowl/git-perf/compare/git-perf-v0.16.0...git-perf-v0.17.0) - 2025-08-06

### Added

- add z-score and direction decorator ([#226](https://github.com/kaihowl/git-perf/pull/226))
- add new GitError variant for empty or never pushed remote ([#175](https://github.com/kaihowl/git-perf/pull/175))
- generate man page with build ([#156](https://github.com/kaihowl/git-perf/pull/156))

### Fixed

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
