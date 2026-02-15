# Implementation Plan: Source Respawn Cycling

## Overview

Implement a cooldown-then-respawn cycle for depleted non-renewable sources. The work proceeds bottom-up: data structures first, then depletion detection, then respawn logic, then tick integration, then config/docs.

## Tasks

- [x] 1. Add respawn config fields to `SourceFieldConfig`
  - [x] 1.1 Add `respawn_enabled: bool`, `min_respawn_cooldown_ticks: u32`, `max_respawn_cooldown_ticks: u32` to `SourceFieldConfig` in `src/grid/world_init.rs`
    - Add serde defaults: `respawn_enabled = false`, `min_respawn_cooldown_ticks = 50`, `max_respawn_cooldown_ticks = 150`
    - Update `Default` impl for `SourceFieldConfig`
    - _Requirements: 4.1, 4.2, 4.4, 4.5_
  - [x] 1.2 Add respawn validation to `validate_source_field_config` in `src/grid/world_init.rs`
    - When `respawn_enabled` is true: reject `min > max` cooldown and `max == 0`
    - When `respawn_enabled` is false: skip cooldown validation
    - Add label fields to `SourceFieldLabels` for error messages
    - _Requirements: 5.1, 5.2, 5.3_
  - [ ]* 1.3 Write property test for respawn config validation
    - **Property 7: Respawn config validation**
    - **Validates: Requirements 5.1, 5.2, 5.3**

- [x] 2. Add `RespawnEntry` and `RespawnQueue` data structures
  - [x] 2.1 Create `RespawnEntry` struct and `RespawnQueue` in `src/grid/source.rs`
    - `RespawnEntry`: `field: SourceField`, `respawn_tick: u64`
    - `RespawnQueue`: `entries: Vec<RespawnEntry>` with `with_capacity`, `push`, `len`, `is_empty`, `drain_mature(current_tick)` methods
    - `drain_mature` returns entries where `respawn_tick <= current_tick` in deterministic order, retains the rest
    - _Requirements: 2.1, 2.3, 6.2_
  - [x] 2.2 Add `respawn_queue: RespawnQueue` field to `Grid` in `src/grid/mod.rs`
    - Add `respawn_queue()` and `respawn_queue_mut()` accessors
    - Initialize with `RespawnQueue::with_capacity` based on initial source count in `Grid::new` or `initialize`
    - _Requirements: 6.2_

- [x] 3. Implement depletion event detection in `run_emission`
  - [x] 3.1 Add `DepletionEvent` struct and `iter_mut_with_ids` method to `SourceRegistry` in `src/grid/source.rs`
    - `DepletionEvent`: `source_id: SourceId`, `field: SourceField`, `tick: u64`
    - `iter_mut_with_ids`: yields `(SourceId, &mut Source)` in slot order
    - _Requirements: 1.1_
  - [x] 3.2 Modify `run_emission` to detect depletions and return `SmallVec<[DepletionEvent; 8]>`
    - Add `current_tick: u64` parameter
    - Track `was_depleted` before reservoir drain; if reservoir transitions to 0.0, emit `DepletionEvent`
    - Renewable sources (infinite reservoir) never produce events
    - Update call site in `run_emission_phase` to pass `current_tick` and capture return value
    - _Requirements: 1.1, 1.2, 1.3_
  - [ ]* 3.3 Write property test for depletion event correctness
    - **Property 1: Depletion events are correct and trigger slot removal**
    - **Validates: Requirements 1.1, 1.2, 1.3, 8.1**

- [x] 4. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement respawn queue processing and source spawning
  - [x] 5.1 Implement `run_respawn_phase` function in `src/grid/source.rs`
    - Process mature entries from `RespawnQueue::drain_mature`
    - For each mature entry: sample emission rate, reservoir, deceleration threshold from the corresponding `SourceFieldConfig` (always non-renewable: `renewable_prob = 0.0`)
    - Select random unoccupied cell for the target field type
    - If all cells occupied, re-push entry with `respawn_tick += 1`
    - Register new source via `Grid::add_source`
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_
  - [ ]* 5.2 Write property test for respawned source parameters
    - **Property 6: Respawned source parameters within config ranges and always non-renewable**
    - **Validates: Requirements 3.4, 3.5, 3.6**
  - [ ]* 5.3 Write property test for unoccupied cell selection
    - **Property 5: Respawned sources land on unoccupied cells**
    - **Validates: Requirements 3.2**
  - [ ]* 5.4 Write property test for mature entry processing
    - **Property 4: Mature entries spawn exactly one source and are removed from queue**
    - **Validates: Requirements 3.1, 3.7**

- [x] 6. Integrate into tick orchestration
  - [x] 6.1 Modify `run_emission_phase` in `src/grid/tick.rs` to process depletion events
    - After `run_emission` returns depletions: for each event, check field config's `respawn_enabled`
    - If enabled: sample cooldown from `[min, max]` range, push `RespawnEntry` to queue, remove depleted source from registry
    - If disabled: remove depleted source from registry (slot cleanup only, no respawn)
    - _Requirements: 2.1, 2.2, 2.4, 8.1_
  - [x] 6.2 Call `run_respawn_phase` after emission in `run_emission_phase` or `TickOrchestrator::step`
    - Pass seeded RNG, current tick, heat/chemical `SourceFieldConfig` references, `num_chemicals`
    - Thread `WorldInitConfig` (or the two `SourceFieldConfig` refs) through `TickOrchestrator::step` signature
    - _Requirements: 7.1, 7.2, 7.3_
  - [ ]* 6.3 Write property test for respawn cooldown range
    - **Property 2: Respawn cooldown is within configured range**
    - **Validates: Requirements 2.1, 2.2**
  - [ ]* 6.4 Write property test for respawn disabled behavior
    - **Property 3: Respawn disabled produces no queue entries and no respawns**
    - **Validates: Requirements 2.4, 4.3**
  - [ ]* 6.5 Write property test for deterministic respawn
    - **Property 8: Deterministic respawn**
    - **Validates: Requirements 6.1**

- [x] 7. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Update configuration documentation
  - [x] 8.1 Update `example_config.toml` with new respawn fields
    - Add `respawn_enabled`, `min_respawn_cooldown_ticks`, `max_respawn_cooldown_ticks` to both `[world_init.heat_source_config]` and `[world_init.chemical_source_config]` sections with explanatory comments
    - _Requirements: 9.1_
  - [x] 8.2 Update `format_config_info()` in `src/viz_bevy/setup.rs`
    - Display the new respawn fields in the info panel for both heat and chemical source configs
    - _Requirements: 9.3_
  - [x] 8.3 Update `config-documentation.md` steering rule
    - Add `respawn_enabled`, `min_respawn_cooldown_ticks`, `max_respawn_cooldown_ticks` to the `SourceFieldConfig` reference tables
    - _Requirements: 9.2_

- [x] 9. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Replacement sources are always non-renewable — this maintains the depletion→cooldown→respawn loop
- `respawn_enabled` defaults to `false` for backward compatibility
- `SmallVec<[DepletionEvent; 8]>` avoids heap allocation for the common case in `run_emission`
- The `drain_mature` allocation on `RespawnQueue` is acceptable (WARM path, small queue)
