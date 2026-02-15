# Implementation Plan: Grid Actors

## Overview

Incremental implementation of mobile Actors into the existing deterministic grid simulation. Each task builds on the previous, starting with data types and registry, then systems, then tick integration. Property tests are placed adjacent to the code they validate.

## Tasks

- [x] 1. Define Actor data types and error enum
  - [x] 1.1 Create `src/grid/actor.rs` with `Actor`, `ActorId`, `ActorSlot`, and `ActorError` types
    - `Actor`: plain data struct with `cell_index: usize` and `energy: f32`, deriving `Debug, Clone, Copy, PartialEq`
    - `ActorId`: generational index with `index: usize` and `generation: u64`, deriving `Debug, Clone, Copy, PartialEq, Eq, Hash`
    - `ActorSlot`: `pub(crate)` struct with `actor: Option<Actor>` and `generation: u64`
    - `ActorError`: enum with `CellOutOfBounds`, `CellOccupied`, `InvalidActorId` variants using `thiserror`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 11.1, 11.2, 11.3, 11.4_

- [-] 2. Implement ActorRegistry with occupancy map integration
  - [x] 2.1 Implement `ActorRegistry` in `src/grid/actor.rs`
    - `new() -> Self` and `with_capacity(cap: usize) -> Self`
    - `add(actor, cell_count, occupancy) -> Result<ActorId, ActorError>` — validates cell_index, checks occupancy, inserts into slot, updates occupancy map
    - `remove(id, occupancy) -> Result<(), ActorError>` — validates generation, clears slot, clears occupancy, pushes to free list
    - `get(id) -> Result<&Actor, ActorError>` and `get_mut(id) -> Result<&mut Actor, ActorError>`
    - `len()`, `is_empty()`, `iter()`, `iter_mut()` — deterministic slot-index order
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [ ]* 2.2 Write property test: Occupancy invariant (Property 1)
    - **Property 1: Occupancy invariant**
    - Generate random sequences of add/remove operations, verify occupancy map consistency after each operation
    - **Validates: Requirements 3.2, 3.3, 3.4, 3.5, 6.4, 10.1**

  - [ ]* 2.3 Write property test: Add rejects out-of-bounds cell index (Property 2)
    - **Property 2: Add rejects out-of-bounds cell index**
    - Generate cell_index >= cell_count, verify CellOutOfBounds error
    - **Validates: Requirements 2.3, 11.2**

  - [ ]* 2.4 Write property test: Add rejects occupied cell (Property 3)
    - **Property 3: Add rejects occupied cell**
    - Add actor, then add second actor at same cell, verify CellOccupied error
    - **Validates: Requirements 3.4, 11.3**

  - [ ]* 2.5 Write property test: Remove rejects stale ActorId (Property 4)
    - **Property 4: Remove rejects stale ActorId**
    - Add then remove actor, attempt second remove, verify InvalidActorId error
    - **Validates: Requirements 2.4, 11.4**

  - [ ]* 2.6 Write property test: Add-remove round trip (Property 5)
    - **Property 5: Add-remove round trip**
    - Add then remove, verify len==0, is_empty==true, occupancy cleared
    - **Validates: Requirements 2.7, 3.3**

- [x] 3. Create ActorConfig and extend Grid
  - [x] 3.1 Create `src/grid/actor_config.rs` with `ActorConfig` struct
    - Fields: `consumption_rate`, `energy_conversion_factor`, `base_energy_decay`, `initial_energy`, `initial_actor_capacity`
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

  - [x] 3.2 Extend `Grid` struct in `src/grid/mod.rs`
    - Add `actors: ActorRegistry`, `occupancy: Vec<Option<usize>>`, `removal_buffer: Vec<ActorId>`, `movement_targets: Vec<Option<usize>>` fields
    - Pre-allocate occupancy (cell_count), removal_buffer and movement_targets (initial_actor_capacity) in `Grid::new`
    - Add `actors()`, `actors_mut()`, `occupancy()`, `add_actor()`, `remove_actor()` methods
    - Add `take_actors`/`put_actors` pattern matching existing `take_sources`/`put_sources`
    - Update `Grid::new` signature to accept `Option<ActorConfig>` (or a separate builder method)
    - Register `actor` and `actor_config` modules in `src/grid/mod.rs`
    - _Requirements: 3.1, 3.2, 3.3, 3.5, 8.3_

- [x] 4. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement Actor sensing system
  - [x] 5.1 Create `src/grid/actor_systems.rs` with `run_actor_sensing` function
    - Iterate actors in slot order via `ActorRegistry::iter()`
    - For each actor, read Von Neumann neighbors from chemical read buffer (species 0)
    - Compute gradient for each neighbor, select max positive gradient
    - Boundary cells: out-of-bounds neighbors treated as 0.0 concentration
    - Write target cell_index (or None) into pre-allocated `movement_targets` buffer
    - No allocation, no dynamic dispatch
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

  - [ ]* 5.2 Write property test: Sensing selects maximum positive gradient neighbor (Property 6)
    - **Property 6: Sensing selects maximum positive gradient neighbor**
    - Generate random grid chemical state and actor positions, verify selected target matches expected max-gradient neighbor
    - **Validates: Requirements 5.1, 5.3, 5.4, 5.5**

- [-] 6. Implement Actor metabolism system
  - [x] 6.1 Implement `run_actor_metabolism` function in `src/grid/actor_systems.rs`
    - Copy chemical read → write buffer before metabolism (same pattern as emission)
    - Iterate actors in slot order
    - Compute consumed = min(consumption_rate, chemical_read[cell_index])
    - Subtract consumed from chemical_write[cell_index], clamp to 0.0
    - Update actor energy: += consumed * energy_conversion_factor - base_energy_decay
    - If energy <= 0.0, push ActorId to removal_buffer
    - Validate actor energy for NaN/Inf, return TickError if detected
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 11.5_

  - [ ]* 6.2 Write property test: Metabolism energy balance (Property 9)
    - **Property 9: Metabolism energy balance**
    - Generate random actors and chemical state, verify energy delta = min(rate, available) * factor - decay
    - **Validates: Requirements 7.1, 7.2, 7.3, 7.4**

  - [ ]* 6.3 Write property test: Chemical non-negativity after consumption (Property 10)
    - **Property 10: Chemical non-negativity after consumption**
    - Generate random actors and chemical state, verify all write buffer values >= 0.0 after metabolism
    - **Validates: Requirements 7.5, 10.5**

  - [ ]* 6.4 Write property test: Dead actors are removed after metabolism (Property 11)
    - **Property 11: Dead actors are removed after metabolism**
    - Generate actors with low energy and high decay, verify dead actors appear in removal buffer and are absent from registry after deferred removal
    - **Validates: Requirements 7.6, 8.1, 8.2**

- [x] 7. Implement Actor movement system
  - [x] 7.1 Implement `run_actor_movement` function in `src/grid/actor_systems.rs`
    - Iterate actors in slot order
    - For each actor with a movement target: check occupancy map at target cell
    - If unoccupied: update occupancy (clear old, set new), update actor.cell_index
    - If occupied: skip (actor stays)
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_

  - [ ]* 7.2 Write property test: Movement distance invariant (Property 7)
    - **Property 7: Movement distance invariant**
    - Generate random actor positions and movement targets, verify Manhattan distance <= 1 after movement
    - **Validates: Requirements 6.2**

  - [ ]* 7.3 Write property test: Movement conflict — lower slot wins (Property 8)
    - **Property 8: Movement conflict — lower slot wins**
    - Generate two actors targeting the same cell, verify lower slot index occupies target
    - **Validates: Requirements 6.3, 6.6**

- [x] 8. Implement deferred removal
  - [x] 8.1 Implement `run_deferred_removal` function in `src/grid/actor_systems.rs`
    - Sort removal_buffer by slot index (ascending) for deterministic order
    - Call `ActorRegistry::remove` for each entry, clearing occupancy
    - Clear removal_buffer after processing
    - _Requirements: 8.1, 8.2, 8.4_

- [ ] 9. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [-] 10. Integrate Actor phases into TickOrchestrator
  - [x] 10.1 Extend `TickOrchestrator::step` in `src/grid/tick.rs`
    - After emission phase: run actor sensing, metabolism, deferred removal, movement
    - Swap chemical buffers after actor consumption (before diffusion)
    - Skip all actor phases if `actors.is_empty()`
    - Validate chemical write buffers after actor consumption (NaN/Inf check)
    - Preserve existing emission → diffusion → heat → swap ordering around actor phases
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [ ]* 10.2 Write property test: Tick determinism (Property 12)
    - **Property 12: Tick determinism**
    - Generate random initial grid state with actors, run step twice from cloned state, verify identical output
    - **Validates: Requirements 4.4, 10.4**

  - [ ]* 10.3 Write property test: Zero-actor tick equivalence (Property 13)
    - **Property 13: Zero-actor tick equivalence**
    - Generate random grid state with zero actors, run extended step, compare to original step output
    - **Validates: Requirements 4.5, 10.6**

- [ ] 11. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP.
- Each task references specific requirements for traceability.
- Checkpoints ensure incremental validation.
- Property tests use `proptest` with minimum 256 iterations per test.
- Unit tests for edge cases (boundary cells, zero energy, NaN configs) should be added alongside property tests where noted.
- All Actor system functions follow the existing pattern: free functions, no state, `Result` return types.
- No `unwrap()`/`expect()` in any simulation logic.
