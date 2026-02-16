# Implementation Plan: Metabolic Scaling

## Overview

Add `reference_metabolic_rate` to `ActorConfig` and modify four existing system functions to scale behavior by the per-actor metabolic ratio. No new systems, no new traits, no new components. One new config field, four formula changes, documentation updates.

## Tasks

- [x] 1. Add `reference_metabolic_rate` to ActorConfig and validation
  - [x] 1.1 Add `reference_metabolic_rate` field to `ActorConfig` in `src/grid/actor_config.rs`
    - Add `default_reference_metabolic_rate` function returning `0.05`
    - Add field with `#[serde(default = "default_reference_metabolic_rate")]`
    - Add field to `Default` impl
    - _Requirements: 1.1_
  - [x] 1.2 Add config validation for `reference_metabolic_rate` in `src/io/config_file.rs`
    - Reject values that are `<= 0.0` or not finite (`is_nan() || is_infinite()`)
    - Use existing `ConfigError::Validation` variant
    - _Requirements: 1.2, 1.3_
  - [ ]* 1.3 Write property test for config validation
    - **Property 1: Config validation rejects invalid reference_metabolic_rate**
    - **Validates: Requirements 1.2, 1.3**

- [x] 2. Modify metabolism system with consumption efficiency scaling
  - [x] 2.1 Update `run_actor_metabolism` in `src/grid/actor_systems.rs`
    - Compute `metabolic_ratio = actor.traits.base_energy_decay / config.reference_metabolic_rate`
    - Compute `effective_conversion = (config.energy_conversion_factor - config.extraction_cost) * metabolic_ratio`
    - Replace `(config.energy_conversion_factor - config.extraction_cost)` with `effective_conversion` in energy gain and `max_useful` computation
    - _Requirements: 2.1, 2.4_
  - [ ]* 2.2 Write property test for metabolism formula correctness
    - **Property 2: Metabolism formula correctness**
    - **Validates: Requirements 2.1, 2.4**
  - [ ]* 2.3 Write property test for consumption efficiency monotonicity
    - **Property 3: Consumption efficiency monotonicity**
    - **Validates: Requirements 2.2, 2.3, 6.2**

- [-] 3. Modify movement system with movement cost scaling
  - [x] 3.1 Update `run_actor_movement` in `src/grid/actor_systems.rs`
    - Compute `metabolic_ratio = actor.traits.base_energy_decay / config.reference_metabolic_rate`
    - Divide proportional movement cost by `metabolic_ratio`
    - Keep existing floor at `base_movement_cost * 0.1`
    - _Requirements: 3.1, 3.4_
  - [ ]* 3.2 Write property test for movement cost monotonicity
    - **Property 5: Movement cost monotonicity**
    - **Validates: Requirements 3.2, 3.3, 6.3**
  - [ ]* 3.3 Write property test for movement cost floor
    - **Property 6: Movement cost floor**
    - **Validates: Requirements 3.4**

- [-] 4. Modify predation system with predation power scaling
  - [x] 4.1 Update `run_contact_predation` in `src/grid/actor_systems.rs`
    - Compute `metabolic_ratio = actor.traits.base_energy_decay / config.reference_metabolic_rate`
    - Compute `effective_absorption = (config.absorption_efficiency * metabolic_ratio).min(1.0)`
    - Replace `config.absorption_efficiency` with `effective_absorption` in energy gain
    - _Requirements: 4.1, 4.4_
  - [ ]* 4.2 Write property test for predation absorption clamp
    - **Property 8: Predation absorption clamp**
    - **Validates: Requirements 4.4**
  - [ ]* 4.3 Write property test for predation monotonicity
    - **Property 9: Predation monotonicity**
    - **Validates: Requirements 4.2, 4.3, 6.4**

- [x] 5. Modify sensing system with break-even scaling
  - [x] 5.1 Update `run_actor_sensing` in `src/grid/actor_systems.rs`
    - Replace break-even formula: `config.reference_metabolic_rate / (config.energy_conversion_factor - config.extraction_cost)`
    - _Requirements: 5.1_
  - [ ]* 5.2 Write property test for break-even formula
    - **Property 10: Break-even formula correctness**
    - **Validates: Requirements 5.1**

- [x] 6. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Update documentation and visualization
  - [x] 7.1 Update `example_config.toml` with `reference_metabolic_rate` field and comment
    - _Requirements: 8.1_
  - [x] 7.2 Update `format_config_info` in `src/viz_bevy/setup.rs` to display `reference_metabolic_rate`
    - _Requirements: 8.2_
  - [x] 7.3 Update `config-documentation.md` steering file with new field in ActorConfig reference table
    - _Requirements: 8.3_

- [ ]* 8. Write property test for no NaN/Infinity from valid inputs
  - **Property 11: No NaN or Infinity from valid inputs**
  - Generate random valid metabolic rates within clamp range and verify all derived scaling values are finite
  - **Validates: Requirements 6.6**

- [x] 9. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- No new heritable traits → no TraitStats/visualization changes needed
- The break-even formula simplifies to be independent of individual actor metabolic rate after algebraic cancellation
