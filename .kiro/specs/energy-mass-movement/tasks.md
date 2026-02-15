# Implementation Plan: Energy-Mass Movement

## Overview

Replace the flat `movement_cost` with energy-proportional movement physics. The change is localized: config struct, movement system function, call site, visualization, and documentation. All existing tests must continue to pass with updated config field names.

## Tasks

- [-] 1. Update `ActorConfig` with new fields
  - [-] 1.1 Replace `movement_cost` with `base_movement_cost` and add `reference_energy` in `src/grid/actor_config.rs`
    - Remove `movement_cost: f32` field
    - Add `base_movement_cost: f32` (default 0.5) with serde default function
    - Add `reference_energy: f32` (default 25.0) with serde default function
    - Update `Default` impl accordingly
    - _Requirements: 3.1, 3.2, 3.5_
  - [ ] 1.2 Add config validation for new fields
    - Add post-deserialization validation: `reference_energy > 0.0`, `base_movement_cost >= 0.0`
    - Return descriptive error on invalid values, consistent with existing validation patterns
    - _Requirements: 3.3, 3.4_
  - [ ]* 1.3 Write property test for config validation (Property 2)
    - **Property 2: Configuration validation rejects invalid parameters**
    - Generate random invalid `reference_energy` (<= 0.0) and `base_movement_cost` (< 0.0), assert validation rejects them
    - **Validates: Requirements 3.3, 3.4**
  - [ ]* 1.4 Write property test for config TOML round-trip (Property 3)
    - **Property 3: Configuration TOML round-trip**
    - Generate random valid `ActorConfig`, serialize to TOML, parse back, assert `base_movement_cost` and `reference_energy` match
    - **Validates: Requirements 3.5**

- [ ] 2. Implement energy-proportional movement cost in `run_actor_movement`
  - [ ] 2.1 Update `run_actor_movement` signature and implementation in `src/grid/actor_systems.rs`
    - Change fourth parameter from `movement_cost: f32` to `actor_config: &ActorConfig`
    - Compute `base` and `reference` from config outside the loop
    - Compute `floor = base * 0.1` outside the loop
    - Inside the loop after successful move: `let proportional = base * (actor.energy / reference); let actual = if proportional > floor { proportional } else { floor };`
    - Deduct `actual` from `actor.energy` instead of flat cost
    - Preserve existing NaN/Inf check and inert transition
    - _Requirements: 1.1, 1.5, 2.1, 2.3, 4.1, 5.1, 5.2_
  - [ ] 2.2 Update call site in `src/grid/tick.rs`
    - Change `run_actor_movement(&mut actors, &mut occupancy, &movement_targets, actor_config.movement_cost)` to `run_actor_movement(&mut actors, &mut occupancy, &movement_targets, &actor_config)`
    - _Requirements: 1.1_
  - [ ]* 2.3 Write property test for movement cost formula (Property 1)
    - **Property 1: Movement cost formula correctness**
    - Generate random `(energy, base_movement_cost, reference_energy)` tuples, construct minimal actor + config, run movement, verify `energy_after == energy_before - max(base * (energy / ref), base * 0.1)`
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3**
  - [ ]* 2.4 Write property test for inert transition (Property 4)
    - **Property 4: Inert transition on energy depletion**
    - Generate random actors with energy near the deduction threshold, run movement, verify inert flag set when energy <= 0
    - **Validates: Requirements 5.1**
  - [ ]* 2.5 Write property test for inert immobility (Property 5)
    - **Property 5: Inert actors are immobile**
    - Generate random inert actors with movement targets, run movement, verify cell_index and energy unchanged
    - **Validates: Requirements 5.2**

- [ ] 3. Checkpoint — Ensure all tests pass
  - Fix any compilation errors from the field rename (`movement_cost` → `base_movement_cost`) across the codebase
  - Run `cargo test` and ensure all existing and new tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Update documentation and visualization
  - [ ] 4.1 Update `example_config.toml`
    - Replace `movement_cost = 0.1` with `base_movement_cost = 0.1` and add `reference_energy = 25.0` with explanatory comments
    - _Requirements: 6.1, 6.2_
  - [ ] 4.2 Update `format_config_info` in `src/viz_bevy/setup.rs`
    - Replace `movement_cost` display line with `base_movement_cost` and `reference_energy` lines
    - _Requirements: 6.3, 6.4_
  - [ ] 4.3 Update `config-documentation.md` steering file
    - Replace `movement_cost` row in the `[actor]` table with `base_movement_cost` and `reference_energy` rows, including type, default, and description
    - _Requirements: 6.5_

- [ ] 5. Final checkpoint — Ensure all tests pass
  - Run `cargo test` and `cargo clippy -- -D warnings`
  - Verify `example_config.toml` parses without error
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The field rename from `movement_cost` to `base_movement_cost` will cause compile errors in any code referencing the old field — task 3 catches these
- Property tests use `proptest` crate with minimum 256 iterations per property
- The `deny_unknown_fields` on `ActorConfig` automatically handles Requirement 7.1 (old `movement_cost` field rejection) — no code change needed beyond the field rename
