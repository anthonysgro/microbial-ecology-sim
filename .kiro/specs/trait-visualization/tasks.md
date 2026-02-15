# Implementation Plan: Trait Visualization

## Overview

Add population stats panel and actor inspection to the Bevy visualization. All new code in `src/viz_bevy/`. Pure formatting functions extracted for testability. Follows existing marker component + resource + `is_changed()` patterns.

## Tasks

- [x] 1. Add new resources and marker components
  - [x] 1.1 Define `TraitStats`, `SingleTraitStats`, `SelectedActor`, `StatsPanelVisible`, `StatsPanel`, and `ActorInspector` in `src/viz_bevy/resources.rs`
    - `TraitStats`: `actor_count: usize`, `tick: u64`, `traits: Option<[SingleTraitStats; 4]>`
    - `SingleTraitStats`: `min`, `max`, `mean`, `p25`, `p50`, `p75` (all `f32`)
    - `SelectedActor(pub Option<usize>)` — default `None`
    - `StatsPanelVisible(pub bool)` — default `false`
    - `StatsPanel` and `ActorInspector` marker components
    - _Requirements: 1.2, 1.3, 3.1_

- [x] 2. Implement trait stats computation
  - [x] 2.1 Write `compute_trait_stats_from_actors` pure function and `compute_trait_stats` Bevy system in `src/viz_bevy/systems.rs`
    - Pure function takes `impl Iterator<Item = &Actor>` and `tick: u64`, returns `TraitStats`
    - Collect trait values from non-inert actors into four `Vec<f32>`, sort with `total_cmp`, compute min/max/mean/percentiles via nearest-rank
    - Handle zero actors (traits: None) and single actor (all stats equal) edge cases
    - Bevy system reads `Res<SimulationState>`, writes `ResMut<TraitStats>`
    - _Requirements: 1.1, 1.3, 1.4, 1.5_

  - [ ]* 2.2 Write property test for stats computation (Property 1)
    - **Property 1: Stats reflect living actors only**
    - Generate random `Vec<Actor>` with random `inert` flags (0..200 actors)
    - Verify `actor_count` equals non-inert count, stats computed from non-inert actors only
    - Cover edge cases: zero actors, one actor
    - **Validates: Requirements 1.1, 1.3, 1.4, 1.5**

- [x] 3. Implement stats panel formatting and display
  - [x] 3.1 Write `format_trait_stats` pure function in `src/viz_bevy/setup.rs`
    - Format tick, actor count, and four trait rows with min/p25/p50/p75/max/mean to two decimal places
    - Handle `traits: None` case with "No living actors." message
    - _Requirements: 2.2, 2.3_

  - [x] 3.2 Spawn `StatsPanel` entity in `setup` system and implement `stats_panel_input` + `update_stats_panel` systems
    - Spawn hidden text entity with `StatsPanel` marker, semi-transparent background, positioned top-right (to the left of scale bar)
    - `stats_panel_input`: toggle `StatsPanelVisible` on `T` key press
    - `update_stats_panel`: sync text content from `TraitStats` and visibility from `StatsPanelVisible`, gated on `is_changed()`
    - Insert `StatsPanelVisible(false)` and initial `TraitStats` resources in setup
    - _Requirements: 2.1, 2.4, 2.5, 2.6_

  - [ ]* 3.3 Write property test for stats panel formatting (Property 2) and toggle (Property 8)
    - **Property 2: Stats panel formatting completeness**
    - Generate random `TraitStats` with `actor_count > 0`, verify output contains all trait names and stat values
    - **Property 8: Stats panel toggle**
    - For any initial bool, toggling produces negation
    - **Validates: Requirements 2.1, 2.2, 2.3**

- [x] 4. Implement actor selection
  - [x] 4.1 Write `select_actor_input` system in `src/viz_bevy/systems.rs`
    - On left-click: reuse cursor → world → grid cell mapping from `update_hover_tooltip`, look up occupancy, write `SelectedActor`
    - Extract coordinate mapping into a shared helper function to avoid duplication with `update_hover_tooltip`
    - _Requirements: 3.1, 3.2, 3.5_

  - [x] 4.2 Modify `handle_input` to gate Escape on `SelectedActor`
    - When `SelectedActor` is `Some`: clear to `None`, do not exit
    - When `SelectedActor` is `None`: exit as before
    - Also handle Escape deselection in `select_actor_input` or `handle_input` (one place only)
    - _Requirements: 3.3, 6.3, 6.4_

  - [x] 4.3 Add stale selection detection in `update_actor_inspector` or a dedicated system
    - If `SelectedActor` holds a slot index that no longer maps to a living actor, clear to `None`
    - _Requirements: 3.4_

  - [ ]* 4.4 Write property tests for selection logic (Properties 3, 4, 7)
    - **Property 3: Click selection matches occupancy**
    - Generate random occupancy map and cell index, verify selection equals occupancy
    - **Property 4: Stale selection cleared**
    - Generate random slot index and actor registry state, verify stale selections are cleared
    - **Property 7: Escape dispatch by selection state**
    - Generate random `Option<usize>`, verify Escape behavior
    - **Validates: Requirements 3.1, 3.2, 3.4, 6.3, 6.4**

- [x] 5. Implement actor inspector panel
  - [x] 5.1 Write `format_actor_info` pure function in `src/viz_bevy/setup.rs`
    - Format slot index, active/inert state, grid position (col, row from cell_index and grid_width), energy (2dp), four trait values (4dp)
    - _Requirements: 4.1, 4.2_

  - [x] 5.2 Spawn `ActorInspector` entity in `setup` and implement `update_actor_inspector` system
    - Spawn hidden text entity with `ActorInspector` marker, semi-transparent background, positioned bottom-left above hover tooltip
    - System reads `SelectedActor` and `SimulationState`, updates text and visibility
    - Hidden when `SelectedActor` is `None`, visible with formatted actor info when `Some`
    - _Requirements: 4.3, 4.4, 4.5, 4.6_

  - [ ]* 5.3 Write property test for actor inspector formatting (Property 5)
    - **Property 5: Actor inspector formatting completeness**
    - Generate random `Actor` and grid width, verify output contains slot index, energy, position, state, and all four trait values
    - **Validates: Requirements 4.1, 4.2**

- [x] 6. Implement selected actor highlight
  - [x] 6.1 Modify `update_texture` in `src/viz_bevy/systems.rs` to read `SelectedActor` and render cyan highlight
    - After the existing white-pixel actor overlay loop, check `SelectedActor`
    - If `Some(slot_index)`: find actor's `cell_index`, overwrite pixel with `[0, 255, 255, 255]`
    - _Requirements: 5.1, 5.2, 5.3_

  - [ ]* 6.2 Write property test for highlight color correctness (Property 6)
    - **Property 6: Highlight color correctness**
    - Generate random occupancy map and optional selection, verify pixel colors
    - **Validates: Requirements 5.1, 5.2**

- [x] 7. Register new systems in BevyVizPlugin
  - [x] 7.1 Register all new systems in `src/viz_bevy/mod.rs`
    - `compute_trait_stats` in `FixedUpdate` with `.after(tick_simulation)`
    - `stats_panel_input`, `update_stats_panel`, `select_actor_input`, `update_actor_inspector` in `Update`
    - Ensure `select_actor_input` runs before `update_texture` via `.before()` ordering
    - _Requirements: 1.1, 2.4, 6.1, 6.2_

- [x] 8. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Final checkpoint — Integration verification
  - Verify stats panel toggles with `T`, shows correct stats, updates each tick
  - Verify click-to-select works, inspector shows correct data, highlight follows actor
  - Verify Escape deselects before exiting, existing controls unaffected
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- No simulation core changes — all code in `src/viz_bevy/`
- No new config fields — the config-documentation steering rule does not apply
- Pure functions extracted for testability without Bevy App harness
- Property tests use `proptest` crate with minimum 100 iterations
