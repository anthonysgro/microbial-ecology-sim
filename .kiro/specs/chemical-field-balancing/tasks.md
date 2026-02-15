# Implementation Plan: Chemical Field Balancing

## Overview

Add per-species chemical decay and rebalance actor consumption to restore chemical field equilibrium. Changes touch config structs, error types, a new decay system, tick orchestration, and default values in both binaries.

## Tasks

- [x] 1. Extend GridConfig and error types for decay rates
  - [x] 1.1 Add `chemical_decay_rates: Vec<f32>` field to `GridConfig` in `src/grid/config.rs`
    - One entry per chemical species, each in [0.0, 1.0]
    - _Requirements: 1.1_
  - [x] 1.2 Add `DecayRateCountMismatch` and `InvalidDecayRate` variants to `GridError` in `src/grid/error.rs`
    - _Requirements: 1.4_
  - [x] 1.3 Add validation in `Grid::new()` (`src/grid/mod.rs`): check `chemical_decay_rates.len() == num_chemicals` and each rate in [0.0, 1.0], return new `GridError` variants on failure
    - _Requirements: 1.2, 1.3, 1.4_
  - [x] 1.4 Update all existing `GridConfig` construction sites to include `chemical_decay_rates` field
    - `src/main.rs`, `src/bin/bevy_viz.rs`, `src/grid/world_init.rs`, and any test files that construct `GridConfig`
    - Use `vec![0.05; num_chemicals]` as the default decay rate (5% per tick)
    - _Requirements: 1.1, 1.2_
  - [ ]* 1.5 Write property test for invalid decay config rejection (Property 1)
    - **Property 1: Invalid decay config rejects construction**
    - Generate `num_chemicals` in [1, 8], then `chemical_decay_rates` with wrong length or out-of-range values. Verify `Grid::new()` returns the expected error variant.
    - **Validates: Requirements 1.2, 1.3, 1.4**

- [x] 2. Implement the chemical decay system
  - [x] 2.1 Create `src/grid/decay.rs` with `run_decay(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError>`
    - HOT PATH classification. For each species with `decay_rate > 0.0`: copy read→write, multiply write buffer by `(1.0 - decay_rate)`, clamp to >= 0.0. Skip species with rate == 0.0.
    - _Requirements: 2.1, 2.4, 2.5, 2.6_
  - [x] 2.2 Register `decay` module in `src/grid/mod.rs`
    - Add `pub mod decay;` to the module declarations
    - _Requirements: 2.1_
  - [x] 2.3 Insert decay phase into `TickOrchestrator::step()` in `src/grid/tick.rs`
    - After diffusion validate+swap, before heat: call `run_decay`, validate write buffers, swap chemicals
    - _Requirements: 2.2, 2.3_
  - [ ]* 2.4 Write property test for decay computation correctness (Property 2)
    - **Property 2: Decay computation correctness**
    - Generate small grid (4×4 to 16×16), random concentrations in [0.0, 100.0], random decay rates in (0.0, 1.0]. Run `run_decay`, compare each cell to `original * (1 - rate)` within floating-point tolerance.
    - **Validates: Requirements 2.1, 2.6, 4.3**
  - [ ]* 2.5 Write property test for zero-rate species unchanged (Property 3)
    - **Property 3: Zero-rate species are unchanged**
    - Generate grid with 2+ species, mix of zero and non-zero decay rates. Verify zero-rate species are bitwise identical after decay.
    - **Validates: Requirements 1.5, 2.5**
  - [ ]* 2.6 Write unit tests for decay edge cases
    - Rate 1.0 zeroes all concentrations
    - NaN input triggers `TickError::NumericalError`
    - _Requirements: 2.3, 2.6_

- [x] 3. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Rebalance actor consumption defaults
  - [x] 4.1 Update `consumption_rate` from `0.1` to `1.5` in `src/main.rs` ActorConfig construction
    - _Requirements: 3.1, 3.3_
  - [x] 4.2 Update `consumption_rate` from `0.1` to `1.5` in `src/bin/bevy_viz.rs` ActorConfig construction
    - _Requirements: 3.1, 3.3_
  - [ ]* 4.3 Write unit test verifying default consumption_rate value
    - Construct ActorConfig with the values from main.rs and verify `consumption_rate == 1.5`
    - _Requirements: 3.1, 3.3_

- [ ] 5. Integration: bounded convergence validation
  - [ ]* 5.1 Write property test for bounded convergence (Property 4)
    - **Property 4: Bounded convergence under constant emission**
    - Generate small grid with 1–3 sources, constant emission, non-zero decay. Run 200 ticks. Verify max concentration is bounded and not growing monotonically.
    - **Validates: Requirements 4.1, 4.2**

- [x] 6. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` crate (already in dev-dependencies) with minimum 100 iterations
- The decay system follows the existing double-buffer discipline: copy read→write, compute, validate, swap
- HOT path classification for `run_decay` — zero allocations, no dynamic dispatch, deterministic species-index iteration
