# Implementation Plan: Simulation Stats HUD

## Overview

Thread predation count from `run_contact_predation` up through the tick orchestrator to a new Bevy resource, and extend the trait stats computation and formatting to include actor energy statistics. All changes are COLD-path visualization plumbing except the one-line return value change in `run_contact_predation`.

## Tasks

- [x] 1. Propagate predation count through the simulation layer
  - [x] 1.1 Change `run_contact_predation` return type from `Result<(), TickError>` to `Result<usize, TickError>` and return `events.len()` on success
    - Update all `Ok(())` returns to `Ok(events.len())` (there is one at the end of the function)
    - _Requirements: 1.1_
  - [x] 1.2 Change `run_actor_phases` return type to `Result<usize, TickError>` and capture/return the predation count
    - Bind the `usize` from `run_contact_predation` call, return it at the end via `Ok(predation_count)`
    - Update all other `Ok(())` early returns (if any) and the final `Ok(())` to `Ok(predation_count)`
    - _Requirements: 1.2_
  - [x] 1.3 Change `TickOrchestrator::step` return type to `Result<usize, TickError>` and propagate the count
    - When actors exist: capture count from `run_actor_phases`, return `Ok(count)` at the end
    - When actors are absent: return `Ok(0)` at the end
    - Update all other `Ok(())` returns to `Ok(predation_count)` or `Ok(0)` as appropriate
    - _Requirements: 1.3, 1.4_
  - [ ]* 1.4 Write property test for predation count accuracy
    - **Property 1: Predation count accuracy**
    - **Validates: Requirements 1.1**

- [x] 2. Add PredationCounter resource and wire into tick_simulation
  - [x] 2.1 Define `PredationCounter` resource in `src/viz_bevy/resources.rs`
    - Fields: `last_tick: usize`, `total: u64`
    - Derive/implement `Resource`, `Default`
    - _Requirements: 2.1, 2.2_
  - [x] 2.2 Insert `PredationCounter::default()` as a resource during Bevy app setup in `src/viz_bevy/setup.rs`
    - _Requirements: 2.1_
  - [x] 2.3 Update `tick_simulation` in `src/viz_bevy/systems.rs` to accept `ResMut<PredationCounter>` and update it on successful tick
    - `counter.last_tick = predation_count; counter.total += predation_count as u64;`
    - _Requirements: 2.3, 2.4_
  - [ ]* 2.4 Write property test for predation counter accumulation
    - **Property 2: Predation counter accumulation**
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [x] 3. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Add energy stats to TraitStats and compute_trait_stats_from_actors
  - [x] 4.1 Add `energy_stats: Option<SingleTraitStats>` field to `TraitStats` in `src/viz_bevy/resources.rs`
    - Update all construction sites of `TraitStats` to include the new field
    - _Requirements: 4.1, 4.2, 4.3_
  - [x] 4.2 Extend `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs` to collect `actor.energy` and compute energy stats
    - Add a 10th `Vec<f32>` for energy in the same single-pass loop
    - Call `compute_single_stats` on the energy vec
    - Populate `energy_stats` field (Some when actors exist, None otherwise)
    - _Requirements: 4.1, 4.2, 4.3_
  - [ ]* 4.3 Write property test for energy stats correctness
    - **Property 4: Energy stats correctness**
    - **Validates: Requirements 4.1, 4.2, 4.3**

- [x] 5. Update HUD formatting
  - [x] 5.1 Update `format_trait_stats` signature in `src/viz_bevy/setup.rs` to accept `&PredationCounter` and render predation values in the header line
    - Format: `Tick: N  |  Actors: N  |  Predations: N (total: N)`
    - _Requirements: 3.1, 3.2_
  - [x] 5.2 Append energy row to `format_trait_stats` output when `energy_stats` is `Some`
    - Same tabular format as heritable traits, labeled "energy"
    - _Requirements: 4.4_
  - [x] 5.3 Update `update_stats_panel` system in `src/viz_bevy/systems.rs` to pass `Res<PredationCounter>` to `format_trait_stats`
    - _Requirements: 3.1_
  - [ ]* 5.4 Write property test for header format includes predation values
    - **Property 3: Header format includes predation values**
    - **Validates: Requirements 3.1**
  - [ ]* 5.5 Write property test for energy row in formatted output
    - **Property 5: Energy row in formatted output**
    - **Validates: Requirements 4.4**

- [x] 6. Update documentation
  - [x] 6.1 Update `config-documentation.md` steering file to document `PredationCounter` resource and `TraitStats::energy_stats` field
    - _Requirements: 5.1, 5.2_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties using `proptest`
- Unit tests validate specific examples and edge cases
