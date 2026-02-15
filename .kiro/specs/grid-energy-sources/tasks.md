# Implementation Plan: Grid Energy Sources

## Overview

Two-phase implementation: first strip all moisture infrastructure from the codebase, then implement energy sources for heat and chemicals. A compilation checkpoint separates the phases to ensure the codebase is clean before adding new functionality.

## Tasks

- [x] 1. Remove moisture from the grid layer
  - [x] 1.1 Delete `src/grid/evaporation.rs` entirely
    - Remove the entire evaporation system module
    - _Requirements: 2.1_

  - [x] 1.2 Strip moisture from `src/grid/mod.rs`
    - Remove `pub mod evaporation;` declaration
    - Remove `moisture: FieldBuffer<f32>` field from `Grid` struct
    - Remove `read_moisture()`, `write_moisture()`, `swap_moisture()`, `read_write_moisture()` methods
    - Remove `heat_read_moisture_rw()` combined accessor
    - Remove moisture initialization from `Grid::new()`
    - _Requirements: 1.1, 1.2, 1.3, 2.2_

  - [x] 1.3 Strip moisture from `src/grid/config.rs`
    - Remove `evaporation_coefficient: f32` from `GridConfig`
    - Remove `moisture: f32` from `CellDefaults`
    - _Requirements: 1.4, 1.5_

  - [x] 1.4 Strip moisture from `src/grid/tick.rs`
    - Remove `use crate::grid::evaporation::run_evaporation;` import
    - Remove Phase 3: `run_evaporation(grid, config)?;` call
    - Remove `validate_buffer(grid.write_moisture(), "evaporation", "moisture")?;`
    - Remove `grid.swap_moisture();`
    - Tick sequence becomes: diffusion → validate → swap chemicals, heat → validate → swap heat
    - _Requirements: 2.3, 2.4_

- [x] 2. Remove moisture from the visualization layer
  - [x] 2.1 Strip moisture from `src/viz/mod.rs`
    - Remove `OverlayMode::Moisture` variant from enum
    - Remove `"Moisture"` label from `OverlayMode::label()`
    - _Requirements: 3.1_

  - [x] 2.2 Strip moisture from `src/viz/renderer.rs`
    - Remove `use crate::viz::color::moisture_bg_color;` import
    - Remove `OverlayMode::Moisture => grid.read_moisture()` field selection branch
    - Remove `OverlayMode::Moisture => { ... moisture_bg_color ... }` rendering branch
    - _Requirements: 3.2_

  - [x] 2.3 Strip moisture from `src/viz/input.rs`
    - Remove `KeyCode::Char('m') => InputAction::SwitchOverlay(OverlayMode::Moisture)` keybinding
    - _Requirements: 3.3_

  - [x] 2.4 Strip moisture from `src/viz/color.rs`
    - Remove `moisture_bg_color()` function
    - Remove associated moisture color tests
    - _Requirements: 3.4_

  - [x] 2.5 Strip moisture from `src/viz/stats.rs`
    - Remove `format_stats_bar_moisture_overlay` test (or any test referencing `OverlayMode::Moisture`)
    - _Requirements: 3.5_

- [x] 3. Remove moisture from application entry point
  - [x] 3.1 Update `src/main.rs`
    - Remove `moisture: 1.0` from `CellDefaults` construction
    - Remove `evaporation_coefficient: 0.01` from `GridConfig` construction
    - _Requirements: 4.1, 4.2_

- [x] 4. Checkpoint — Verify clean compilation after moisture removal
  - Ensure `cargo build` succeeds with no errors
  - Ensure all existing tests pass (`cargo test`)
  - Ask the user if questions arise.

- [x] 5. Define source data types and error enum
  - [x] 5.1 Create `src/grid/source.rs` with `SourceField`, `SourceId`, `Source`, `SourceSlot`, and `SourceError` types
    - `SourceField` enum: `Heat`, `Chemical(usize)`
    - `SourceId` struct: `index: usize`, `generation: u64`
    - `Source` struct: `cell_index: usize`, `field: SourceField`, `emission_rate: f32`
    - `SourceSlot` struct: `source: Option<Source>`, `generation: u64`
    - `SourceError` enum with `thiserror`: `CellOutOfBounds`, `InvalidChemicalSpecies`, `InvalidSourceId`
    - Register the module in `src/grid/mod.rs` as `pub mod source;`
    - _Requirements: 5.4_

- [-] 6. Implement SourceRegistry
  - [x] 6.1 Implement `SourceRegistry::new`, `add`, `remove`, `len`, `is_empty`, and `iter`
    - `new()` creates empty registry
    - `add()` validates cell_index against cell_count, validates chemical species against num_chemicals, reuses free slots via free_list, returns `SourceId` with current generation
    - `remove()` checks index bounds and generation match, sets slot to None, pushes index to free_list, decrements active_count
    - `iter()` yields `&Source` for all `Some` slots in index order
    - _Requirements: 5.1, 5.2, 5.3, 5.5, 10.1, 10.2, 10.3, 10.4_

  - [ ]* 6.2 Write property test: source validation (Property 1)
    - **Property 1: Source validation accepts valid sources and rejects invalid ones**
    - Generate random cell indices, species indices, and emission rates against random grid configs
    - Verify add() succeeds iff cell_index < cell_count AND chemical species < num_chemicals
    - **Validates: Requirements 5.2, 5.3, 5.5**

  - [ ]* 6.3 Write property test: add/remove round-trip with count invariant (Property 5)
    - **Property 5: Add/remove round-trip with count invariant**
    - Generate random sequences of add/remove operations
    - Verify remove(id) succeeds after add, source disappears from iter(), len() tracks correctly
    - **Validates: Requirements 10.1, 10.2, 10.4**

  - [ ]* 6.4 Write property test: deterministic iteration order (Property 6)
    - **Property 6: Deterministic iteration order**
    - Generate random add/remove sequences, apply to two independent registries
    - Verify iter() yields identical sequences
    - **Validates: Requirements 11.1, 11.2**

  - [ ]* 6.5 Write unit tests for edge cases
    - Double-removal returns `SourceError::InvalidSourceId`
    - Empty registry: len() == 0, iter() yields nothing
    - _Requirements: 10.3_

- [x] 7. Implement FieldBuffer::copy_read_to_write and run_emission
  - [x] 7.1 Add `copy_read_to_write` method to `FieldBuffer<T: Copy>`
    - Copies read buffer contents into write buffer via `copy_from_slice`
    - _Requirements: 8.2_

  - [x] 7.2 Implement `run_emission` function in `src/grid/source.rs`
    - Iterates `SourceRegistry::iter()`, matches on `SourceField`, adds emission_rate to the appropriate write buffer at cell_index
    - WARM path: sequential, no parallelism, no allocation
    - _Requirements: 6.1, 7.1, 8.4_

  - [ ]* 7.3 Write property test: emission additivity (Property 2)
    - **Property 2: Emission is additive injection into the correct field**
    - Generate random grid state and random source list
    - Copy read→write, run emission, verify write_buf[cell] == read_buf[cell] + sum(emission_rates) for each (field, cell)
    - Verify untargeted cells are unchanged
    - **Validates: Requirements 6.1, 6.2, 6.3, 7.1, 7.2, 7.3, 8.2**

- [x] 8. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [~] 9. Integrate emission phase into TickOrchestrator
  - [ ] 9.1 Implement `run_emission_phase` helper in `src/grid/tick.rs`
    - Scan source registry to determine which field types have active sources
    - For each affected field: copy_read_to_write, run_emission
    - Clamp chemical write buffers to ≥ 0.0
    - Validate affected write buffers (reuse existing `validate_buffer`)
    - Swap affected field buffers
    - No-op if source registry is empty
    - _Requirements: 8.1, 8.2, 8.3, 9.1, 9.2_

  - [ ] 9.2 Update `TickOrchestrator::step` to call `run_emission_phase` before existing systems
    - Add `run_emission_phase(grid, config)?;` as the first call in `step()`
    - _Requirements: 8.1_

  - [ ]* 9.3 Write property test: non-negative chemical clamping (Property 4)
    - **Property 4: Non-negative clamping for chemicals**
    - Generate grid with low chemical values and large negative-rate drain sources
    - Run emission phase, verify all chemical values ≥ 0.0
    - Verify heat values are NOT clamped (can go negative)
    - **Validates: Requirements 9.2**

  - [ ]* 9.4 Write property test: emission-tick integration (Property 3)
    - **Property 3: Emission-tick integration — downstream systems process post-emission state**
    - Generate a grid with a single heat source, run one tick with sources vs one tick without
    - Verify post-tick heat values differ (source cell and neighbors should be warmer)
    - **Validates: Requirements 8.1, 8.3**

  - [ ]* 9.5 Write unit tests for emission phase edge cases
    - NaN emission rate triggers TickError
    - Infinite emission rate triggers TickError
    - Empty registry produces identical pre/post tick state
    - _Requirements: 9.1_

- [~] 10. Add Grid convenience API and wire into main.rs
  - [ ] 10.1 Add `sources`, `sources_mut`, `add_source`, `remove_source` methods to `Grid`
    - `add_source` delegates to `sources.add()` with grid's cell_count and num_chemicals
    - `remove_source` delegates to `sources.remove()`
    - Initialize `SourceRegistry::new()` in `Grid::new()`
    - _Requirements: 10.1, 10.2, 10.3, 10.4_

  - [ ] 10.2 Update `main.rs` to use `add_source` instead of direct buffer writes
    - Replace the manual `write_heat()[center] = 50.0; swap_heat()` with `grid.add_source(Source { cell_index: center, field: SourceField::Heat, emission_rate: 50.0 })`
    - Replace the manual chemical hotspot seeding with a chemical source
    - _Requirements: 6.1, 7.1_

- [~] 11. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Phase 1 (tasks 1–4) removes all moisture infrastructure; Phase 2 (tasks 5–11) adds energy sources
- Task 4 is a hard gate: the codebase must compile cleanly before starting energy source work
- Property tests use `proptest` crate — add it as a dev-dependency in Cargo.toml
- The emission phase is WARM classification: sequential iteration over a small source list, no parallelism needed
- No new `unsafe` blocks are introduced
