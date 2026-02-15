# Implementation Plan: Actor Energy Costs

## Overview

Add movement energy costs and an inert actor state to the simulation. Modifies `Actor`, `ActorConfig`, `GridError`, and the three actor systems (sensing, metabolism, movement). All changes are in existing files — no new modules.

## Tasks

- [x] 1. Extend data models with inert state and new config fields
  - [x] 1.1 Add `inert: bool` field to `Actor` struct in `src/grid/actor.rs`
    - Initialize to `false` in all existing construction sites
    - Update any existing tests that construct `Actor` literals
    - _Requirements: 2.7_

  - [x] 1.2 Add `movement_cost: f32` and `removal_threshold: f32` fields to `ActorConfig` in `src/grid/actor_config.rs`
    - _Requirements: 1.3, 2.6_

  - [x] 1.3 Add `InvalidActorConfig` variant to `GridError` in `src/grid/error.rs`
    - Fields: `field: &'static str`, `value: f32`, `reason: &'static str`
    - Add `Display` impl arm matching existing pattern
    - _Requirements: 4.1, 4.2, 4.3_

  - [x] 1.4 Add config validation to `Grid::new` in `src/grid/mod.rs`
    - Validate `movement_cost >= 0.0`, `removal_threshold <= 0.0`, `base_energy_decay >= 0.0` when `actor_config` is `Some`
    - Return `GridError::InvalidActorConfig` on failure
    - _Requirements: 4.1, 4.2, 4.3_

  - [ ]* 1.5 Write unit tests for config validation
    - Test negative `movement_cost` → error
    - Test positive `removal_threshold` → error
    - Test negative `base_energy_decay` → error
    - Test valid config → Ok
    - _Requirements: 4.1, 4.2, 4.3_

- [-] 2. Modify metabolism system for inert state transitions
  - [x] 2.1 Update `run_actor_metabolism` in `src/grid/actor_systems.rs`
    - For active actors: existing consumption + energy balance logic, but instead of pushing to `removal_buffer` when `energy <= 0`, set `actor.inert = true`
    - For inert actors: skip chemical consumption, subtract only `base_energy_decay`, push to `removal_buffer` when `energy <= removal_threshold`
    - Pass `removal_threshold` from `ActorConfig` (add parameter or pass full config — config ref is already a parameter)
    - Preserve existing NaN/Inf validation
    - _Requirements: 2.1, 2.3, 2.5, 3.2_

  - [ ]* 2.2 Write property test: metabolism-induced energy depletion sets inert (Property 3)
    - **Property 3: Metabolism-induced energy depletion sets inert**
    - Generate active actors with energy near zero, run metabolism, verify `inert == true` and actor not in removal buffer
    - **Validates: Requirements 2.1**

  - [ ]* 2.3 Write property test: inert actors lose only basal cost (Property 5)
    - **Property 5: Inert actors lose only basal cost with no chemical consumption**
    - Generate inert actors, run metabolism, verify energy decreased by exactly `base_energy_decay` and chemical buffer unchanged at actor's cell
    - **Validates: Requirements 2.3**

  - [ ]* 2.4 Write property test: inert actors below threshold are removed (Property 6)
    - **Property 6: Inert actors below removal threshold are scheduled for removal**
    - Generate inert actors with energy near `removal_threshold`, run metabolism, verify removal buffer contains the actor
    - **Validates: Requirements 2.5**

- [x] 3. Modify sensing system to skip inert actors
  - [x] 3.1 Update `run_actor_sensing` in `src/grid/actor_systems.rs`
    - Add `if actor.inert { movement_targets[slot_index] = None; continue; }` at the top of the iteration loop
    - _Requirements: 2.2_

  - [ ]* 3.2 Write property test: inert actors do not sense or move (Property 4)
    - **Property 4: Inert actors do not sense or move**
    - Generate a mix of inert and active actors, run sensing, verify all inert actors have `movement_targets[slot] == None`
    - **Validates: Requirements 2.2, 2.4**

- [-] 4. Modify movement system to deduct energy and handle inert transition
  - [x] 4.1 Update `run_actor_movement` in `src/grid/actor_systems.rs`
    - Add `movement_cost: f32` parameter
    - Skip inert actors (`if actor.inert { continue; }`)
    - After successful move, subtract `movement_cost` from `actor.energy`
    - If `energy <= 0.0` after subtraction, set `actor.inert = true`
    - Add NaN/Inf validation after energy subtraction, return `TickError::NumericalError`
    - Change return type to `Result<(), TickError>`
    - _Requirements: 1.1, 1.2, 1.4, 2.4, 3.1, 3.2_

  - [ ]* 4.2 Write property test: movement cost applied iff actor moved (Property 1)
    - **Property 1: Movement cost applied if and only if actor moved**
    - Generate actors with movement targets (some valid, some blocked), run movement, verify energy change matches whether cell_index changed
    - **Validates: Requirements 1.1, 1.2**

  - [ ]* 4.3 Write property test: movement-induced energy depletion sets inert (Property 2)
    - **Property 2: Movement-induced energy depletion sets inert**
    - Generate active actors with energy just above zero and movement_cost that pushes them to <= 0, run movement, verify `inert == true`
    - **Validates: Requirements 1.4**

  - [ ]* 4.4 Write property test: NaN/Inf energy triggers error (Property 7)
    - **Property 7: NaN/Inf energy triggers numerical error**
    - Generate actors with NaN or Inf energy, run movement with valid movement_cost, verify `Err(TickError::NumericalError)` returned
    - **Validates: Requirements 3.2**

- [x] 5. Update tick orchestrator and fix all call sites
  - [x] 5.1 Update `run_actor_phases` in `src/grid/tick.rs`
    - Pass `actor_config.movement_cost` to `run_actor_movement`
    - Handle the new `Result` return from `run_actor_movement` with `?`
    - Phase order remains: Sensing → Metabolism → Deferred Removal → Movement
    - _Requirements: 3.3_

  - [x] 5.2 Update all existing tests that construct `Actor` or `ActorConfig`
    - Add `inert: false` to all `Actor` literals
    - Add `movement_cost` and `removal_threshold` to all `ActorConfig` literals
    - Ensure existing tests still pass with the new fields
    - _Requirements: 1.3, 2.6, 2.7_

- [-] 6. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- All changes are in existing files — no new modules or crates
- `proptest` crate must be added as a dev-dependency if not already present
- Property tests use `proptest!` macro blocks in `#[cfg(test)]` modules
- Each property test references its design document property number
