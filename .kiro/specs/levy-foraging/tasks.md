# Implementation Plan: Lévy Flight Foraging

## Overview

Incremental implementation of Lévy flight-based random foraging. Each task builds on the previous: data model changes first, then config, then the core sensing logic, then orchestrator wiring, and finally documentation updates.

## Tasks

- [x] 1. Add tumble state fields to Actor and update ActorConfig
  - [x] 1.1 Add `tumble_direction: u8` and `tumble_remaining: u16` to the `Actor` struct in `src/grid/actor.rs`
    - Default both to 0 in all existing Actor construction sites (tests, world_init, etc.)
    - Update any `Actor { ... }` literals across the codebase to include the new fields
    - _Requirements: 1.1, 1.2, 1.3_
  - [x] 1.2 Add `levy_exponent: f32` (default 1.5) and `max_tumble_steps: u16` (default 20) to `ActorConfig` in `src/grid/actor_config.rs`
    - Update the `Default` impl
    - _Requirements: 7.1, 7.2, 7.3, 7.4_
  - [x] 1.3 Add `seed: u64` field to the `Grid` struct in `src/grid/mod.rs`
    - Add `seed` parameter to `Grid::new()`, store it on the struct
    - Add `pub fn seed(&self) -> u64` accessor
    - Update all `Grid::new()` call sites to pass the seed (world_init, tests)
    - _Requirements: 6.1_
  - [x] 1.4 Add config validation for `levy_exponent > 1.0` and `max_tumble_steps >= 1` in `validate_world_config` in `src/io/config_file.rs`
    - _Requirements: 8.1, 8.2, 8.3_
  - [ ]* 1.5 Write property test for config validation (Property 7)
    - **Property 7: Validation accepts valid and rejects invalid Lévy config**
    - Use `proptest` to generate random `levy_exponent` and `max_tumble_steps` values
    - Assert validation passes iff `levy_exponent > 1.0 && max_tumble_steps >= 1`
    - **Validates: Requirements 8.1, 8.2, 8.3**
  - [ ]* 1.6 Write property test for ActorConfig TOML round-trip (Property 6)
    - **Property 6: ActorConfig TOML round-trip**
    - Use `proptest` to generate random valid ActorConfig instances including new fields
    - Serialize to TOML, deserialize, assert equality
    - **Validates: Requirements 7.3**

- [ ] 2. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Implement `sample_tumble_steps` and `direction_to_target` helpers
  - [x] 3.1 Add `rand_chacha = "0.3"` dependency to `Cargo.toml` (if not already present; `rand` should already be a dependency)
    - _Requirements: 6.1_
  - [x] 3.2 Implement `sample_tumble_steps(rng, alpha, max_steps) -> u16` in `src/grid/actor_systems.rs`
    - Inverse transform sampling: `u ~ Uniform(0,1)`, `steps = floor(u^(-1/(α-1)))`, clamp to `[1, max_steps]`
    - Floor `u` to `f32::EPSILON` to avoid infinity
    - _Requirements: 5.1, 5.2, 3.2_
  - [x] 3.3 Implement `direction_to_target(cell_index, direction, w, h) -> Option<usize>` in `src/grid/actor_systems.rs`
    - 0=North, 1=South, 2=West, 3=East; return None for out-of-bounds
    - _Requirements: 4.1, 4.2_
  - [ ]* 3.4 Write property test for step distribution range (Property 4)
    - **Property 4: Step distribution range invariant**
    - Use `proptest` to generate α ∈ (1.0, 20.0] and max_steps ∈ [1, 1000]
    - Assert output ∈ [1, max_steps] for all generated inputs
    - **Validates: Requirements 3.2, 5.1, 5.2**
  - [ ]* 3.5 Write unit tests for `direction_to_target`
    - Test all four directions at center, corners, and edges of a grid
    - Test out-of-bounds returns None
    - _Requirements: 4.1, 4.2_

- [x] 4. Rewrite `run_actor_sensing` with Lévy flight logic
  - [x] 4.1 Update `run_actor_sensing` signature to accept `&mut ActorRegistry`, `&ActorConfig`, and `&mut impl Rng`
    - Change `actors: &ActorRegistry` to `actors: &mut ActorRegistry`
    - Add `config: &ActorConfig` and `rng: &mut impl Rng` parameters
    - _Requirements: 9.1, 9.2, 9.3_
  - [x] 4.2 Implement break-even threshold evaluation and tumble state machine in `run_actor_sensing`
    - Compute `break_even = config.base_energy_decay / (config.energy_conversion_factor - config.extraction_cost)`
    - For each active actor: check if any neighbor is above break-even with positive gradient
    - If gradient found: follow it, reset tumble_remaining to 0
    - If no gradient and tumble_remaining > 0: continue tumble direction, decrement remaining
    - If no gradient and tumble_remaining == 0: sample new tumble via `sample_tumble_steps` and random direction
    - Handle boundary hits (direction_to_target returns None → reset tumble)
    - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.3, 4.1, 4.2, 4.3_
  - [x] 4.3 Update all `run_actor_sensing` call sites to pass the new parameters
    - Update `run_actor_phases` in `src/grid/tick.rs`
    - Update all test call sites in `src/grid/actor_systems.rs`
    - _Requirements: 9.1, 9.2, 9.3_
  - [ ]* 4.4 Write property test for gradient priority (Property 1)
    - **Property 1: Gradient takes priority over tumble**
    - Generate random grid states with at least one above-threshold neighbor, random actor tumble states
    - Assert sensing produces gradient target and tumble_remaining = 0
    - **Validates: Requirements 2.3, 4.3**
  - [ ]* 4.5 Write property test for no-gradient triggers tumble (Property 2)
    - **Property 2: No worthwhile gradient triggers tumble**
    - Generate random grid states with all cells at/below threshold, non-tumbling actors
    - Assert tumble is initiated with valid direction and step count
    - **Validates: Requirements 2.2, 3.1, 3.3**
  - [ ]* 4.6 Write property test for tumble continuation (Property 3)
    - **Property 3: Tumble continuation decrements remaining**
    - Generate random mid-tumble actors on grids with no above-threshold cells
    - Assert tumble_remaining decrements and target matches direction
    - **Validates: Requirements 4.1, 4.2**

- [x] 5. Wire per-tick RNG into the tick orchestrator
  - [x] 5.1 Update `TickOrchestrator::step` signature to accept `tick: u64` in `src/grid/tick.rs`
    - Create `ChaCha8Rng::seed_from_u64(grid.seed().wrapping_add(tick))` in `run_actor_phases`
    - Pass `&mut tick_rng` to `run_actor_sensing`
    - _Requirements: 6.1, 6.2_
  - [x] 5.2 Update all `TickOrchestrator::step` call sites to pass the tick number
    - Bevy: `src/viz_bevy/systems.rs` — pass `sim.tick`
    - Any other callers (tests, headless main)
    - _Requirements: 6.1_
  - [ ]* 5.3 Write property test for deterministic tumble sequences (Property 5)
    - **Property 5: Deterministic tumble sequences**
    - Generate random seed, tick, grid state; run sensing twice from cloned state
    - Assert identical movement targets and tumble state
    - **Validates: Requirements 6.1, 6.3**

- [x] 6. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Update configuration documentation
  - [x] 7.1 Update `example_config.toml` with `levy_exponent` and `max_tumble_steps` under `[actor]` with explanatory comments
    - _Requirements: 10.1_
  - [x] 7.2 Update `format_config_info()` in `src/viz_bevy/setup.rs` to display `levy_exponent` and `max_tumble_steps`
    - _Requirements: 10.2_
  - [x] 7.3 Update `README.md` ActorConfig parameter table with `levy_exponent` and `max_tumble_steps`
    - _Requirements: 10.4_
  - [x] 7.4 Update `.kiro/steering/config-documentation.md` ActorConfig table with `levy_exponent` and `max_tumble_steps`
    - _Requirements: 10.3_

- [x] 8. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` with minimum 100 iterations per property
- `rand_chacha` is the only new dependency; `rand` is already present
- The movement system (`run_actor_movement`) requires no changes
