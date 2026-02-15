# Implementation Plan: Source Config Decoupling

## Overview

Extract all source-generation parameters from `WorldInitConfig` into a reusable `SourceFieldConfig` struct. Update `WorldInitConfig` to hold one `SourceFieldConfig` per fundamental (heat, chemical). Refactor `validate_config`, `generate_sources`, and `sample_reservoir_params` to operate on `SourceFieldConfig`. Update `main.rs` call site. All changes are in `src/grid/world_init.rs` and `src/main.rs`.

## Tasks

- [x] 1. Define `SourceFieldConfig` and refactor `WorldInitConfig`
  - [x] 1.1 Add `SourceFieldConfig` struct to `src/grid/world_init.rs`
    - Define struct with fields: `min_sources`, `max_sources`, `min_emission_rate`, `max_emission_rate`, `renewable_fraction`, `min_reservoir_capacity`, `max_reservoir_capacity`, `min_deceleration_threshold`, `max_deceleration_threshold`
    - Derive `Debug, Clone, PartialEq`
    - No `Default` impl on `SourceFieldConfig` (source count has no universal default)
    - _Requirements: 1.1, 1.5_
  - [x] 1.2 Replace shared fields on `WorldInitConfig` with `heat_source_config` and `chemical_source_config`
    - Remove the 11 shared source fields
    - Add `heat_source_config: SourceFieldConfig` and `chemical_source_config: SourceFieldConfig`
    - Keep `min_initial_heat`, `max_initial_heat`, `min_initial_concentration`, `max_initial_concentration`, `min_actors`, `max_actors` unchanged
    - _Requirements: 1.2, 1.3, 1.4_
  - [x] 1.3 Update `Default` impl for `WorldInitConfig`
    - Heat: source count `[1, 5]`, emission rate `[0.1, 5.0]`, renewable fraction `0.3`, reservoir capacity `[50.0, 200.0]`, deceleration threshold `[0.1, 0.5]`
    - Chemical: source count `[1, 3]`, emission rate `[0.1, 5.0]`, renewable fraction `0.3`, reservoir capacity `[50.0, 200.0]`, deceleration threshold `[0.1, 0.5]`
    - _Requirements: 4.1, 4.2, 4.3_

- [x] 2. Refactor validation and generation functions
  - [x] 2.1 Extract `validate_source_field_config` helper
    - Signature: `fn validate_source_field_config(config: &SourceFieldConfig, field_label: &'static str) -> Result<(), WorldInitError>`
    - Validate all ranges and bounds, prefixing error field names with `field_label` (e.g., `"heat_emission_rate"`)
    - Call from `validate_config` for both `heat_source_config` and `chemical_source_config`
    - Keep non-source validations (initial_heat, initial_concentration, actors) unchanged
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_
  - [x] 2.2 Update `sample_reservoir_params` signature
    - Change from `(&mut impl Rng, &WorldInitConfig, f64)` to `(&mut impl Rng, &SourceFieldConfig, f64)`
    - Body reads from `SourceFieldConfig` fields (same field names, just different source struct)
    - _Requirements: 5.2_
  - [x] 2.3 Update `generate_sources` to use per-field configs
    - Heat loop: read count, emission rate, renewable_prob from `config.heat_source_config`
    - Chemical loop: read count, emission rate, renewable_prob from `config.chemical_source_config`
    - Pass the appropriate `SourceFieldConfig` to `sample_reservoir_params`
    - _Requirements: 3.1, 3.2, 3.3_

- [ ] 3. Update call sites
  - [ ] 3.1 Update `main.rs` to use new `WorldInitConfig` layout
    - Construct `WorldInitConfig` with `heat_source_config` and `chemical_source_config` fields
    - _Requirements: 5.1_
  - [ ] 3.2 Ensure `SourceFieldConfig` is exported from the module's public API
    - Add `pub use` or ensure visibility so `main.rs` and external consumers can access it
    - _Requirements: 5.3_

- [ ] 4. Checkpoint
  - Ensure the codebase compiles without errors or warnings. Run `cargo clippy`. Ask the user if questions arise.

- [ ]* 5. Property tests
  - [ ]* 5.1 Write property test: validation rejects invalid sub-configs independently
    - **Property 1: Validation rejects invalid sub-configs independently**
    - Generate arbitrary `SourceFieldConfig` pairs where one is valid and one has an invalid range. Assert `validate_config` returns `Err` with the correct field type label.
    - Use `proptest` with minimum 100 iterations
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8**
  - [ ]* 5.2 Write property test: generated source parameters fall within their SourceFieldConfig ranges
    - **Property 2: Generated source parameters fall within their corresponding SourceFieldConfig ranges**
    - Generate valid `WorldInitConfig` with non-overlapping heat and chemical ranges. Run `generate_sources` with a random seed. Assert every heat source's parameters fall within heat config ranges and every chemical source's parameters fall within chemical config ranges.
    - Use `proptest` with minimum 100 iterations
    - **Validates: Requirements 3.1, 3.2, 3.3**
  - [ ]* 5.3 Write property test: default config backward compatibility
    - **Property 3: Default config backward compatibility**
    - For any seed, verify `WorldInitConfig::default()` field values match the expected constants and that `initialize` produces deterministic output.
    - Use `proptest` with minimum 100 iterations
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4**

- [ ]* 6. Unit tests
  - [ ]* 6.1 Write unit tests for validation edge cases
    - Test each specific invalid condition from Requirement 2 (one assertion per condition)
    - Test that a fully valid default config passes validation
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

- [ ] 7. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- All changes are COLD-path only — no hot-path code is affected
- No new modules, no new dependencies (proptest may already be in dev-dependencies)
- Adding a future fundamental requires only adding another `SourceFieldConfig` field to `WorldInitConfig` and a generation loop in `generate_sources`
