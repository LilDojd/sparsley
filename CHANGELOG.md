# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/LilDojd/sparsley/releases/tag/v0.1.0) - 2026-10-04

### Added

- add SparseMap with core operations
- add map iterators and entry API
- implement std traits and positional access for SparseMap
- add SparseSet
- add SparseMap::sort_unstable_by
- add SparseSet::sort_unstable
- add swap_remove_index and swap_indices
- *(diagram)* add sparsley-diagram crate with trace! macro
- add get_key_value and sort_unstable_keys
- add set positions, superset, intersection and difference
- add optional serde support
- *(diagram)* draw diagrams in a dependency-free proc macro

### Changed

- encode dense indices in a Slot type
- simplify dense layout helpers
- [**breaking**] rename Key::index to slot and position to get_index_of
- give SparseSet its own IntoIter
- name dense indices and key slots consistently
- spell out the serde preallocation limit

### Documentation

- document crate layout and limits
- add examples to core methods
- generate sparsley crate layout diagrams
- regenerate diagrams with framed changes
- diagram insert, remove, clear and retain on map and set
- align crate docs with slot and index naming
- add search aliases
- generate method and crate diagrams with diagram!
- add README
- drop the README layout section and shrink the logo

### Fixed

- keep maps consistent when clone_from panics
- cap extend reservation and make Drain covariant

### Performance

- store keys and values in a single allocation
- allocate zeroed sparse slots and rebuild them from live keys
- add call-free insert fast path
- write fresh sparse pages without reading them first
- test for empty slots before bounds
- keep chunk flags off removal and membership paths
- grow dense storage in place with realloc
- skip chunk flags below a flagged boundary
- repair the moved slot without reloading its key
