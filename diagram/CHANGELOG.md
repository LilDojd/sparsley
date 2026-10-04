# Changelog: sparsley-diagram

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/LilDojd/sparsley/releases/tag/sparsley-diagram-v0.1.0) - 2026-10-04

### Added

- *(diagram)* add sparsley-diagram crate with trace! macro
- *(diagram)* draw arrays as tables
- *(diagram)* frame changed cells and link slots to positions
- *(diagram)* draw SparseSet through an Observe trait
- *(diagram)* draw diagrams in a dependency-free proc macro
- *(diagram)* say dense index instead of position
- *(diagram)* model get_key_value, entry or_insert and set index calls

### Changed

- *(diagram)* model slots, cells and changes as types
- [**breaking**] rename Key::index to slot and position to get_index_of
