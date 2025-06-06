# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0](https://github.com/kaihowl/git-perf/compare/v0.15.5...v0.16.0) - 2025-06-01

### Added

- log retry attempts on default level ([#147](https://github.com/kaihowl/git-perf/pull/147))
- better output on pull ([#143](https://github.com/kaihowl/git-perf/pull/143))
- produce useful output on git perf push ([#141](https://github.com/kaihowl/git-perf/pull/141))
- proper ground for logging ([#132](https://github.com/kaihowl/git-perf/pull/132))
- revamp conflict handling and allow full removal of measurements ([#119](https://github.com/kaihowl/git-perf/pull/119))
- readable print for Stats ([#113](https://github.com/kaihowl/git-perf/pull/113))
- check git version before operation ([#102](https://github.com/kaihowl/git-perf/pull/102))
- use tab as delimiter in csv ([#101](https://github.com/kaihowl/git-perf/pull/101))

### Fixed

- setup git user explicitly ([#145](https://github.com/kaihowl/git-perf/pull/145))
- always print stdout / stderr on separate lines ([#142](https://github.com/kaihowl/git-perf/pull/142))
- lifetime warning ([#125](https://github.com/kaihowl/git-perf/pull/125))
- remove unused method ([#126](https://github.com/kaihowl/git-perf/pull/126))
- remove unused struct SerializeMeasurementData ([#128](https://github.com/kaihowl/git-perf/pull/128))
- accept expected perf regression from b935a401 ([#122](https://github.com/kaihowl/git-perf/pull/122))
- report size changed independent of changes ([#121](https://github.com/kaihowl/git-perf/pull/121))
- set CI to true ([#118](https://github.com/kaihowl/git-perf/pull/118))
- correct parsing of measured value in test ([#116](https://github.com/kaihowl/git-perf/pull/116))
- upgrade to macos-latest in test_action ([#115](https://github.com/kaihowl/git-perf/pull/115))

### Other

- assert that references are cleaned up ([#154](https://github.com/kaihowl/git-perf/pull/154))
- rename action files to match their names ([#153](https://github.com/kaihowl/git-perf/pull/153))
- separate upstream ([#149](https://github.com/kaihowl/git-perf/pull/149))
- manual setup of git user name and email ([#152](https://github.com/kaihowl/git-perf/pull/152))
- add todo ([#151](https://github.com/kaihowl/git-perf/pull/151))
- move to proper integration tests ([#150](https://github.com/kaihowl/git-perf/pull/150))
- *(deps)* bump fregante/setup-git-user from 1 to 2 in the github-actions group ([#148](https://github.com/kaihowl/git-perf/pull/148))
- add some notes on limitations ([#146](https://github.com/kaihowl/git-perf/pull/146))
- create GitOutput ([#140](https://github.com/kaihowl/git-perf/pull/140))
- add trace for git exec ([#138](https://github.com/kaihowl/git-perf/pull/138))
- better error messsages ([#137](https://github.com/kaihowl/git-perf/pull/137))
- run bash scripts through cargo ([#134](https://github.com/kaihowl/git-perf/pull/134))
- allow concurrency again ([#135](https://github.com/kaihowl/git-perf/pull/135))
- move libtime installation into github action ([#133](https://github.com/kaihowl/git-perf/pull/133))
- use branch version for git report ([#91](https://github.com/kaihowl/git-perf/pull/91))
- run with a separate concurrency token for each branch for reporting ([#129](https://github.com/kaihowl/git-perf/pull/129))
- add manpages ([#124](https://github.com/kaihowl/git-perf/pull/124))
- *(deps)* bump actions/create-github-app-token from 1 to 2 in the github-actions group ([#123](https://github.com/kaihowl/git-perf/pull/123))
- do not show cargo.lock changes in review ([#120](https://github.com/kaihowl/git-perf/pull/120))
- use better script_dir determination ([#117](https://github.com/kaihowl/git-perf/pull/117))
- *(deps)* bump rlespinasse/github-slug-action from 4 to 5 in the github-actions group ([#112](https://github.com/kaihowl/git-perf/pull/112))
- *(deps)* bump peaceiris/actions-gh-pages from 3 to 4 in the github-actions group ([#111](https://github.com/kaihowl/git-perf/pull/111))
- *(deps)* bump the github-actions group with 3 updates ([#99](https://github.com/kaihowl/git-perf/pull/99))
- bump report measurement epoch ([#108](https://github.com/kaihowl/git-perf/pull/108))
- must measure report command repeatedly for significance ([#107](https://github.com/kaihowl/git-perf/pull/107))
- remove release-binary-size tracking ([#104](https://github.com/kaihowl/git-perf/pull/104))

## [0.15.5](https://github.com/kaihowl/git-perf/compare/v0.15.4...v0.15.5) - 2024-01-22

### Fixed
- disable no-separator option ([#97](https://github.com/kaihowl/git-perf/pull/97))

## [0.15.4](https://github.com/kaihowl/git-perf/compare/v0.15.3...v0.15.4) - 2024-01-20

### Fixed
- install action uses installer ([#88](https://github.com/kaihowl/git-perf/pull/88))

### Other
- add musl artifact ([#89](https://github.com/kaihowl/git-perf/pull/89))

## [0.15.3](https://github.com/kaihowl/git-perf/compare/v0.15.2...v0.15.3) - 2024-01-20

### Other
- upload release artifacts and retain generated release text ([#85](https://github.com/kaihowl/git-perf/pull/85))

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
