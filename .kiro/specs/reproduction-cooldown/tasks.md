# Implementation Plan: Reproduction Cooldown

## Overview

Add a reproduction cooldown heritable trait with a continuous reproductive readiness metabolic cost. Implementation proceeds bottom-up: config fields first, then the heritable trait and runtime state, then the metabolism readiness cost, then the reproduction cooldown gate, then genetic distance, then visualization, then documentation. Tests are interleaved with implementation.

## Tasks

- [ ] 1. Add reproduction cooldown and readiness config fields to ActorConfig
  - [ ] 1.1 Add serde default functions and new fields to `src/grid/actor_config.rs`
    - Add `default_reproduction_cooldown() -> u16 { 5 }`, `default_trait_reproduction_cooldown_min() -> u16 { 1 }`, `default_trait_reproduction_cooldown_max() -> u16 { 100 }`, `default_readiness_sensitivity() -> f32 { 0.01 }`, `default_reference_cooldown() -> f32 { 5.0 }`
    - Add `reproduction_cooldown`, `trait_reproduction_cooldown_min`, `trait_reproduction_cooldown_max`, `readiness_sensitivity`, `reference_cooldown` fields to `ActorConfig` with `#[serde(default = "...")]` attributes
    - Add the five fields to the `Default` impl
    - _Requirements: 1.4, 1.5, 3.3, 3.4_

  - [ ] 1.2 Add config validation to `validate_world_config` in `src/io/config_file.rs`
    - Validate `trait_reproduction_cooldown_min >= 1`
    - Validate `trait_reproduction_cooldown_min < trait_reproduction_cooldown_max`
    - Validate `reproduction_cooldown` within `[trait_reproduction_cooldown_min, trait_reproduction_cooldown_max]`
    - Validate `readiness_sensitivity >= 0.0` and finite
    - Validate `reference_cooldown > 0.0` and finite
    - _Requirements: 5.2, 5.3, 5.4, 5.5_

- [ ] 2. Add reproduction_cooldown to HeritableTraits and cooldown_remaining to Actor
  - [ ] 2.1 Add `reproduction_cooldown: u16` field to `HeritableTraits` in `src/grid/actor.rs`
    - Add the field after `optimal_temp`
    - Update the compile-time size assertion
    - Update `from_config` to initialize `reproduction_cooldown` from `config.reproduction_cooldown`
    - Update `mutate` to apply proportional gaussian mutation in f32 space, round, clamp to `[trait_reproduction_cooldown_min, trait_reproduction_cooldown_max]` (same pattern as `max_tumble_steps`)
    - _Requirements: 1.1, 1.2, 1.3_

  - [ ] 2.2 Add `cooldown_remaining: u16` field to `Actor` in `src/grid/actor.rs`
    - Add the field after `traits`
    - Update all `Actor` construction sites (in `run_deferred_spawn`, `world_init`, and any test helpers) to include `cooldown_remaining: 0`
    - _Requirements: 2.1_

  - [ ]* 2.3 Write property tests for HeritableTraits reproduction_cooldown
    - **Property 1: from_config initializes reproduction_cooldown from config**
    - **Property 2: Mutation clamp invariant for reproduction_cooldown**
    - **Validates: Requirements 1.2, 1.3, 1.6**

- [ ] 3. Add reproductive readiness cost to run_actor_metabolism
  - [ ] 3.1 Update `run_actor_metabolism` in `src/grid/actor_systems.rs`
    - For active (non-inert) actors: compute `readiness_cost = config.readiness_sensitivity * (reproduction_cost + offspring_energy) / max(reproduction_cooldown, 1) / config.reference_cooldown`
    - Subtract `readiness_cost` from energy alongside `base_energy_decay` and `thermal_cost`
    - Inert actors: no change (no readiness cost)
    - _Requirements: 3.1, 3.2, 3.5, 3.6, 3.7_

  - [ ]* 3.2 Write property tests for readiness cost
    - **Property 5: Readiness cost follows formula**
    - **Property 6: Zero readiness_sensitivity produces zero readiness cost**
    - **Property 7: Inert actors receive no readiness cost**
    - **Validates: Requirements 3.1, 3.2, 3.5, 3.6**

- [ ] 4. Add cooldown gate to run_actor_reproduction
  - [ ] 4.1 Update `run_actor_reproduction` in `src/grid/actor_systems.rs`
    - After the inert check, add cooldown check: if `cooldown_remaining > 0`, decrement by 1 and skip
    - After successful fission, set `actor.cooldown_remaining = actor.traits.reproduction_cooldown`
    - _Requirements: 2.2, 2.3, 2.4, 2.5_

  - [ ]* 4.2 Write property tests for cooldown gate
    - **Property 3: Cooldown decrement and reproduction skip**
    - **Property 4: Cooldown set after successful reproduction**
    - **Validates: Requirements 2.2, 2.3**

- [ ] 5. Checkpoint
  - Run `cargo test` and `cargo clippy`. Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Update genetic distance
  - [ ] 6.1 Update `genetic_distance` in `src/grid/actor_systems.rs`
    - Change `TRAIT_COUNT` from 10 to 11
    - Add `(a.reproduction_cooldown as f32, b.reproduction_cooldown as f32, config.trait_reproduction_cooldown_min as f32, config.trait_reproduction_cooldown_max as f32)` to the traits array
    - _Requirements: 4.1, 4.2_

  - [ ]* 6.2 Write property test for genetic_distance with reproduction_cooldown
    - **Property 8: Genetic distance sensitivity to reproduction_cooldown**
    - **Validates: Requirements 4.1**

- [ ] 7. Update visualization layer
  - [ ] 7.1 Update `TraitStats` and `compute_trait_stats_from_actors` in `src/viz_bevy/resources.rs` and `src/viz_bevy/systems.rs`
    - Change `TraitStats.traits` from `[SingleTraitStats; 10]` to `[SingleTraitStats; 11]`
    - Add `reproduction_cooldown` buffer in `compute_trait_stats_from_actors`, collect values, compute stats at index 10
    - _Requirements: 6.1, 6.2_

  - [ ] 7.2 Update `TRAIT_NAMES`, `format_actor_info`, and `format_config_info` in `src/viz_bevy/setup.rs`
    - Extend `TRAIT_NAMES` from 10 to 11 entries, appending `"repro_cooldown"`
    - Add `reproduction_cooldown` trait line and `cooldown_remaining` timer line to `format_actor_info`
    - Add `reproduction_cooldown`, `trait_reproduction_cooldown_min`, `trait_reproduction_cooldown_max`, `readiness_sensitivity`, `reference_cooldown` to `format_config_info`
    - _Requirements: 6.3, 6.4, 5.7_

  - [ ]* 7.3 Write property tests for visualization
    - **Property 11: format_config_info contains all new fields**
    - **Property 12: format_actor_info contains reproduction_cooldown and cooldown_remaining**
    - **Property 13: Stats collection includes reproduction_cooldown**
    - **Validates: Requirements 5.7, 6.2, 6.4**

- [ ] 8. Update configuration documentation
  - [ ] 8.1 Update `example_config.toml`
    - Add `reproduction_cooldown`, `trait_reproduction_cooldown_min`, `trait_reproduction_cooldown_max`, `readiness_sensitivity`, `reference_cooldown` with comments in the `[actor]` section
    - _Requirements: 5.6_

  - [ ] 8.2 Update `config-documentation.md` steering file
    - Add all five new ActorConfig fields to the configuration reference table
    - Add `reproduction_cooldown` to the heritable trait list in the Heritable Trait Update Rule
    - Update `TraitStats.traits` array size from 10 to 11 in the Bevy Runtime Resources section
    - _Requirements: 5.8_

  - [ ]* 8.3 Write property tests for config validation
    - **Property 9: Config validation rejects invalid reproduction_cooldown configurations**
    - **Property 10: Config validation rejects invalid readiness parameters**
    - **Validates: Requirements 5.2, 5.3, 5.4, 5.5**

- [ ] 9. Final checkpoint
  - Run `cargo test` and `cargo clippy`. Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` with minimum 100 iterations per property
- The readiness cost computation is pure arithmetic in the HOT path — three multiplications and one division, zero allocations, no new branches
- The cooldown check is a single u16 comparison and decrement — zero allocation
- All `Actor` construction sites must be updated in task 2.2 to include `cooldown_remaining: 0` or the project will not compile
