# Implementation Plan: Source Depletion

## Overview

Extend the `Source` struct with reservoir, initial_capacity, and deceleration_threshold fields. Modify `run_emission` to compute effective emission rates, drain reservoirs, and skip depleted sources. Update `WorldInitConfig` and `generate_sources` to parameterize renewability. All changes are in `src/grid/source.rs`, `src/grid/world_init.rs`, and `src/grid/tick.rs`.

## Tasks

- [x] 1. Extend Source struct and SourceError with reservoir fields
  - [x] 1.1 Add `reservoir`, `initial_capacity`, and `deceleration_threshold` fields to `Source` in `src/grid/source.rs`
    - `reservoir: f32` — remaining emittable quantity, `f32::INFINITY` for renewable
    - `initial_capacity: f32` — total at creation, `f32::INFINITY` for renewable
    - `deceleration_threshold: f32` — fraction in [0.0, 1.0]
    - _Requirements: 1.1, 1.2, 3.1_
  - [x] 1.2 Add `InvalidReservoir` and `InvalidDecelerationThreshold` variants to `SourceError`
    - _Requirements: 1.3_
  - [x] 1.3 Add validation in `SourceRegistry::add()` for the new fields
    - Reject finite reservoir ≤ 0.0, initial_capacity ≤ 0.0, reservoir > initial_capacity, threshold outside [0.0, 1.0]
    - Allow `f32::INFINITY` for both reservoir and initial_capacity (renewable)
    - _Requirements: 1.3_
  - [ ]* 1.4 Write property test: invalid reservoir rejection (Property 1)
    - **Property 1: Invalid reservoir rejection**
    - **Validates: Requirements 1.3**
  - [x] 1.5 Fix all existing call sites that construct `Source` to include the new fields
    - Update `generate_sources` in `src/grid/world_init.rs` to pass default renewable values (`f32::INFINITY`) temporarily
    - Update any test code that constructs `Source` directly
    - _Requirements: 1.1, 1.2_

- [-] 2. Implement `iter_mut`, `is_depleted`, and `active_emitting_count` on SourceRegistry
  - [-] 2.1 Add `iter_mut()` method returning `impl Iterator<Item = &mut Source>` over active slots
    - _Requirements: 2.1_
  - [-] 2.2 Add `is_depleted(id: SourceId) -> Result<bool, SourceError>` method
    - Returns true if source exists and reservoir == 0.0
    - _Requirements: 4.3_
  - [-] 2.3 Add `active_emitting_count()` method returning count of sources with reservoir > 0.0
    - _Requirements: 4.4_

- [ ] 3. Modify `run_emission` to support depletion and deceleration
  - [ ] 3.1 Change `run_emission` signature to take `&mut SourceRegistry` instead of `&SourceRegistry`
    - Update call site in `run_emission_phase` in `src/grid/tick.rs`
    - _Requirements: 2.1_
  - [ ] 3.2 Implement effective emission rate computation with deceleration
    - If `reservoir > threshold * initial_capacity`: effective_rate = emission_rate
    - Else if `threshold > 0.0`: effective_rate = emission_rate * (reservoir / (threshold * initial_capacity))
    - Else: effective_rate = emission_rate
    - Clamp actual emission to min(effective_rate, reservoir)
    - Subtract actual emission from reservoir
    - Skip sources with reservoir == 0.0 (but not INFINITY)
    - _Requirements: 2.1, 2.2, 2.3, 3.2, 3.3, 3.4, 4.2_
  - [ ]* 3.3 Write property test: renewable source invariance (Property 2)
    - **Property 2: Renewable source invariance**
    - **Validates: Requirements 1.4, 2.4**
  - [ ]* 3.4 Write property test: non-renewable emission drains reservoir (Property 3)
    - **Property 3: Non-renewable emission drains reservoir**
    - **Validates: Requirements 2.1**
  - [ ]* 3.5 Write property test: depleted sources produce zero emission (Property 4)
    - **Property 4: Depleted sources produce zero emission**
    - **Validates: Requirements 2.2, 4.1, 4.2**
  - [ ]* 3.6 Write property test: full rate above deceleration threshold (Property 5)
    - **Property 5: Full rate above deceleration threshold**
    - **Validates: Requirements 3.2**
  - [ ]* 3.7 Write property test: decelerated rate below threshold (Property 6)
    - **Property 6: Decelerated rate below threshold**
    - **Validates: Requirements 3.3**
  - [ ]* 3.8 Write unit tests for edge cases
    - Threshold = 0.0 (no deceleration)
    - Reservoir exactly equal to one tick emission
    - Reservoir less than one tick emission (partial emit, then depleted)
    - Negative emission_rate with finite reservoir
    - _Requirements: 2.3, 3.4_

- [ ] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Update WorldInitConfig and generate_sources for reservoir parameterization
  - [ ] 5.1 Add `renewable_fraction`, `min_reservoir_capacity`, `max_reservoir_capacity`, `min_deceleration_threshold`, `max_deceleration_threshold` fields to `WorldInitConfig`
    - Update `Default` impl with sensible defaults (e.g., renewable_fraction = 0.3, reservoir range [50.0, 200.0], threshold range [0.1, 0.5])
    - _Requirements: 5.1, 5.2, 5.3_
  - [ ] 5.2 Add validation for new fields in `validate_config`
    - renewable_fraction in [0.0, 1.0], min_reservoir > 0.0, max >= min for both ranges, thresholds in [0.0, 1.0]
    - _Requirements: 5.1, 5.2, 5.3_
  - [ ] 5.3 Modify `generate_sources` to assign renewability and reservoir parameters per source
    - For each source: sample `rng.random_bool(renewable_fraction)` to decide renewable vs finite
    - Renewable: set reservoir and initial_capacity to `f32::INFINITY`, deceleration_threshold to 0.0
    - Finite: sample reservoir from [min, max], set initial_capacity = reservoir, sample threshold from [min, max]
    - _Requirements: 5.4, 5.5, 5.6_
  - [ ]* 5.4 Write property test: generated source parameters within configured range (Property 7)
    - **Property 7: Generated source parameters within configured range**
    - **Validates: Requirements 5.5, 5.6**
  - [ ]* 5.5 Write property test: renewable fraction approximation (Property 8)
    - **Property 8: Renewable fraction approximation**
    - **Validates: Requirements 5.4**

- [ ] 6. Determinism verification
  - [ ]* 6.1 Write property test: deterministic emission (Property 9)
    - **Property 9: Deterministic emission**
    - Run emission for N ticks on two identical grids with identical source configs, verify identical reservoir and field state
    - **Validates: Requirements 6.1**

- [~] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The `f32::INFINITY` approach for renewable sources eliminates branching in the emission loop — the deceleration math works identically for both renewable and finite sources
- Existing `run_emission_phase` structure (copy-read-to-write → emit → clamp → validate → swap) is unchanged; only the inner emission logic changes
- `proptest` must be added as a dev-dependency if not already present
