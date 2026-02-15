# Implementation Plan: Extraction Cost

## Overview

Add `extraction_cost: f32` to `ActorConfig`, update validation, modify the metabolism equation in `run_actor_metabolism`, and update all configuration documentation. Minimal, surgical change — no new modules, no new systems.

## Tasks

- [x] 1. Add `extraction_cost` field to `ActorConfig`
  - [x] 1.1 Add `extraction_cost: f32` field to `ActorConfig` struct in `src/grid/actor_config.rs`
    - Add field with doc comment: energy cost per unit of chemical consumed, must be in `[0.0, energy_conversion_factor)`, default `0.2`
    - Update `Default` impl to set `extraction_cost: 0.2`
    - _Requirements: 1.1, 1.2, 1.3_

  - [ ]* 1.2 Write property test for TOML round-trip of extraction_cost
    - **Property 1: TOML round-trip for extraction_cost**
    - **Validates: Requirements 1.2**

- [x] 2. Add extraction_cost validation to `validate_world_config`
  - [x] 2.1 Add validation checks in `src/io/config_file.rs` `validate_world_config`
    - Add check: `extraction_cost < 0.0` → `ConfigError::Validation` with message "extraction_cost ({value}) must be >= 0.0"
    - Add check: `extraction_cost >= energy_conversion_factor` → `ConfigError::Validation` with message "extraction_cost ({value}) must be < energy_conversion_factor ({value})"
    - Place checks in the existing `if let Some(ref actor)` block alongside `removal_threshold` and `max_energy` checks
    - _Requirements: 2.1, 2.2, 2.3_

  - [ ]* 2.2 Write property tests for extraction_cost validation
    - **Property 2: Negative extraction_cost rejected**
    - **Property 3: extraction_cost >= energy_conversion_factor rejected**
    - **Property 4: Valid extraction_cost accepted**
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [x] 3. Update metabolism equation in `run_actor_metabolism`
  - [x] 3.1 Modify active actor branch in `src/grid/actor_systems.rs` `run_actor_metabolism`
    - Change `max_useful` computation: `headroom / config.energy_conversion_factor` → `headroom / (config.energy_conversion_factor - config.extraction_cost)`
    - Change energy update: `consumed * config.energy_conversion_factor - config.base_energy_decay` → `consumed * (config.energy_conversion_factor - config.extraction_cost) - config.base_energy_decay`
    - Do NOT modify the inert actor branch
    - _Requirements: 3.1, 3.2, 4.1, 4.2_

  - [ ]* 3.2 Write property tests for metabolism with extraction cost
    - **Property 5: Active actor energy delta matches formula**
    - **Property 6: Inert actors unaffected by extraction_cost**
    - **Property 7: Energy never exceeds max_energy after metabolism**
    - **Validates: Requirements 3.1, 3.2, 3.3, 4.1, 4.2**

  - [x] 3.3 Update existing metabolism unit tests in `src/grid/actor_systems.rs`
    - Update `default_config()` helper to include `extraction_cost: 0.0` (preserves existing test behavior since 0.0 extraction cost is equivalent to the old equation)
    - Verify all existing tests still pass with the updated equation
    - _Requirements: 3.1_

- [x] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Update configuration documentation
  - [x] 5.1 Update `example_config.toml`
    - Add `extraction_cost = 0.2` in the `[actor]` section with a comment explaining purpose and valid range `[0.0, energy_conversion_factor)`
    - _Requirements: 5.1_

  - [x] 5.2 Update `format_config_info()` in `src/viz_bevy/setup.rs`
    - Add `writeln!(out, "extraction_cost: {:.4}", ac.extraction_cost).ok();` in the actor config display block
    - _Requirements: 5.2_

  - [ ]* 5.3 Write property test for info panel display
    - **Property 8: Info panel contains extraction_cost**
    - **Validates: Requirements 5.2**

  - [x] 5.4 Update `README.md` ActorConfig parameter table
    - Add row: `extraction_cost` | `f32` | Energy cost per unit of chemical consumed. Reduces net gain from consumption. Must be in `[0.0, energy_conversion_factor)` |
    - _Requirements: 5.4_

  - [x] 5.5 Update `.kiro/steering/config-documentation.md` ActorConfig table
    - Add row: `extraction_cost` | `f32` | `0.2` | Energy cost per unit of chemical consumed. Net gain = `consumed * (energy_conversion_factor - extraction_cost)`. Must be `>= 0.0` and `< energy_conversion_factor`. |
    - _Requirements: 5.3_

- [-] 6. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The `extraction_cost: 0.0` default in test helpers preserves backward compatibility with existing tests
- Validation guarantees `energy_conversion_factor - extraction_cost > 0.0`, so division in `max_useful` is safe
- This replaces the `.kiro/specs/consumption-threshold/` spec entirely
