# Implementation Plan: New Heritable Traits

## Overview

Promote `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` from global `ActorConfig` values to per-actor heritable traits. Follows the established mutation/clamping pattern. Updates span the core data model, three system functions, config validation, visualization, and documentation.

## Tasks

- [-] 1. Extend HeritableTraits struct and ActorConfig
  - [ ] 1.1 Add three new fields to `HeritableTraits` in `src/grid/actor.rs`
    - Add `max_tumble_steps: u16`, `reproduction_cost: f32`, `offspring_energy: f32` after the existing four fields
    - Update the compile-time size assert from `== 16` to `== 28`
    - Update `from_config` to initialize the three new fields from `ActorConfig`
    - Update `mutate` to apply gaussian noise + clamping for all three new fields, with `max_tumble_steps` mutated in `f32` space then rounded and cast to `u16`
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 2.5_

  - [ ] 1.2 Add six new clamp bound fields to `ActorConfig` in `src/grid/actor_config.rs`
    - Add `trait_max_tumble_steps_min: u16` (default 1), `trait_max_tumble_steps_max: u16` (default 50)
    - Add `trait_reproduction_cost_min: f32` (default 0.1), `trait_reproduction_cost_max: f32` (default 100.0)
    - Add `trait_offspring_energy_min: f32` (default 0.1), `trait_offspring_energy_max: f32` (default 100.0)
    - Add serde default functions for each new field
    - Update `Default` impl to include the six new fields
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ] 1.3 Add config validation rules in `src/io/config_file.rs`
    - `trait_max_tumble_steps_min >= 1`
    - `trait_max_tumble_steps_min < trait_max_tumble_steps_max`
    - `trait_reproduction_cost_min > 0.0` and `< trait_reproduction_cost_max`
    - `trait_offspring_energy_min > 0.0` and `< trait_offspring_energy_max`
    - `trait_offspring_energy_max <= max_energy`
    - Default `max_tumble_steps` within `[trait_max_tumble_steps_min, trait_max_tumble_steps_max]`
    - Default `reproduction_cost` within `[trait_reproduction_cost_min, trait_reproduction_cost_max]`
    - Default `offspring_energy` within `[trait_offspring_energy_min, trait_offspring_energy_max]`
    - _Requirements: 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11_

  - [ ]* 1.4 Write property test: from_config initializes all seven traits
    - **Property 1: from_config initializes all seven traits from config defaults**
    - **Validates: Requirements 1.2**

  - [ ]* 1.5 Write property test: mutation clamps all seven traits
    - **Property 2: Mutation clamps all seven traits to configured bounds**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4**

  - [ ]* 1.6 Write property test: zero-stddev mutation is identity
    - **Property 3: Zero-stddev mutation is identity**
    - **Validates: Requirements 2.5**

  - [ ]* 1.7 Write property test: validation rejects invalid configs
    - **Property 4: Validation rejects invalid new trait clamp configurations**
    - **Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11**

- [ ] 2. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 3. Update system functions to use per-actor traits
  - [ ] 3.1 Update `run_actor_sensing` in `src/grid/actor_systems.rs`
    - Change `config.max_tumble_steps` to `actor.traits.max_tumble_steps` in the tumble initiation branch
    - _Requirements: 4.1_

  - [ ] 3.2 Update `run_actor_reproduction` in `src/grid/actor_systems.rs`
    - Change `config.reproduction_cost` to `actor.traits.reproduction_cost` for parent energy deduction
    - Change `config.offspring_energy` to `actor.traits.offspring_energy` in the spawn buffer push
    - _Requirements: 5.1, 6.1_

  - [ ] 3.3 Fix existing tests in `src/grid/actor_systems.rs`
    - Update all `HeritableTraits::from_config` call sites and any direct `HeritableTraits` construction in tests to include the three new fields
    - Update `default_config()` test helper if needed
    - _Requirements: 1.1, 4.1, 5.1, 6.1_

  - [ ]* 3.4 Write property test: sensing uses per-actor max_tumble_steps
    - **Property 5: Sensing uses per-actor max_tumble_steps**
    - **Validates: Requirements 4.1**

  - [ ]* 3.5 Write property test: reproduction uses per-actor reproduction_cost and offspring_energy
    - **Property 6: Reproduction uses per-actor reproduction_cost and offspring_energy**
    - **Validates: Requirements 5.1, 6.1, 6.2**

- [ ] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Update visualization
  - [ ] 5.1 Update `TraitStats` in `src/viz_bevy/resources.rs`
    - Change `Option<[SingleTraitStats; 4]>` to `Option<[SingleTraitStats; 7]>`
    - Update the doc comment to list all seven traits
    - _Requirements: 7.5_

  - [ ] 5.2 Update `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs`
    - Add three new `Vec<f32>` buffers for `max_tumble_steps` (cast to f32), `reproduction_cost`, `offspring_energy`
    - Collect values from non-inert actors
    - Compute stats and include in the 7-element array
    - _Requirements: 7.1_

  - [ ] 5.3 Update `TRAIT_NAMES`, `format_trait_stats`, and `format_actor_info` in `src/viz_bevy/setup.rs`
    - Extend `TRAIT_NAMES` array from 4 to 7 entries
    - Add three new trait lines to `format_actor_info`
    - `format_trait_stats` loop already iterates `TRAIT_NAMES` — no logic change needed beyond the array
    - _Requirements: 7.2, 7.3_

  - [ ] 5.4 Update `format_config_info` in `src/viz_bevy/setup.rs`
    - Add six new clamp bound lines to the Actors section
    - _Requirements: 7.4_

  - [ ]* 5.5 Write property test: trait stats covers seven traits
    - **Property 7: Trait stats computation covers all seven traits**
    - **Validates: Requirements 7.1, 7.5**

  - [ ]* 5.6 Write property test: formatting includes all seven traits
    - **Property 8: Formatting includes all seven trait names and values**
    - **Validates: Requirements 7.2, 7.3**

- [ ] 6. Update configuration files and documentation
  - [ ] 6.1 Update `example_config.toml`
    - Add `trait_max_tumble_steps_min`, `trait_max_tumble_steps_max`, `trait_reproduction_cost_min`, `trait_reproduction_cost_max`, `trait_offspring_energy_min`, `trait_offspring_energy_max` with explanatory comments
    - Update the heritable trait mutation comment block to list all seven traits
    - _Requirements: 8.1_

  - [ ] 6.2 Update `.kiro/steering/config-documentation.md`
    - Add six new fields to the `[actor]` — `ActorConfig` configuration reference table
    - Update the Heritable Trait Update Rule to list all seven traits
    - Update `TraitStats.traits` array size reference from 4 to 7
    - _Requirements: 8.2, 8.3_

- [ ] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- The `run_deferred_spawn` function requires no code changes — it already reads energy from the spawn buffer
- Existing tests in `actor_systems.rs` must be updated to construct `HeritableTraits` with 7 fields
