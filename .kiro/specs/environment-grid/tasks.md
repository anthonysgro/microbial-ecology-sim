# Implementation Plan: Environment Grid

## Overview

Implement the Layer 0 environment grid substrate in Rust: a double-buffered, SoA-layout 2D grid with chemical diffusion, heat radiation, and moisture evaporation systems. Uses `rayon` for data parallelism and `proptest` for property-based testing. Each task builds incrementally, wiring components together as they are created.

## Tasks

- [x] 1. Project setup and dependencies
  - Add `rayon` and `proptest` (dev-dependency) to `Cargo.toml`
  - Create module structure: `src/grid/mod.rs`, `src/grid/field_buffer.rs`, `src/grid/partition.rs`, `src/grid/config.rs`, `src/grid/error.rs`
  - Wire modules into `src/lib.rs` and `src/main.rs`
  - _Requirements: 8.2_

- [x] 2. Implement FieldBuffer and error types
  - [x] 2.1 Implement `GridError` and `TickError` enums in `src/grid/error.rs`
    - `GridError::InvalidDimensions`, `GridError::OutOfBounds`, `GridError::InvalidChemicalSpecies`
    - `TickError::NumericalError` with system, cell_index, field, value
    - _Requirements: 1.4, 2.3, 9.4_
  - [x] 2.2 Implement `FieldBuffer<T>` in `src/grid/field_buffer.rs`
    - `new(len, default)` allocating two `Vec<T>` buffers
    - `read() -> &[T]`, `write() -> &mut [T]`, `swap()`
    - Swap via XOR on index, no data copy
    - _Requirements: 1.5, 6.2, 6.4_
  - [ ]* 2.3 Write property tests for FieldBuffer (P1, P3)
    - **Property 1: Field buffer sizing and contiguity** — verify buffer length equals requested size
    - **Validates: Requirements 1.1, 1.3, 8.1**
    - **Property 3: Double-buffer distinctness and swap round-trip** — write, swap, read-back; verify non-aliasing
    - **Validates: Requirements 1.5, 6.2**

- [x] 3. Implement GridConfig, CellDefaults, and Partition
  - [x] 3.1 Implement `GridConfig` and `CellDefaults` structs in `src/grid/config.rs`
    - All fields as described in design (width, height, num_chemicals, rates, num_threads)
    - _Requirements: 3.2, 4.2, 5.2_
  - [x] 3.2 Implement `Partition` struct and row-band partitioning logic in `src/grid/partition.rs`
    - `Partition { start_row, end_row, start_col, end_col }`
    - `cell_indices()` iterator
    - `compute_partitions(width, height, num_threads) -> Vec<Partition>` — row-band slicing
    - _Requirements: 7.1, 7.2, 7.4_
  - [ ]* 3.3 Write property tests for spatial partitioning (P10, P11)
    - **Property 10: Spatial partitions cover all cells with no overlap** — union = full set, no duplicates
    - **Validates: Requirements 7.1, 7.4**
    - **Property 11: Spatial partitions are balanced** — max-min cell count ≤ width
    - **Validates: Requirements 7.2**

- [x] 4. Implement Grid struct and coordinate access
  - [x] 4.1 Implement `Grid` struct in `src/grid/mod.rs`
    - Owns `Vec<FieldBuffer<f32>>` for chemicals, `FieldBuffer<f32>` for heat and moisture, `Vec<Partition>`
    - `Grid::new(config, defaults) -> Result<Self, GridError>` — validate dimensions, allocate SoA buffers, compute partitions
    - `index(x, y) -> Result<usize, GridError>` — bounds check, return y*width+x
    - Read/write accessors for heat, moisture, chemical species
    - Swap methods for each field
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 8.1, 8.2_
  - [ ]* 4.2 Write property tests for Grid initialization and coordinate access (P2, P4, P5)
    - **Property 2: Cell defaults initialization** — all read buffer elements match supplied defaults
    - **Validates: Requirements 1.2**
    - **Property 4: Coordinate access round-trip** — index formula correctness and write/swap/read round-trip
    - **Validates: Requirements 2.1, 2.2, 2.4**
    - **Property 5: Out-of-bounds coordinate rejection** — invalid coords return OutOfBounds error
    - **Validates: Requirements 2.3**

- [x] 5. Checkpoint — core data structures
  - Ensure all tests pass, ask the user if questions arise.

- [-] 6. Implement Diffusion System
  - [x] 6.1 Implement `run_diffusion(grid, config) -> Result<(), TickError>` in `src/grid/diffusion.rs`
    - Read from read buffer, write to write buffer
    - Discrete Laplacian: `new_c[i] = c[i] + Σ(rate × (c[neighbor] - c[i]) × dt)` for each chemical species
    - 8-connectivity neighbor lookup with bounds checking
    - Open boundary condition: missing neighbors have zero concentration
    - Parallelize over partitions using `rayon::par_iter`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
  - [ ]* 6.2 Write property tests for diffusion (P6-diffusion, P7)
    - **Property 6: System runs preserve the read buffer** (diffusion variant) — snapshot read buffer, run diffusion, verify unchanged
    - **Validates: Requirements 3.1**
    - **Property 7: Chemical diffusion conserves mass** — total concentration preserved for zero-boundary grids; total_after ≤ total_before for non-zero boundaries
    - **Validates: Requirements 3.4**
  - [ ]* 6.3 Write unit tests for diffusion edge cases
    - Single hot cell in corner, edge, center on small grids — verify expected neighbor flow values
    - 1×1 grid diffusion (no neighbors)
    - Uniform concentration grid (no change expected)
    - _Requirements: 3.2, 3.3_

- [-] 7. Implement Heat System
  - [x] 7.1 Implement `run_heat(grid, config) -> Result<(), TickError>` in `src/grid/heat.rs`
    - Same structure as diffusion but for heat field
    - Boundary condition: missing neighbors use `config.ambient_heat`
    - Parallelize over partitions using `rayon::par_iter`
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - [ ]* 7.2 Write property tests for heat radiation (P6-heat, P8)
    - **Property 6: System runs preserve the read buffer** (heat variant)
    - **Validates: Requirements 4.1**
    - **Property 8: Heat radiation conserves energy with ambient accounting** — total_after = total_before + boundary_flux
    - **Validates: Requirements 4.4**
  - [ ]* 7.3 Write unit tests for heat edge cases
    - Small grid with known temperatures — verify exact output against hand-computed values
    - Boundary cells exchanging with ambient
    - Uniform heat grid (no change expected)
    - _Requirements: 4.2, 4.3_

- [ ] 8. Implement Evaporation System
  - [ ] 8.1 Implement `run_evaporation(grid, config) -> Result<(), TickError>` in `src/grid/evaporation.rs`
    - Per-cell: `loss = coeff × heat × moisture × dt`, clamp to zero
    - No neighbor interaction — reads heat and moisture from read buffer, writes moisture to write buffer
    - Parallelize over partitions using `rayon::par_iter`
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ]* 8.2 Write property tests for evaporation (P9)
    - **Property 9: Evaporation monotonically decreases moisture and clamps to zero** — for all cells, 0 ≤ moisture_after ≤ moisture_before
    - **Validates: Requirements 5.1, 5.3**
  - [ ]* 8.3 Write unit tests for evaporation edge cases
    - Known heat/moisture/coefficient → verify exact moisture loss
    - Zero heat (no evaporation)
    - Very high heat causing clamp to zero
    - _Requirements: 5.2, 5.3_

- [ ] 9. Checkpoint — all three systems
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 10. Implement Tick Orchestrator
  - [ ] 10.1 Implement `TickOrchestrator::step(grid, config) -> Result<(), TickError>` in `src/grid/tick.rs`
    - Run diffusion → swap chemicals → run heat → swap heat → run evaporation → swap moisture
    - After each system, scan write buffer for NaN/infinity before swapping
    - Return `TickError::NumericalError` on first invalid value found
    - _Requirements: 9.1, 9.2, 9.4_
  - [ ]* 10.2 Write property tests for tick orchestration (P12, P13, P14)
    - **Property 12: Full tick equals sequential system execution** — run step() vs manual sequence, verify identical output
    - **Validates: Requirements 9.1, 9.2**
    - **Property 13: Deterministic execution** — same initial state → bit-identical results across two runs
    - **Validates: Requirements 9.3**
    - **Property 14: NaN/infinity detection** — inject NaN/inf, verify TickError returned with correct metadata
    - **Validates: Requirements 9.4**

- [ ] 11. Final checkpoint — full integration
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` with minimum 100 iterations per property
- Unit tests cover specific examples and edge cases that complement property tests
- Checkpoints ensure incremental validation at natural breakpoints
- All systems are stateless functions — no mutable system state between ticks
