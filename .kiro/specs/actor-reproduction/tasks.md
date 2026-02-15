# Implementation Plan: Actor Reproduction (Binary Fission)

## Overview

Add binary fission reproduction to the actor simulation. Extend `ActorConfig` with reproduction fields, add config validation, implement the reproduction and deferred spawn systems, wire them into the tick pipeline, and update all configuration documentation.

## Tasks

- [x] 1. Extend ActorConfig with reproduction fields
  - [x] 1.1 Add `reproduction_threshold`, `reproduction_cost`, and `offspring_energy` fields to `ActorConfig` in `src/grid/actor_config.rs`
    - Add fields with `pub` visibility, serde defaults, and doc comments
    - Update `Default` impl: `reproduction_threshold: 20.0`, `reproduction_cost: 12.0`, `offspring_energy: 10.0`
    - _Requirements: 3.3, 3.4, 3.5_

  - [x] 1.2 Add reproduction config validation to `Grid::new` in `src/grid/mod.rs`
    - Validate: threshold > 0, cost > 0, offspring_energy > 0, cost >= offspring_energy, offspring_energy <= max_energy, threshold >= cost
    - Use existing `GridError::InvalidActorConfig` variant for all errors
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

  - [ ]* 1.3 Write property test for config validation
    - **Property 7: Config validation rejects invalid reproduction parameters**
    - **Validates: Requirements 3.3, 3.4, 3.5, 9.1, 9.2, 9.3, 9.4, 9.5, 9.6**

- [x] 2. Add spawn buffer to Grid
  - [x] 2.1 Add `spawn_buffer: Vec<(usize, f32)>` field to `Grid` struct in `src/grid/mod.rs`
    - Pre-allocate to `initial_actor_capacity` in `Grid::new`
    - Extend `take_actors` return tuple and `put_actors` parameters to include `spawn_buffer`
    - _Requirements: 5.3_

- [x] 3. Implement reproduction and deferred spawn systems
  - [x] 3.1 Implement `run_actor_reproduction` in `src/grid/actor_systems.rs`
    - Iterate actors in slot-index order via `iter_mut_with_ids()`
    - Check eligibility: not inert, energy >= threshold
    - Scan N/S/W/E via `direction_to_target`, check occupancy and spawn buffer for collisions
    - Deduct `reproduction_cost` from parent, push `(target_cell, offspring_energy)` to spawn buffer
    - NaN/Inf check on parent energy after deduction
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 7.1, 7.2, 7.3_

  - [x] 3.2 Implement `run_deferred_spawn` in `src/grid/actor_systems.rs`
    - Iterate spawn buffer in insertion order
    - Construct offspring Actor with `inert: false`, `tumble_remaining: 0`, `tumble_direction: 0`
    - Call `actors.add(actor, cell_count, occupancy)`, map errors to `TickError`
    - Clear spawn buffer
    - _Requirements: 4.1, 4.2, 4.3, 5.1, 5.2_

  - [ ]* 3.3 Write property test for eligibility correctness
    - **Property 1: Eligibility correctness**
    - **Validates: Requirements 1.1, 1.2, 1.3**

  - [ ]* 3.4 Write property test for placement direction correctness
    - **Property 2: Placement direction correctness**
    - **Validates: Requirements 2.1, 2.2, 2.3, 7.2**

  - [ ]* 3.5 Write property test for energy conservation on fission
    - **Property 3: Energy conservation on fission**
    - **Validates: Requirements 3.1, 3.2**

  - [ ]* 3.6 Write property test for offspring initial state
    - **Property 4: Offspring initial state**
    - **Validates: Requirements 4.1, 4.2, 4.3**

  - [ ]* 3.7 Write property test for no duplicate spawn targets
    - **Property 5: No duplicate spawn targets**
    - **Validates: Requirements 7.3**

  - [ ]* 3.8 Write property test for deferred spawn occupancy consistency
    - **Property 7: Deferred spawn occupancy consistency**
    - **Validates: Requirements 5.2**

  - [ ]* 3.9 Write unit tests for edge cases
    - Corner cell (0,0) on 3x3 grid: only South and East valid
    - Fully surrounded actor: reproduction blocked, energy unchanged
    - Exact threshold energy: should reproduce
    - Inert actor with high energy: skipped
    - _Requirements: 1.3, 2.2, 2.3_

- [x] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Wire reproduction into the tick pipeline
  - [x] 5.1 Insert reproduction and deferred spawn calls into `run_actor_phases` in `src/grid/tick.rs`
    - Call `run_actor_reproduction` after deferred removal, before movement
    - Call `run_deferred_spawn` immediately after reproduction
    - Pass spawn_buffer through the take/put_actors flow
    - _Requirements: 6.1, 6.2_

- [x] 6. Update configuration documentation
  - [x] 6.1 Update `example_config.toml` with new reproduction fields and comments
    - Add `reproduction_threshold`, `reproduction_cost`, `offspring_energy` under `[actor]`
    - Include comments explaining purpose and valid ranges
    - _Requirements: 10.1_

  - [x] 6.2 Update `README.md` with new configuration fields
    - Add reproduction fields to the `[actor]` configuration reference table
    - _Requirements: 10.2_

  - [x] 6.3 Update `format_config_info()` in `src/viz_bevy/setup.rs`
    - Display reproduction_threshold, reproduction_cost, offspring_energy in the info panel
    - _Requirements: 10.3_

  - [x] 6.4 Update `config-documentation.md` steering file
    - Add reproduction fields to the `[actor]` — `ActorConfig` table
    - _Requirements: 10.4_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- The reproduction system reuses the existing `direction_to_target` helper for neighbor computation
- Spawn buffer follows the same deferred pattern as the existing removal buffer
