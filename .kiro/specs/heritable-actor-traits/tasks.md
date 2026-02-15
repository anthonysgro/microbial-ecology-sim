# Implementation Plan: Heritable Actor Traits

## Overview

Introduce per-actor heritable traits (16-byte `HeritableTraits` struct) with gaussian mutation during binary fission. Four fields move from global `ActorConfig` reads to per-actor reads: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`. Mutation is deterministic, clamped, and configurable.

## Tasks

- [x] 1. Define `HeritableTraits` struct and embed in `Actor`
  - [x] 1.1 Create `HeritableTraits` struct in `src/grid/actor.rs`
    - Four `f32` fields: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`
    - Derive `Debug, Clone, Copy, PartialEq`
    - Add `from_config(config: &ActorConfig) -> Self` constructor
    - Add `static_assert` that `size_of::<HeritableTraits>() == 16`
    - _Requirements: 1.1, 1.2, 1.3_
  - [x] 1.2 Add `traits: HeritableTraits` field to `Actor` struct
    - Update all `Actor` construction sites: `generate_actors` in `world_init.rs`, `run_deferred_spawn` in `actor_systems.rs`, and all test helpers
    - Seed actors use `HeritableTraits::from_config(&actor_config)`
    - _Requirements: 1.4, 2.1_
  - [ ]* 1.3 Write property test: seed actor traits match config (Property 1)
    - **Property 1: Seed actor traits match config**
    - Generate random valid `ActorConfig`, call `HeritableTraits::from_config`, assert all four fields match
    - **Validates: Requirements 2.1**

- [x] 2. Add mutation config fields to `ActorConfig`
  - [x] 2.1 Add new fields to `ActorConfig` in `src/grid/actor_config.rs`
    - `mutation_stddev: f32` (default `0.05`)
    - Eight clamp range fields: `trait_consumption_rate_min/max`, `trait_base_energy_decay_min/max`, `trait_levy_exponent_min/max`, `trait_reproduction_threshold_min/max` with defaults per spec
    - Update `Default` impl
    - Add `#[serde(default)]` attributes for backward-compatible TOML parsing
    - _Requirements: 4.1, 4.3, 4.4, 4.5, 4.6, 6.1, 6.2_
  - [x] 2.2 Add config validation for new fields
    - In `validate_world_config` (`src/io/config_file.rs`): reject `mutation_stddev < 0.0`, reject `trait_*_min >= trait_*_max`, reject `trait_levy_exponent_min <= 1.0`, reject `trait_consumption_rate_min <= 0.0`, reject `trait_base_energy_decay_min <= 0.0`, reject `trait_reproduction_threshold_min <= 0.0`
    - In `Grid::new` (`src/grid/mod.rs`): mirror the same checks
    - Validate that default config values for the four heritable fields fall within their clamp ranges
    - _Requirements: 4.2, 6.4_
  - [ ]* 2.3 Write unit tests for config validation and defaults
    - Test default clamp range values match spec (4.3–4.6)
    - Test default `mutation_stddev` is `0.05`
    - Test validation rejects negative `mutation_stddev`
    - Test validation rejects `trait_*_min >= trait_*_max`
    - _Requirements: 4.3, 4.4, 4.5, 4.6, 6.2, 6.4_

- [x] 3. Implement `HeritableTraits::mutate` method
  - [x] 3.1 Add `mutate(&mut self, config: &ActorConfig, rng: &mut impl Rng)` to `HeritableTraits`
    - Add `rand_distr` dependency to `Cargo.toml` (for `Normal` distribution)
    - Early return if `mutation_stddev == 0.0`
    - Sample from `Normal(0.0, mutation_stddev)` independently for each field
    - Clamp each field to its configured range after mutation
    - _Requirements: 3.2, 3.3, 3.4_
  - [ ]* 3.2 Write property test: mutation clamp invariant (Property 2)
    - **Property 2: Mutation clamp invariant**
    - Generate random traits, random valid config with arbitrary clamp ranges and stddev, call `mutate()`, assert all fields within bounds
    - **Validates: Requirements 3.4, 4.2**
  - [ ]* 3.3 Write property test: zero-stddev identity (Property 3)
    - **Property 3: Zero-stddev identity**
    - Generate random traits, config with `mutation_stddev = 0.0`, call `mutate()`, assert traits unchanged
    - **Validates: Requirements 3.1, 6.3**
  - [ ]* 3.4 Write property test: non-zero mutation produces variation (Property 4)
    - **Property 4: Non-zero mutation produces variation**
    - Generate random traits, config with `mutation_stddev > 0.0`, call `mutate()` 100 times with different RNG seeds, assert at least one result differs from input
    - **Validates: Requirements 3.2, 3.3**

- [x] 4. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Update spawn buffer and reproduction pipeline
  - [x] 5.1 Change spawn buffer type from `Vec<(usize, f32)>` to `Vec<(usize, f32, HeritableTraits)>`
    - Update `Grid.spawn_buffer` field type in `src/grid/mod.rs`
    - Update `Grid::take_actors` and `Grid::put_actors` signatures
    - Update all spawn buffer construction and access sites
    - _Requirements: 3.1_
  - [x] 5.2 Update `run_actor_reproduction` to use per-actor threshold and copy traits
    - Read `actor.traits.reproduction_threshold` instead of `config.reproduction_threshold`
    - Push `(cell, offspring_energy, actor.traits)` into spawn buffer
    - _Requirements: 3.1, 5.3_
  - [x] 5.3 Update `run_deferred_spawn` to apply mutation
    - Add `config: &ActorConfig`, `seed: u64`, `tick: u64` parameters
    - Derive per-offspring RNG seed from `seed`, `tick`, and spawn buffer index
    - Clone parent traits, call `mutate()`, assign to offspring `Actor`
    - Update call site in `run_actor_phases` (`src/grid/tick.rs`) to pass new args
    - _Requirements: 3.2, 3.4, 3.5, 7.1_
  - [ ]* 5.4 Write property test: replay determinism (Property 5)
    - **Property 5: Replay determinism**
    - Generate random seed, tick, and spawn buffer contents, run `run_deferred_spawn` twice with identical inputs, assert all offspring traits are identical
    - **Validates: Requirements 7.1, 7.2**
  - [ ]* 5.5 Write property test: reproduction uses per-actor threshold (Property 8)
    - **Property 8: Reproduction uses per-actor threshold**
    - Generate actor with energy between per-actor and global thresholds, run `run_actor_reproduction`, assert eligibility matches per-actor threshold
    - **Validates: Requirements 5.3**

- [x] 6. Update metabolism and sensing systems to read per-actor traits
  - [x] 6.1 Update `run_actor_metabolism` to read per-actor traits
    - Replace `config.consumption_rate` with `actor.traits.consumption_rate`
    - Replace `config.base_energy_decay` with `actor.traits.base_energy_decay` (both active and inert branches)
    - _Requirements: 5.1, 5.4_
  - [x] 6.2 Update `run_actor_sensing` to read per-actor traits
    - Move break-even computation inside the per-actor loop
    - Replace `config.base_energy_decay` with `actor.traits.base_energy_decay` in break-even formula
    - Replace `config.levy_exponent` with `actor.traits.levy_exponent` in `sample_tumble_steps` call
    - _Requirements: 5.2, 5.5_
  - [ ]* 6.3 Write property test: metabolism uses per-actor traits (Property 6)
    - **Property 6: Metabolism uses per-actor traits**
    - Generate two actors with different `consumption_rate` and `base_energy_decay` on cells with identical chemical concentration, run `run_actor_metabolism`, assert different energy deltas
    - **Validates: Requirements 5.1, 5.4**
  - [ ]* 6.4 Write property test: sensing uses per-actor traits (Property 7)
    - **Property 7: Sensing uses per-actor traits**
    - Generate two actors with sufficiently different `levy_exponent`, call `sample_tumble_steps` with same RNG state, assert different step counts. Generate two actors with different `base_energy_decay`, compute break-even, assert different thresholds
    - **Validates: Requirements 5.2, 5.5**

- [x] 7. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Update existing tests
  - [x] 8.1 Fix all existing tests that construct `Actor` instances
    - Add `traits: HeritableTraits::from_config(&config)` (or equivalent) to every `Actor { .. }` literal in test code
    - Fix spawn buffer type in any test that constructs or asserts on spawn buffer contents
    - Ensure all existing tests still pass with the new `Actor` layout
    - _Requirements: 1.4_

- [x] 9. Update configuration documentation
  - [x] 9.1 Update `example_config.toml`
    - Add `mutation_stddev` and all eight `trait_*_min/max` fields under `[actor]` with descriptive comments
    - _Requirements: 8.1_
  - [x] 9.2 Update `README.md`
    - Document new `ActorConfig` fields in the configuration reference section
    - _Requirements: 8.2_
  - [x] 9.3 Update Bevy config info panel
    - Add `mutation_stddev` and clamp range fields to `format_config_info()` in `src/viz_bevy/setup.rs`
    - _Requirements: 8.3_
  - [x] 9.4 Update `config-documentation.md` steering file
    - Add new `ActorConfig` fields to the configuration reference table
    - _Requirements: 8.1, 8.2, 8.3_

- [x] 10. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` with minimum 256 iterations
- The spawn buffer type change (task 5.1) is the most invasive refactor — it touches `Grid`, `take_actors`/`put_actors`, reproduction, deferred spawn, and tick orchestration
- Existing test fixes (task 8) are deferred to after all implementation is complete to avoid fixing tests twice
