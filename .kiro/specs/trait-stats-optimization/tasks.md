# Implementation Plan: Trait Stats Optimization

## Overview

Three independent optimizations to `compute_trait_stats`: O(n) percentile selection, tick-based throttling, and single-pass collection. All changes confined to the viz_bevy module and config layer. Simulation logic untouched.

## Tasks

- [x] 1. Add `stats_update_interval` config field and `StatsTickCounter` resource
  - [x] 1.1 Add `stats_update_interval` field to `BevyExtras` in `src/io/config_file.rs` with serde default of 10, and update `Default` impl
    - _Requirements: 2.1, 2.4, 2.5_
  - [x] 1.2 Add `stats_update_interval` field to `BevyVizConfig` in `src/viz_bevy/resources.rs` and wire it from `BevyExtras` in `src/main.rs`
    - _Requirements: 2.1_
  - [x] 1.3 Add `StatsTickCounter` resource struct to `src/viz_bevy/resources.rs`
    - _Requirements: 2.1_
  - [x] 1.4 Insert `StatsTickCounter` resource during setup in `src/viz_bevy/setup.rs`, reading interval from `BevyVizConfig`
    - _Requirements: 2.1_
  - [ ]* 1.5 Write unit tests for config parsing of `stats_update_interval` (default value, explicit value)
    - _Requirements: 2.4, 2.5_

- [x] 2. Optimize `compute_single_stats` with O(n) selection
  - [x] 2.1 Replace sort-based implementation in `src/viz_bevy/systems.rs` with streaming min/max/mean pass followed by `select_nth_unstable_by` for p25/p50/p75
    - Compute p50 first, then p25 on left partition, then p75 on right partition
    - Handle degenerate cases (n < 4) where percentile indices collapse
    - _Requirements: 1.1, 1.2, 1.3, 1.4_
  - [ ]* 2.2 Write property test: `compute_single_stats` equivalence
    - **Property 1: compute_single_stats equivalence**
    - Generate random Vec<f32> of lengths 1..1000, compare optimized output against sort-based reference
    - Include edge cases: n=1, n=2, n=3, all-identical values
    - **Validates: Requirements 1.1, 1.2, 1.4**

- [x] 3. Optimize `compute_trait_stats_from_actors` with single-pass collection
  - [x] 3.1 Refactor `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs` to pre-allocate 8 Vecs using iterator size hint and collect all trait values in a single loop
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - [ ]* 3.2 Write property test: inert actor exclusion
    - **Property 3: Inert actor exclusion**
    - Generate random actor collections with mixed inert/active status, verify stats reflect only non-inert actors
    - **Validates: Requirements 3.3, 3.4, 4.3**

- [x] 4. Implement throttle gate in `compute_trait_stats`
  - [x] 4.1 Modify `compute_trait_stats` system in `src/viz_bevy/systems.rs` to accept `ResMut<StatsTickCounter>`, increment counter each tick, skip recomputation when `ticks_since_update < interval`, reset on recomputation. Treat interval 0 or 1 as every-tick.
    - _Requirements: 2.2, 2.3, 2.6, 4.4, 5.1, 5.2_
  - [ ]* 4.2 Write property test: throttle gate correctness
    - **Property 2: Throttle gate correctness**
    - Generate random intervals (2..100) and tick sequences, verify recomputation occurs if and only if ticks_since_update >= interval
    - **Validates: Requirements 2.2, 2.3, 2.6**

- [x] 5. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Update documentation
  - [x] 6.1 Add `stats_update_interval` to `example_config.toml` under `[bevy]` with explanatory comment
    - _Requirements: 6.1_
  - [x] 6.2 Update `format_config_info()` in `src/viz_bevy/setup.rs` to display `stats_update_interval` — pass the value through as a parameter or read from a resource
    - _Requirements: 6.2_
  - [x] 6.3 Update `config-documentation.md` steering file to add `stats_update_interval` to the `[bevy]` config reference table
    - _Requirements: 6.1_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- All changes are COLD path — heap allocation is acceptable
- `proptest` is the PBT library for all property tests
- The `SingleTraitStats` and `TraitStats` structs are unchanged — stats panel formatting requires no updates
