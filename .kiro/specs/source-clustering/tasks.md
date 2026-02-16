# Implementation Plan: Source Clustering

## Overview

Add `source_clustering` field to `SourceFieldConfig`, implement the clustered placement algorithm in `generate_sources`, update validation, documentation, and tests. All changes are COLD-path (world init only).

## Tasks

- [x] 1. Add `source_clustering` field to `SourceFieldConfig`
  - [x] 1.1 Add `source_clustering: f32` field to `SourceFieldConfig` struct in `src/grid/world_init.rs`
    - Add the field with doc comment: `/// Spatial clustering of sources. 0.0 = uniform random, 1.0 = tight clusters. Range: [0.0, 1.0]. Default: 0.0.`
    - Update `Default` impl to set `source_clustering: 0.0`
    - _Requirements: 1.1, 4.2_

  - [x] 1.2 Add validation for `source_clustering` in `validate_source_field_config`
    - Add label fields to `SourceFieldLabels` for `source_clustering` range and finiteness errors
    - Add corresponding entries to `HEAT_LABELS` and `CHEMICAL_LABELS`
    - Validate `source_clustering` is in `[0.0, 1.0]` and is finite
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ]* 1.3 Write property test for validation rejection
    - **Property 5: Validation rejects out-of-range values**
    - **Validates: Requirements 3.1, 3.2, 3.3**

- [x] 2. Implement clustered placement algorithm
  - [x] 2.1 Implement `sample_clustered_position` helper function in `src/grid/world_init.rs`
    - Private function taking `rng`, `center_col`, `center_row`, `width`, `height`, `source_clustering`
    - At `source_clustering == 0.0`: return uniform random cell index
    - Compute `sigma = max(width, height) as f32 * (1.0 - source_clustering)`
    - Sample 2D normal offsets, apply toroidal wrapping via `rem_euclid`
    - Return flat cell index `row * width + col`
    - _Requirements: 1.2, 1.3, 1.4, 2.1, 2.2_

  - [x] 2.2 Modify `generate_sources` to use clustered placement
    - For each source batch (heat, each chemical species): sample a cluster center `(col, row)` before the source loop
    - Replace `rng.random_range(0..cell_count)` with `sample_clustered_position(rng, center_col, center_row, width, height, cfg.source_clustering)`
    - Pass grid `width` and `height` into `generate_sources` (available from `Grid`)
    - _Requirements: 1.2, 1.3, 1.4, 2.1, 2.3, 2.4, 6.1, 6.2_

  - [ ]* 2.3 Write property test for tight clustering at maximum
    - **Property 1: Tight clustering at maximum**
    - **Validates: Requirements 1.3, 2.4**

  - [ ]* 2.4 Write property test for monotonic spread
    - **Property 2: Monotonic spread**
    - **Validates: Requirements 1.4**

  - [ ]* 2.5 Write property test for valid cell indices
    - **Property 3: All positions are valid cell indices**
    - **Validates: Requirements 2.2**

  - [ ]* 2.6 Write property test for deterministic placement
    - **Property 4: Deterministic placement**
    - **Validates: Requirements 2.3, 6.1**

- [x] 3. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Update TOML serialization and documentation
  - [x] 4.1 Update `example_config.toml`
    - Add `source_clustering = 0.0` to `[world_init.heat_source_config]` with comment
    - Add `source_clustering = 0.0` to `[world_init.chemical_source_config]` with comment
    - _Requirements: 5.1_

  - [x] 4.2 Update `format_config_info` in `src/viz_bevy/setup.rs`
    - Add `source_clustering` display line for heat source config section
    - Add `source_clustering` display line for chemical source config section
    - _Requirements: 5.2_

  - [x] 4.3 Update `config-documentation.md` steering file
    - Add `source_clustering` row to the `SourceFieldConfig` configuration reference table
    - _Requirements: 5.3_

  - [ ]* 4.4 Write property test for TOML round-trip
    - **Property 6: TOML serialization round-trip**
    - **Validates: Requirements 4.1**

  - [ ]* 4.5 Write unit tests for defaults and info panel
    - Test default `SourceFieldConfig` has `source_clustering == 0.0`
    - Test TOML without `source_clustering` deserializes to `0.0`
    - Test `format_config_info` output contains `"source_clustering"` for both sections
    - _Requirements: 1.1, 4.2, 5.2_

- [ ] 5. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- All code is COLD-path (world init only) — no runtime performance impact
- The `proptest` crate should be added as a dev-dependency if not already present
- Property tests validate universal correctness properties; unit tests validate specific examples and edge cases
