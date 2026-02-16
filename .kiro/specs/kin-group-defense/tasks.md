# Implementation Plan: Kin-Group Defense

## Overview

Implement probabilistic group defense for contact predation. Each prey's allied neighbors contribute their heritable `kin_group_defense` trait value to reduce predation success probability. The mechanic is always active and degrades gracefully to current behavior when no allies are present.

## Tasks

- [x] 1. Add `kin_group_defense` heritable trait to data model
  - [x] 1.1 Add `kin_group_defense: f32` field to `HeritableTraits` in `src/grid/actor.rs`
    - Insert after `kin_tolerance` field
    - _Requirements: 2.1_
  - [x] 1.2 Update `HeritableTraits::from_config` to initialize `kin_group_defense` from `config.kin_group_defense`
    - _Requirements: 2.1, 2.2_
  - [x] 1.3 Add mutation line for `kin_group_defense` in `HeritableTraits::mutate`
    - Use same proportional Gaussian noise pattern as other f32 traits
    - Clamp to `[config.trait_kin_group_defense_min, config.trait_kin_group_defense_max]`
    - Insert after the `kin_tolerance` mutation line
    - _Requirements: 2.5_
  - [ ]* 1.4 Write property test: mutation preserves kin_group_defense clamp bounds
    - **Property 5: Mutation preserves kin_group_defense clamp bounds**
    - **Validates: Requirements 2.5**

- [x] 2. Add `kin_group_defense` config fields to `ActorConfig`
  - [x] 2.1 Add `kin_group_defense`, `trait_kin_group_defense_min`, `trait_kin_group_defense_max` fields to `ActorConfig` in `src/grid/actor_config.rs`
    - Add default functions: `default_kin_group_defense() -> 0.5`, `default_trait_kin_group_defense_min() -> 0.0`, `default_trait_kin_group_defense_max() -> 1.0`
    - Add `#[serde(default = "...")]` attributes
    - Place in the contact predation config section after `trait_kin_tolerance_max`
    - _Requirements: 2.2, 2.3_
  - [x] 2.2 Add validation for new fields in `ActorConfig::validate()`
    - `trait_kin_group_defense_min < trait_kin_group_defense_max`
    - `kin_group_defense` within `[trait_kin_group_defense_min, trait_kin_group_defense_max]`
    - _Requirements: 2.2, 2.3_

- [x] 3. Update `genetic_distance` to include `kin_group_defense`
  - [x] 3.1 Increment `TRAIT_COUNT` from 11 to 12 in `src/grid/actor_systems.rs`
    - _Requirements: 2.4_
  - [x] 3.2 Add `kin_group_defense` entry to the traits array in `genetic_distance`
    - Insert after `kin_tolerance` entry, before `optimal_temp`
    - `(a.kin_group_defense, b.kin_group_defense, config.trait_kin_group_defense_min, config.trait_kin_group_defense_max)`
    - _Requirements: 2.4_
  - [ ]* 3.3 Write property test: genetic distance includes kin_group_defense
    - **Property 4: Genetic distance includes kin_group_defense**
    - **Validates: Requirements 2.4**

- [x] 4. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement group defense mechanic in `run_contact_predation`
  - [x] 5.1 Add `sum_allied_defense` helper function in `src/grid/actor_systems.rs`
    - Signature: `fn sum_allied_defense(prey_cell: usize, prey_traits: &HeritableTraits, predator_slot: usize, occupancy: &[Option<usize>], actors: &ActorRegistry, config: &ActorConfig, w: usize, h: usize) -> f32`
    - Iterate directions 0..4 via `direction_to_target`, skip predator slot, skip inert, check genetic distance < prey's kin_tolerance, sum `kin_group_defense` values
    - _Requirements: 1.1, 1.2, 1.5, 1.6, 1.7_
  - [ ]* 5.2 Write property test: allied neighbor identification correctness
    - **Property 1: Allied neighbor identification correctness**
    - **Validates: Requirements 1.1, 1.5, 1.6, 1.7**
  - [x] 5.3 Update `run_contact_predation` signature to accept `rng: &mut impl Rng`
    - _Requirements: 4.1_
  - [x] 5.4 Integrate probabilistic check into pass 1 of `run_contact_predation`
    - After existing eligibility checks (energy dominance, genetic distance), call `sum_allied_defense`
    - Compute `success_probability = 1.0 / (1.0 + ally_defense_sum)`
    - Sample `rng.gen::<f32>()` and succeed only if sample < success_probability
    - On failure: mark predator as participated, do NOT mark prey, continue to next direction
    - On success: push event, mark both participated, break
    - _Requirements: 1.2, 1.3, 1.4, 3.1, 3.2, 5.1, 5.2_
  - [ ]* 5.5 Write property test: defense probability and outcome correctness
    - **Property 2: Defense probability formula correctness**
    - **Validates: Requirements 1.2, 1.3, 1.4**
  - [ ]* 5.6 Write property test: deterministic replay
    - **Property 3: Deterministic replay**
    - **Validates: Requirements 3.1, 3.2, 3.3**
  - [ ]* 5.7 Write property test: predator energy clamped after successful predation
    - **Property 6: Predator energy clamped after successful predation**
    - **Validates: Requirements 6.2**

- [x] 6. Update call site in `run_actor_phases`
  - [x] 6.1 Pass `&mut tick_rng` to `run_contact_predation` in `src/grid/tick.rs`
    - _Requirements: 4.2_

- [x] 7. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Update visualization for new heritable trait
  - [x] 8.1 Update `TraitStats.traits` array size from 11 to 12 in `src/viz_bevy/resources.rs`
    - _Requirements: 8.2_
  - [x] 8.2 Add `kin_group_defense` collection and stats computation in `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs`
    - Add `kin_group_defense` Vec, push values in the actor loop, compute stats as index 11
    - _Requirements: 8.1_
  - [x] 8.3 Add `"kin_group_defense"` to `TRAIT_NAMES` array in `src/viz_bevy/setup.rs`
    - Update array size from 11 to 12, append entry
    - _Requirements: 8.3_
  - [x] 8.4 Add `kin_group_defense` line to `format_actor_info` in `src/viz_bevy/setup.rs`
    - Insert after `kin_tolerance` line
    - _Requirements: 8.4_

- [x] 9. Update configuration documentation
  - [x] 9.1 Add `kin_group_defense`, `trait_kin_group_defense_min`, `trait_kin_group_defense_max` to `format_config_info` in `src/viz_bevy/setup.rs`
    - Insert after the `trait_kin_tolerance` line
    - _Requirements: 7.2_
  - [x] 9.2 Add new fields to `example_config.toml` with explanatory comments
    - Place in the `[actor]` section after `trait_kin_tolerance_max`
    - _Requirements: 7.1_

- [x] 10. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- `TRAIT_COUNT` increment (11→12) affects `genetic_distance` normalization — all existing genetic distances will shift slightly. This is expected and correct.
- The `TraitStats` array index for `kin_group_defense` is 11 (appended after `repro_cooldown`), keeping all existing indices stable.
- Property tests use `proptest` crate with minimum 100 iterations per property.
