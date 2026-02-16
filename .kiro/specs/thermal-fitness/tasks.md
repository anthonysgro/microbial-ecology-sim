# Implementation Plan: Thermal Fitness

## Overview

Introduce a Gaussian thermal fitness factor that multiplicatively degrades consumption efficiency and increases movement cost based on thermal mismatch. Two new config fields (`thermal_fitness_width`, `thermal_movement_cap`), a pure helper function, and modifications to two existing HOT-path systems.

## Tasks

- [x] 1. Add configuration fields and thermal fitness function
  - [x] 1.1 Add `thermal_fitness_width` and `thermal_movement_cap` fields to `ActorConfig`
    - Add fields with serde defaults (`0.5` and `5.0` respectively) and doc comments
    - Add default helper functions following existing pattern
    - Update `Default` impl
    - Add config validation: `thermal_fitness_width >= 0.0 && finite`, `thermal_movement_cap > 1.0 && finite`
    - _Requirements: 4.1, 4.2, 4.4, 4.5_

  - [x] 1.2 Implement `thermal_fitness` pure function in `src/grid/actor_systems.rs`
    - `pub(crate) fn thermal_fitness(cell_heat: f32, optimal_temp: f32, width: f32) -> f32`
    - Gaussian decay: `exp(-delta² / (2 * width²))`, returns `1.0` when `width == 0.0`
    - Mark `#[inline]` for HOT-path inlining
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

  - [ ]* 1.3 Write property test for thermal fitness function
    - **Property 1: Thermal fitness function correctness**
    - Generate random finite (cell_heat, optimal_temp, width) triples
    - Verify: output in [0,1], identity at zero mismatch, monotonic decrease, disabled when width==0
    - **Validates: Requirements 1.1, 1.2, 1.3, 4.3**

- [x] 2. Integrate thermal fitness into metabolism
  - [x] 2.1 Modify `run_actor_metabolism` to scale consumption by thermal fitness
    - Compute `fitness = thermal_fitness(heat_read[ci], actor.traits.optimal_temp, config.thermal_fitness_width)`
    - Multiply `effective_conversion` by `fitness` in the energy balance
    - Existing additive `thermal_cost` line remains unchanged
    - _Requirements: 2.1, 2.2, 2.3, 2.4_

  - [ ]* 2.2 Write property test for metabolism with thermal fitness
    - **Property 2: Metabolism energy balance with thermal fitness**
    - Generate random actor state, chemical concentration, and cell heat
    - Verify energy delta matches expected formula including both multiplicative fitness and additive thermal cost
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4**

- [x] 3. Integrate thermal fitness into movement
  - [x] 3.1 Modify `run_actor_movement` to scale cost by thermal fitness
    - Add `heat_read: &[f32]` parameter to `run_actor_movement`
    - Update all call sites to pass `heat_read`
    - Compute `fitness = thermal_fitness(heat_read[target], actor.traits.optimal_temp, config.thermal_fitness_width)`
    - Compute `capped_fitness = fitness.max(1.0 / config.thermal_movement_cap)`
    - Divide proportional cost by `capped_fitness`
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

  - [ ]* 3.2 Write property test for movement cost with thermal fitness
    - **Property 3: Movement cost with thermal fitness and cap**
    - Generate random actor state, movement target, and cell heat
    - Verify energy deducted matches expected formula including cap
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4**

- [x] 4. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Update configuration documentation and visualization
  - [x] 5.1 Update `example_config.toml` with new fields
    - Add `thermal_fitness_width` and `thermal_movement_cap` under the "Thermal Metabolism" section
    - Include explanatory comments matching existing style
    - _Requirements: 4.6, 5.2_

  - [x] 5.2 Update `format_config_info` in `src/viz_bevy/setup.rs`
    - Add `thermal_fitness_width` and `thermal_movement_cap` display lines after existing thermal fields
    - _Requirements: 4.7_

  - [x] 5.3 Update `config-documentation.md` steering file
    - Add `thermal_fitness_width` and `thermal_movement_cap` rows to the ActorConfig reference table
    - _Requirements: 5.1_

- [x] 6. Update existing tests for compatibility
  - [x] 6.1 Update `default_config()` in `actor_systems.rs` tests
    - Set `thermal_fitness_width = 0.0` in the test helper to preserve pre-feature behavior in existing tests
    - Verify all existing tests pass without modification
    - _Requirements: 6.2, 6.3_

- [x] 7. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The `thermal_fitness` function is HOT-path: zero allocation, no branching beyond the width==0 guard
- Existing tests use `heat_read` set to `optimal_temp`, so fitness == 1.0 — they should pass if `thermal_fitness_width` defaults to 0.0 in test config
- Property tests use `proptest` crate with minimum 100 iterations per property
