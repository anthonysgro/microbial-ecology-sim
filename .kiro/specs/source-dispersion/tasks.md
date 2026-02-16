# Implementation Plan: Source Dispersion

## Overview

Add `source_dispersion: f32` to `SourceFieldConfig`, update cluster center storage to support multiple centers per field, add `cluster_index: u8` to `Source` and `RespawnEntry`, update generation and respawn logic, and update all documentation surfaces.

## Tasks

- [x] 1. Add `source_dispersion` field and validation
  - [x] 1.1 Add `source_dispersion: f32` to `SourceFieldConfig` in `src/grid/world_init.rs`
    - Add the field with `pub source_dispersion: f32`
    - Set default to `0.0` in `impl Default for SourceFieldConfig`
    - Add `source_dispersion_range` and `source_dispersion_finite` labels to `SourceFieldLabels`
    - Add corresponding entries to `HEAT_LABELS` and `CHEMICAL_LABELS` constants
    - Add validation checks in `validate_source_field_config()`: finite check and `[0.0, 1.0]` range check, matching the existing `source_clustering` pattern
    - _Requirements: 1.1, 2.1, 2.2_
  - [ ]* 1.2 Write property test for dispersion validation
    - **Property 2: Validation rejects out-of-range dispersion**
    - Generate random f32 values outside [0.0, 1.0] (negative, >1.0, NaN, infinity). Construct a SourceFieldConfig with that dispersion. Call validate_source_field_config. Assert error.
    - **Validates: Requirements 2.1, 2.2**

- [x] 2. Update `Source`, `RespawnEntry`, and `ClusterCenterMap` data structures
  - [x] 2.1 Add `cluster_index: u8` to `Source` in `src/grid/source.rs`
    - Add `pub cluster_index: u8` field to the `Source` struct
    - Update all existing `Source { ... }` construction sites to include `cluster_index: 0` (backward compatible default)
    - Sites: `generate_sources()` in `world_init.rs`, `run_respawn_phase()` in `source.rs`, and any test code
    - _Requirements: 4.1_
  - [x] 2.2 Add `cluster_index: u8` to `RespawnEntry` in `src/grid/source.rs`
    - Add `pub cluster_index: u8` field to the `RespawnEntry` struct
    - Update all `RespawnEntry { ... }` construction sites to include `cluster_index: 0`
    - Sites: `run_respawn_phase()` re-queue path, `run_emission_phase()` in `tick.rs`
    - _Requirements: 4.3_
  - [x] 2.3 Update `ClusterCenterMap` type and `lookup_cluster_center` in `src/grid/source.rs`
    - Change type alias to `SmallVec<[(SourceField, u8, ClusterCenter); 8]>`
    - Update `lookup_cluster_center` signature to accept `cluster_index: u8` parameter
    - Update the find predicate to match on both `field` and `cluster_index`
    - Update all call sites of `lookup_cluster_center` (currently in `run_respawn_phase`)
    - Update all `.push(...)` calls on `cluster_centers_mut()` (currently in `generate_sources`) to include the cluster index
    - _Requirements: 5.1_

- [x] 3. Checkpoint
  - Ensure all code compiles after data structure changes. Run `cargo check`. Ask the user if questions arise.

- [x] 4. Implement multi-center source generation
  - [x] 4.1 Update `generate_sources()` in `src/grid/world_init.rs`
    - For each field batch (heat, each chemical species):
      - Compute `K = max(1, round(source_dispersion * num_sources))`, clamped to `min(K, 255)`
      - Sample K independent cluster center positions `(col, row)` uniformly at random
      - Store centers in `ClusterCenterMap` when `source_clustering > 0.0` OR `source_dispersion > 0.0`
      - Assign each source round-robin: `cluster_index = (i % K) as u8`
      - Sample position via `sample_clustered_position` using the assigned center's coordinates
      - Set `source.cluster_index` on each created `Source`
    - When `source_dispersion == 0.0`, K=1, collapsing to current single-center behavior
    - _Requirements: 1.2, 1.3, 1.4, 3.1, 3.2, 3.3, 3.4, 5.2, 5.3, 5.4, 6.2_
  - [ ]* 4.2 Write property test for cluster count formula
    - **Property 1: Cluster count matches dispersion formula**
    - Generate random (source_dispersion, num_sources) pairs. Compute expected K. Run generate_sources on a minimal grid. Count cluster centers in the map for the field. Assert count == expected K.
    - **Validates: Requirements 1.2, 1.3, 1.4, 5.4**
  - [ ]* 4.3 Write property test for round-robin cluster assignment
    - **Property 3: Round-robin cluster assignment**
    - Generate random (source_dispersion, num_sources) pairs. Run generate_sources. Iterate all sources for the field. Assert source at index i has cluster_index == (i % K) as u8.
    - **Validates: Requirements 3.2, 4.2**

- [x] 5. Update respawn pipeline to preserve cluster index
  - [x] 5.1 Update depletion → respawn entry creation in `src/grid/tick.rs`
    - In `run_emission_phase`, before calling `registry.remove(event.source_id)`, read the depleted source's `cluster_index` from the registry
    - Add a method or use existing `iter_mut_with_ids` / slot access to read `cluster_index` from a `SourceId`
    - Pass `cluster_index` into the `RespawnEntry` constructor
    - _Requirements: 4.3_
  - [x] 5.2 Update `run_respawn_phase()` in `src/grid/source.rs`
    - Use `entry.cluster_index` when calling `lookup_cluster_center`
    - Set `cluster_index: entry.cluster_index` on the replacement `Source`
    - Preserve `cluster_index` when re-queuing deferred entries (all-cells-occupied path)
    - _Requirements: 4.4, 6.3_
  - [ ]* 5.3 Write property test for respawn cluster index preservation
    - **Property 4: Respawn preserves cluster index**
    - Generate a config with respawn_enabled=true, finite sources, and known cluster_index values. Run simulation ticks until depletion. Verify the respawn entry and replacement source carry the original cluster_index.
    - **Validates: Requirements 4.3, 4.4**

- [x] 6. Checkpoint
  - Ensure all tests pass. Run `cargo test`. Ask the user if questions arise.

- [x] 7. Documentation updates
  - [x] 7.1 Update `example_config.toml`
    - Add `source_dispersion = 0.0` with explanatory comment in `[world_init.heat_source_config]`
    - Add `source_dispersion = 0.0` with explanatory comment in `[world_init.chemical_species_configs.source_config]`
    - _Requirements: 7.1_
  - [x] 7.2 Update `format_config_info()` in `src/viz_bevy/setup.rs`
    - Add `source_dispersion` display line after `source_clustering` for heat config
    - Add `source_dispersion` display line after `source_clustering` for each chemical species config
    - _Requirements: 7.2_
  - [x] 7.3 Update `config-documentation.md` steering file
    - Add `source_dispersion` row to the `SourceFieldConfig` table with type `f32`, default `0.0`, and description
    - _Requirements: 7.3_

- [x] 8. Final checkpoint
  - Ensure all tests pass. Run `cargo test`. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The implementation language is Rust, using `proptest` for property-based tests
- COLD path (generation) and WARM path (respawn) classifications are preserved — no HOT path changes
- `cluster_index: u8` caps at 255 clusters, sufficient for any realistic source count
- Backward compatibility: `source_dispersion` defaults to `0.0`, producing identical behavior to pre-feature code
