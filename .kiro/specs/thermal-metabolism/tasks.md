# Implementation Plan: Thermal Metabolism

## Overview

Add a quadratic thermal performance curve to actor metabolism. Implementation proceeds bottom-up: config fields first, then the heritable trait, then the metabolism HOT path, then genetic distance, then visualization, then documentation. Tests are interleaved with implementation to catch errors early.

## Tasks

- [x] 1. Add thermal metabolism fields to ActorConfig
  - [x] 1.1 Add serde default functions and new fields to `src/grid/actor_config.rs`
    - Add `default_thermal_sensitivity() -> f32 { 0.01 }`, `default_optimal_temp() -> f32 { 0.5 }`, `default_trait_optimal_temp_min() -> f32 { 0.0 }`, `default_trait_optimal_temp_max() -> f32 { 2.0 }`
    - Add `thermal_sensitivity`, `optimal_temp`, `trait_optimal_temp_min`, `trait_optimal_temp_max` fields to `ActorConfig` with `#[serde(default = "...")]` attributes
    - Add the four fields to the `Default` impl
    - _Requirements: 1.4, 1.5, 2.1_

- [x] 2. Add optimal_temp to HeritableTraits
  - [x] 2.1 Add `optimal_temp: f32` field to `HeritableTraits` in `src/grid/actor.rs`
    - Add the field after `kin_tolerance`
    - Update the compile-time size assertion from 36 to 40
    - Update `from_config` to initialize `optimal_temp` from `config.optimal_temp`
    - Update `mutate` to apply proportional gaussian mutation and clamp to `[trait_optimal_temp_min, trait_optimal_temp_max]`
    - _Requirements: 1.1, 1.2, 1.3_

  - [ ]* 2.2 Write property tests for HeritableTraits optimal_temp
    - **Property 1: from_config initializes optimal_temp from config**
    - **Property 2: mutate clamps optimal_temp within configured bounds**
    - **Validates: Requirements 1.2, 1.3**

- [x] 3. Add thermal penalty to run_actor_metabolism
  - [x] 3.1 Update `run_actor_metabolism` signature and implementation in `src/grid/actor_systems.rs`
    - Add `heat_read: &[f32]` parameter after `chemical_write`
    - For active actors: compute `thermal_cost = config.thermal_sensitivity * (heat_read[ci] - actor.traits.optimal_temp).powi(2)` and subtract from energy alongside `base_energy_decay`
    - Inert actors: no change (no thermal penalty)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 3.2 Update call site in `src/grid/tick.rs`
    - In `run_actor_phases`, read `heat_read = grid.read_heat()` before the metabolism block
    - Pass `heat_read` to `run_actor_metabolism`
    - _Requirements: 3.1_

  - [x] 3.3 Update existing unit tests in `src/grid/actor_systems.rs`
    - Add `heat_read` parameter to all existing `run_actor_metabolism` test calls (use a zero-heat buffer or matching optimal_temp to preserve existing test semantics)
    - _Requirements: 3.2_

  - [ ]* 3.4 Write property tests for thermal penalty
    - **Property 3: zero thermal_sensitivity produces zero thermal penalty**
    - **Property 4: thermal penalty follows quadratic formula**
    - **Property 5: inert actors receive no thermal penalty**
    - **Validates: Requirements 2.3, 3.2, 3.3, 3.4**

- [x] 4. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Update genetic distance
  - [x] 5.1 Update `genetic_distance` in `src/grid/actor_systems.rs`
    - Change `TRAIT_COUNT` from 9 to 10
    - Add `(a.optimal_temp, b.optimal_temp, config.trait_optimal_temp_min, config.trait_optimal_temp_max)` to the traits array
    - _Requirements: 4.1, 4.2_

  - [ ]* 5.2 Write property test for genetic_distance with optimal_temp
    - **Property 6: genetic_distance reflects optimal_temp differences**
    - **Validates: Requirements 4.1**

- [x] 6. Update visualization layer
  - [x] 6.1 Update `TraitStats` and `compute_trait_stats_from_actors` in `src/viz_bevy/resources.rs` and `src/viz_bevy/systems.rs`
    - Change `TraitStats.traits` from `[SingleTraitStats; 9]` to `[SingleTraitStats; 10]`
    - Add `optimal_temp` buffer in `compute_trait_stats_from_actors`, collect values, compute stats at index 9
    - _Requirements: 5.1, 5.2_

  - [x] 6.2 Update `TRAIT_NAMES`, `format_actor_info`, and `format_config_info` in `src/viz_bevy/setup.rs`
    - Extend `TRAIT_NAMES` from 9 to 10 entries, appending `"optimal_temp"`
    - Add `optimal_temp` line to `format_actor_info`
    - Add `thermal_sensitivity`, `optimal_temp`, and `trait_optimal_temp` range to `format_config_info`
    - _Requirements: 5.3, 5.4, 5.5, 6.2_

  - [ ]* 6.3 Write property tests for visualization
    - **Property 7: trait stats include optimal_temp statistics**
    - **Property 8: actor inspector displays optimal_temp**
    - **Property 9: config info panel displays thermal metabolism fields**
    - **Validates: Requirements 5.2, 5.4, 6.2**

- [x] 7. Update configuration documentation
  - [x] 7.1 Update `example_config.toml`
    - Add `thermal_sensitivity`, `optimal_temp`, `trait_optimal_temp_min`, `trait_optimal_temp_max` with comments in the `[actor]` section
    - _Requirements: 6.1_

  - [x] 7.2 Update `config-documentation.md` steering file
    - Add all four new ActorConfig fields to the configuration reference table
    - Add `optimal_temp` to the heritable trait list
    - Update `TraitStats.traits` array size from 9 to 10 in the Bevy Runtime Resources section
    - _Requirements: 6.3, 6.4_

- [x] 8. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` with minimum 100 iterations per property
- The thermal penalty computation is pure arithmetic in the HOT path — zero allocations, no branching beyond the existing inert check
- Existing `run_actor_metabolism` unit tests must be updated (task 3.3) before the property tests can run, since the function signature changes
