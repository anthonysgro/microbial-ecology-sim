# Implementation Plan: Respawn Cluster Persistence

## Overview

Persist cluster centers computed during `generate_sources` on the `Grid`, then use them in `run_respawn_phase` to place replacement sources near the original cluster region. Minimal changes: one new struct, one new field on `Grid`, a visibility promotion, and a branch in the respawn loop.

## Tasks

- [-] 1. Add ClusterCenter struct and ClusterCenterMap type
  - [-] 1.1 Define `ClusterCenter` struct and `ClusterCenterMap` type alias in `src/grid/source.rs`
    - Add `ClusterCenter { col: u32, row: u32 }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`
    - Add `pub type ClusterCenterMap = SmallVec<[(SourceField, ClusterCenter); 4]>;`
    - Add `pub fn lookup_cluster_center(map: &ClusterCenterMap, field: SourceField) -> Option<ClusterCenter>` free function
    - _Requirements: 5.1, 5.2, 5.3, 1.3_

  - [ ]* 1.2 Write unit tests for `lookup_cluster_center`
    - Test empty map returns `None`
    - Test correct lookup for Heat vs Chemical(0) vs Chemical(1)
    - Test returns `None` for missing field
    - _Requirements: 1.3_

- [ ] 2. Add `cluster_centers` field to `Grid`
  - [ ] 2.1 Add `cluster_centers: ClusterCenterMap` field to `Grid` struct in `src/grid/mod.rs`
    - Initialize as `SmallVec::new()` in `Grid::new`
    - Add `pub fn cluster_centers(&self) -> &ClusterCenterMap` accessor
    - Add `pub fn cluster_centers_mut(&mut self) -> &mut ClusterCenterMap` accessor
    - _Requirements: 1.4_

  - [ ]* 2.2 Write unit test that `Grid::new` initializes `cluster_centers` as empty
    - _Requirements: 1.4_

- [ ] 3. Promote `sample_clustered_position` visibility and store centers during init
  - [ ] 3.1 Change `sample_clustered_position` in `src/grid/world_init.rs` from `fn` to `pub(crate) fn`
    - _Requirements: 6.2_

  - [ ] 3.2 Modify `generate_sources` to store cluster centers in the Grid's ClusterCenterMap
    - After sampling `heat_center_col`/`heat_center_row`, push `(SourceField::Heat, ClusterCenter { col, row })` if `heat_cfg.source_clustering > 0.0`
    - After sampling each chemical species center, push `(SourceField::Chemical(species), ClusterCenter { col, row })` if `chem_cfg.source_clustering > 0.0`
    - _Requirements: 1.1, 1.2_

  - [ ]* 3.3 Write property test: cluster center storage biconditional
    - **Property 1: Cluster center storage biconditional**
    - **Validates: Requirements 1.1, 1.2, 1.3**

- [ ] 4. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Modify `run_respawn_phase` to use stored cluster centers
  - [ ] 5.1 Update `run_respawn_phase` in `src/grid/source.rs` to look up cluster center and branch on result
    - Import `sample_clustered_position` from `crate::grid::world_init`
    - Before cell selection, call `lookup_cluster_center(grid.cluster_centers(), entry.field)`
    - If `Some(center)`: use `sample_clustered_position` with stored center, grid dimensions, and `config.source_clustering` in a rejection-sampling loop over occupied cells
    - If `None`: keep existing uniform-random cell selection logic unchanged
    - Preserve the dense-grid saturation check (`occupied.len() >= cell_count`) before the branch
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 6.1_

  - [ ]* 5.2 Write property test: clustered respawn uses stored center
    - **Property 2: Clustered respawn placement uses stored center**
    - **Validates: Requirements 2.1**

  - [ ]* 5.3 Write property test: unclustered respawn falls back to uniform placement
    - **Property 3: Unclustered respawn falls back to uniform placement**
    - **Validates: Requirements 2.2**

  - [ ]* 5.4 Write property test: respawn collision avoidance
    - **Property 4: Respawn collision avoidance**
    - **Validates: Requirements 2.3**

- [ ] 6. End-to-end determinism verification
  - [ ]* 6.1 Write property test: end-to-end determinism
    - **Property 5: End-to-end determinism**
    - **Validates: Requirements 3.1, 3.2**

  - [ ]* 6.2 Write golden-seed regression test
    - Specific seed with `source_clustering = 0.7`, `respawn_enabled = true`
    - Verify exact source positions after init and after one respawn cycle
    - _Requirements: 3.1, 3.2_

- [ ] 7. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP.
- No TOML config changes needed — `source_clustering` already exists on `SourceFieldConfig`.
- No HOT-path code is modified. All changes are COLD (init) or WARM (respawn).
- `sample_clustered_position` already handles toroidal wrapping and degenerate sigma — no new edge cases introduced.
- Property tests use `proptest` crate with minimum 100 iterations each.
