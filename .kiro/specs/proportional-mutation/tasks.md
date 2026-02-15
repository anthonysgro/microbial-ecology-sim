# Implementation Plan: Proportional Mutation with Heritable Mutation Rate

## Overview

Two coupled changes: (1) switch all trait mutations from additive to proportional (multiplicative) noise, and (2) promote `mutation_stddev` to a per-actor heritable trait (`mutation_rate`). Fixes the `max_tumble_steps` dead-mutation bug and enables evolvability selection.

## Tasks

- [-] 1. Add mutation_rate to HeritableTraits and rewrite mutate()
  - [ ] 1.1 Add `mutation_rate: f32` field to `HeritableTraits` struct in `src/grid/actor.rs`
    - Append after `offspring_energy`
    - Update compile-time size assert from `== 28` to `== 32`
    - _Requirements: 5.1, 5.5_

  - [ ] 1.2 Update `HeritableTraits::from_config` to initialize `mutation_rate` from `config.mutation_stddev`
    - _Requirements: 5.2_

  - [ ] 1.3 Rewrite `HeritableTraits::mutate()` to proportional model with per-actor mutation_rate
    - Change early-return guard from `config.mutation_stddev == 0.0` to `self.mutation_rate == 0.0`
    - Change `Normal::new` to use `self.mutation_rate` instead of `config.mutation_stddev`
    - Replace all additive mutation lines (`trait + noise`) with proportional (`trait * (1.0 + noise)`) for all seven existing traits
    - Add proportional self-mutation for `mutation_rate` as the last trait mutated, clamped to `[trait_mutation_rate_min, trait_mutation_rate_max]`
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 5.3, 5.4, 7.1, 7.2, 7.3_

  - [ ] 1.4 Fix all existing code that constructs `HeritableTraits` literals (tests, etc.) to include the new `mutation_rate` field
    - _Requirements: 5.1_

- [~] 2. Add mutation_rate clamp config fields and validation
  - [ ] 2.1 Add `default_trait_mutation_rate_min()` (0.001) and `default_trait_mutation_rate_max()` (0.5) serde default functions in `src/grid/actor_config.rs`
    - _Requirements: 6.1_

  - [ ] 2.2 Add `trait_mutation_rate_min: f32` and `trait_mutation_rate_max: f32` fields to `ActorConfig` with serde defaults, and update `Default` impl
    - _Requirements: 6.1, 6.5_

  - [ ] 2.3 Add validation in `validate_world_config` in `src/io/config_file.rs`
    - `trait_mutation_rate_min > 0.0`
    - `trait_mutation_rate_min < trait_mutation_rate_max`
    - `mutation_stddev` within `[trait_mutation_rate_min, trait_mutation_rate_max]`
    - Add `trait_mutation_rate` to the f32 clamp range batch check or as a separate check
    - _Requirements: 6.2, 6.3, 6.4_

- [~] 3. Checkpoint — ensure all tests pass
  - Run `cargo test` and fix any compilation or test failures from the struct and mutation changes.

- [~] 4. Update visualization for 8th trait
  - [ ] 4.1 Update `TraitStats.traits` from `[SingleTraitStats; 7]` to `[SingleTraitStats; 8]` in `src/viz_bevy/resources.rs`
    - Update doc comment to list all eight traits
    - _Requirements: 8.1_

  - [ ] 4.2 Add `mutation_rate` buffer to `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs`
    - Collect `mutation_rate` values from non-inert actors
    - Include as the 8th element in the stats array
    - _Requirements: 8.2_

  - [ ] 4.3 Update `TRAIT_NAMES` from 7 to 8 entries, appending `"mutation_rate"` in `src/viz_bevy/setup.rs`
    - _Requirements: 8.3_

  - [ ] 4.4 Add `mutation_rate` line to `format_actor_info` in `src/viz_bevy/setup.rs`
    - _Requirements: 8.4_

  - [ ] 4.5 Add `trait_mutation_rate` clamp range line to `format_config_info` in `src/viz_bevy/setup.rs`
    - _Requirements: 8.6_

- [~] 5. Update documentation
  - [ ] 5.1 Update `example_config.toml`
    - Add `trait_mutation_rate_min` and `trait_mutation_rate_max` fields with comments
    - Update `mutation_stddev` comment to clarify it is the seed genome default for per-actor `mutation_rate`
    - _Requirements: 9.1, 9.2_

  - [ ] 5.2 Update `.kiro/steering/config-documentation.md`
    - Add `trait_mutation_rate_min/max` to the ActorConfig table
    - Update `mutation_stddev` description
    - Update heritable trait list from 7 to 8
    - Update `TraitStats.traits` array size from 7 to 8
    - _Requirements: 9.3_

- [~] 6. Checkpoint — ensure all tests pass
  - Run `cargo test` and fix any remaining compilation or test failures.

- [ ] 7. Property-based tests for proportional mutation and heritable mutation_rate
  - [ ] 7.1 Write `arb_valid_actor_config` proptest generator that produces valid configs including `trait_mutation_rate_min/max` and `mutation_stddev` within range
  - [ ] 7.2 Write `arb_heritable_traits` proptest generator that produces traits with all 8 fields within clamp ranges

  - [ ]* 7.3 [PBT] Property 1: Proportional mutation produces scale-dependent noise
    - For two trait instances differing only in magnitude, mean absolute deviation scales proportionally
    - **Validates: Requirements 1.1, 3.2**

  - [ ]* 7.4 [PBT] Property 2: All eight traits clamped to configured bounds after mutation
    - **Validates: Requirements 2.1, 2.2, 5.4**

  - [ ]* 7.5 [PBT] Property 3: Zero mutation_rate is identity
    - **Validates: Requirements 1.3, 7.3**

  - [ ]* 7.6 [PBT] Property 4: from_config initializes mutation_rate from mutation_stddev
    - **Validates: Requirements 5.2**

  - [ ]* 7.7 [PBT] Property 5: Deterministic mutation — same seed produces same output
    - **Validates: Requirements 4.1**

  - [ ]* 7.8 [PBT] Property 6: mutate uses per-actor mutation_rate, not config.mutation_stddev
    - **Validates: Requirements 5.3, 7.1**

- [ ] 8. Property-based tests for validation and visualization
  - [ ]* 8.1 [PBT] Property 7: Validation rejects invalid mutation rate clamp configurations
    - **Validates: Requirements 6.2, 6.3, 6.4**

  - [ ]* 8.2 [PBT] Property 8: Trait stats computation covers all 8 traits with valid statistics at index 7
    - **Validates: Requirements 8.1, 8.2**

  - [ ]* 8.3 [PBT] Property 9: format_trait_stats and format_actor_info include mutation_rate
    - **Validates: Requirements 8.4, 8.5**

- [ ] 9. Final checkpoint — ensure all tests pass
  - Run full `cargo test` suite and fix any remaining failures.

## Notes

- Tasks marked with `*` are optional property-based tests — can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- The `run_deferred_spawn` function requires no code changes — it already reads energy from the spawn buffer and calls `mutate()` which will automatically use the new proportional model
- Existing tests in `actor_systems.rs` must be updated to construct `HeritableTraits` with 8 fields (handled in Task 1.4)
