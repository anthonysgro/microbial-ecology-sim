# Implementation Plan: Config Info Panel

## Overview

Add a toggle-able info panel to the Bevy visualization layer. The implementation follows the existing marker-component + resource + stateless-system pattern. The pure formatting function is implemented and tested first, then wired into the Bevy ECS.

## Tasks

- [x] 1. Add marker component, visibility resource, and pure formatting function
  - [x] 1.1 Add `InfoPanel` marker component and `InfoPanelVisible` resource to `src/viz_bevy/resources.rs`
    - `InfoPanel`: plain `#[derive(Component)]` marker, no methods
    - `InfoPanelVisible(pub bool)`: plain `#[derive(Resource)]` struct, default `false`
    - _Requirements: 5.1, 5.2, 1.2_
  - [x] 1.2 Implement `format_config_info` pure function in `src/viz_bevy/setup.rs`
    - Signature: `pub(super) fn format_config_info(seed: u64, grid_config: &GridConfig, init_config: &WorldInitConfig, actor_config: Option<&ActorConfig>) -> String`
    - Sections: Seed, Grid, World Init (heat sources, chemical sources, initial ranges, actor range), Actors
    - Format all floats to consistent decimal precision
    - When `actor_config` is `None`, output "Actors: disabled"
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3_
  - [ ]* 1.3 Write property tests for `format_config_info`
    - **Property 2: Formatted output contains all config values**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2**
    - Use `proptest` to generate random `GridConfig`, `WorldInitConfig`, `Option<ActorConfig>`, and `u64` seed
    - Verify output string contains string representations of every field value
  - [ ]* 1.4 Write property test for float formatting consistency
    - **Property 3: Float formatting consistency**
    - **Validates: Requirements 3.3**
    - Generate random configs, verify all floats in output use consistent decimal precision
  - [ ]* 1.5 Write unit tests for edge cases and defaults
    - Test `InfoPanelVisible` default is `false` (Requirement 1.2)
    - Test `format_config_info` with `actor_config: None` contains "disabled" (Requirement 2.5)
    - Test `format_config_info` with empty `chemical_decay_rates` vec (edge case)

- [x] 2. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Add input system, update system, and spawn the panel entity
  - [x] 3.1 Implement `info_panel_input` system in `src/viz_bevy/systems.rs`
    - Check `keys.just_pressed(KeyCode::KeyI)`, toggle `InfoPanelVisible.0`
    - COLD PATH, follows `rate_control_input` pattern
    - _Requirements: 1.1, 1.3_
  - [x] 3.2 Implement `update_info_panel` system in `src/viz_bevy/systems.rs`
    - Gate on `visible.is_changed()`, set `Visibility::Visible` or `Visibility::Hidden` on `InfoPanel` entity
    - COLD PATH, follows `update_overlay_label` pattern
    - _Requirements: 1.1_
  - [x] 3.3 Spawn info panel entity in `src/viz_bevy/setup.rs` `setup` function
    - Call `format_config_info` with config data from `BevyVizConfig`
    - Spawn `Text` entity with `InfoPanel` marker, `Visibility::Hidden`, semi-transparent `BackgroundColor`, absolute positioning at `top: 40px, left: 10px`
    - Insert `InfoPanelVisible(false)` resource
    - _Requirements: 1.2, 4.1, 4.2_
  - [x] 3.4 Register new systems in `src/viz_bevy/mod.rs` `BevyVizPlugin::build`
    - Add `systems::info_panel_input` and `systems::update_info_panel` to the `Update` schedule
    - _Requirements: 5.3_
  - [ ]* 3.5 Write property test for toggle invariant
    - **Property 1: Toggle inverts visibility**
    - **Validates: Requirements 1.1**
    - For any boolean state, toggling produces negation; toggling twice restores original

- [x] 4. Update README with new key binding
  - [x] 4.1 Add `I` key to the Bevy mode key binding table in `README.md`
    - Add row: `i` | Show / hide config info panel
    - _Requirements: (documentation)_

- [ ] 5. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The pure formatting function (1.2) is implemented first so property tests can validate it before Bevy wiring
- This is entirely COLD path — no hot-path impact, no benchmark requirements
- `proptest` is the property-based testing library for all property tests
