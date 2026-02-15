# Implementation Plan: Contact Predation

## Overview

Implement contact predation as a new tick phase, adding the `kin_tolerance` heritable trait, genetic distance computation, predation logic, config fields, and visualization updates. Tasks are ordered so each builds on the previous, with property tests close to the code they validate.

## Tasks

- [ ] 1. Add `kin_tolerance` to `HeritableTraits` and `ActorConfig`
  - [ ] 1.1 Add `kin_tolerance: f32` field to `HeritableTraits` in `src/grid/actor.rs`
    - Update the struct definition, adjust the size assertion from 32 to 36 bytes
    - Update `from_config()` to initialize `kin_tolerance` from `config.kin_tolerance`
    - Update `mutate()` to apply proportional gaussian mutation and clamp to `[trait_kin_tolerance_min, trait_kin_tolerance_max]`
    - _Requirements: 1.1, 1.2, 1.3_
  - [ ] 1.2 Add predation config fields to `ActorConfig` in `src/grid/actor_config.rs`
    - Add `absorption_efficiency: f32` (default 0.5), `kin_tolerance: f32` (default 0.5), `trait_kin_tolerance_min: f32` (default 0.0), `trait_kin_tolerance_max: f32` (default 1.0)
    - Add serde default functions for each new field
    - Update `Default` impl with new field defaults
    - _Requirements: 1.4, 1.5, 6.1, 6.2_
  - [ ] 1.3 Add config validation for new fields
    - Validate `absorption_efficiency` is in (0.0, 1.0]
    - Validate `trait_kin_tolerance_min < trait_kin_tolerance_max`
    - _Requirements: 6.3, 6.4_
  - [ ] 1.4 Fix all compilation errors from the new field
    - Update any code that constructs `HeritableTraits` directly (tests, spawn logic) to include `kin_tolerance`
    - _Requirements: 1.1_
  - [ ]* 1.5 Write property test: mutation clamp invariant for kin_tolerance
    - **Property 1: Mutation clamp invariant for kin_tolerance**
    - Generate random HeritableTraits within bounds, call mutate(), verify kin_tolerance stays in [min, max]
    - **Validates: Requirements 1.3**
  - [ ]* 1.6 Write property test: config validation rejects invalid predation fields
    - **Property 9: Config validation rejects invalid predation fields**
    - Generate absorption_efficiency outside (0.0, 1.0] and trait_kin_tolerance_min >= max, verify rejection
    - **Validates: Requirements 6.3, 6.4**

- [ ] 2. Implement genetic distance computation
  - [ ] 2.1 Add `genetic_distance` function in `src/grid/actor_systems.rs`
    - Pure function: `fn genetic_distance(a: &HeritableTraits, b: &HeritableTraits, config: &ActorConfig) -> f32`
    - Normalize each of 9 traits to [0,1] using clamp bounds, handle zero-range as 0.0
    - Compute Euclidean distance / sqrt(9)
    - No heap allocation, `#[inline]`
    - _Requirements: 2.1, 2.2, 2.3_
  - [ ]* 2.2 Write property tests for genetic distance
    - **Property 2: Genetic distance range and formula correctness**
    - **Property 3: Genetic distance symmetry**
    - **Property 4: Genetic distance identity**
    - Generate random trait vectors within bounds, verify range [0,1], symmetry, identity, and formula match
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [ ] 3. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement contact predation phase
  - [ ] 4.1 Add `run_contact_predation` function in `src/grid/actor_systems.rs`
    - Two-pass approach: pass 1 collects predation events in SmallVec, pass 2 applies them
    - Iterate actors by ascending slot index
    - For each non-inert actor, scan 4-neighborhood via occupancy map
    - Check energy dominance (strictly higher), genetic distance >= kin_tolerance
    - Record (predator_slot, prey_slot, energy_gain) events
    - Apply: add energy to predator (clamped to max_energy), mark prey inert, queue prey ActorId for removal
    - Each actor participates in at most one event (track with a `participated` set or by checking events)
    - Return `Result<(), TickError>` with NaN/Inf checks on predator energy
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 4.1, 4.2, 4.3, 4.4_
  - [ ] 4.2 Integrate predation phase into `run_actor_phases` in `src/grid/tick.rs`
    - Insert `run_contact_predation` call after deferred spawn (Phase 4.5) and before movement (Phase 5)
    - Follow with `run_deferred_removal` for predated actors
    - Pass grid width, height, actor_config, occupancy, removal_buffer
    - _Requirements: 5.1, 5.3_
  - [ ]* 4.3 Write property tests for predation logic
    - **Property 5: Predation eligibility decision**
    - **Property 6: Predation energy conservation**
    - **Property 7: At most one predation per actor per tick**
    - **Property 8: Predation determinism**
    - Generate small grids with adjacent actors, verify eligibility, energy transfer, uniqueness, and determinism
    - **Validates: Requirements 3.1–3.6, 4.1–4.4**
  - [ ]* 4.4 Write unit tests for predation edge cases
    - Equal energy → no predation
    - Boundary kin_tolerance (distance exactly at threshold)
    - Predator at max_energy → clamped
    - All neighbors inert → no predation
    - Grid boundary actors with fewer than 4 neighbors
    - _Requirements: 3.3, 3.6, 4.1_

- [ ] 5. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Update visualization for 9th trait
  - [ ] 6.1 Update `TraitStats` in `src/viz_bevy/resources.rs`
    - Change `[SingleTraitStats; 8]` to `[SingleTraitStats; 9]`
    - Update array order comment to include `kin_tolerance`
    - _Requirements: 7.1_
  - [ ] 6.2 Update `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs`
    - Add kin_tolerance collection and statistics computation as the 9th element
    - _Requirements: 7.2_
  - [ ] 6.3 Update formatting functions in `src/viz_bevy/setup.rs`
    - Add `kin_tolerance` row to `format_trait_stats` (stats panel)
    - Add `kin_tolerance` value to `format_actor_info` (actor inspector)
    - Add `absorption_efficiency`, `kin_tolerance`, `trait_kin_tolerance_min`, `trait_kin_tolerance_max` to `format_config_info` (config info panel)
    - _Requirements: 7.3, 7.4, 7.5_

- [ ] 7. Update documentation
  - [ ] 7.1 Update `example_config.toml`
    - Add `absorption_efficiency`, `kin_tolerance`, `trait_kin_tolerance_min`, `trait_kin_tolerance_max` fields with comments
    - _Requirements: 8.1_
  - [ ] 7.2 Update `.kiro/steering/config-documentation.md`
    - Add new fields to the `[actor]` ActorConfig table
    - Add `kin_tolerance` to the heritable trait list in the Heritable Trait Update Rule section
    - _Requirements: 8.2_

- [ ] 8. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- The `SmallVec` dependency may need to be added to `Cargo.toml` if not already present; alternatively, a stack-allocated array can be used if the maximum event count is bounded
