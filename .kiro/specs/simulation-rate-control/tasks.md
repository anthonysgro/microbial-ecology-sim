# Implementation Plan: Simulation Rate Control

## Overview

Add interactive simulation rate control (pause, resume, speed up, slow down, reset) to the Bevy visualization layer. All changes are confined to `src/viz_bevy/` and `src/bin/bevy_viz.rs`. The headless simulation core is untouched.

## Tasks

- [x] 1. Add `SimRateController` resource and `RateLabel` marker component
  - [x] 1.1 Define `SimRateController` struct and methods in `src/viz_bevy/resources.rs`
    - Add `SimRateController` with fields: `tick_hz: f64`, `paused: bool`, `initial_tick_hz: f64`
    - Add associated constants `MIN_HZ = 0.5` and `MAX_HZ = 480.0`
    - Implement `new(tick_hz: f64)`, `speed_up()`, `slow_down()`, `reset()`, `toggle_pause()`
    - Add `RateLabel` marker component
    - _Requirements: 1.1, 1.2, 1.3, 1.4_

  - [ ]* 1.2 Write property tests for `SimRateController`
    - **Property 1: SimRateController invariants hold after any operation sequence**
    - **Validates: Requirements 1.1, 1.3**
    - **Property 2: Pause toggle is an involution**
    - **Validates: Requirements 2.1**
    - **Property 4: speed_up doubles tick rate with upper clamp**
    - **Validates: Requirements 3.1, 3.2**
    - **Property 5: slow_down halves tick rate with lower clamp**
    - **Validates: Requirements 4.1, 4.2**
    - **Property 6: Reset restores initial rate**
    - **Validates: Requirements 5.1**

- [x] 2. Extract label formatting into a pure function and add the rate label formatting system
  - [x] 2.1 Add `format_rate_label(tick_hz: f64, paused: bool, running: bool) -> String` in `src/viz_bevy/systems.rs`
    - If `running == false`: return `"HALTED"`
    - If `paused == true`: return `"{tick_hz:.1} Hz — PAUSED"`
    - Otherwise: return `"{tick_hz:.1} Hz"`
    - _Requirements: 6.1, 7.2_

  - [x] 2.2 Add `update_rate_label` system in `src/viz_bevy/systems.rs`
    - Query `SimRateController`, `SimulationState`, and `Query<&mut Text, With<RateLabel>>`
    - Use Bevy change detection to skip when neither resource changed
    - Call `format_rate_label` and update text
    - _Requirements: 6.1, 6.2, 7.2_

  - [ ]* 2.3 Write property test for label formatting
    - **Property 7: Label formatting correctness**
    - **Validates: Requirements 6.1, 7.2**

- [x] 3. Add `rate_control_input` system in `src/viz_bevy/systems.rs`
  - Read `ButtonInput<KeyCode>`, mutate `SimRateController` and `Time<Fixed>`
  - Space → `toggle_pause()`, Up → `speed_up()`, Down → `slow_down()`, R → `reset()`
  - After rate change, set `Time<Fixed>` timestep to `Duration::from_secs_f64(1.0 / tick_hz)`
  - _Requirements: 2.1, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2_

- [x] 4. Modify `tick_simulation` to respect pause state
  - Add `rate: Res<SimRateController>` parameter to `tick_simulation` in `src/viz_bevy/systems.rs`
  - Add early return when `rate.paused` is true (after existing `!sim.running` check)
  - _Requirements: 2.2, 2.3, 2.4, 7.1_

  - [ ]* 4.1 Write property test for tick advancement guard
    - **Property 3: Tick advances if and only if running and not paused**
    - **Validates: Requirements 2.2, 2.3, 2.4, 7.1**

- [x] 5. Wire everything together in plugin and setup
  - [x] 5.1 Update `setup` in `src/viz_bevy/setup.rs`
    - Insert `SimRateController::new(config.tick_hz)` as a resource
    - Spawn `RateLabel` text entity positioned top-right (non-overlapping with existing UI)
    - _Requirements: 1.4, 6.3_

  - [x] 5.2 Register new systems in `BevyVizPlugin::build` in `src/viz_bevy/mod.rs`
    - Add `rate_control_input` and `update_rate_label` to the `Update` schedule
    - _Requirements: 3.3, 4.3, 5.2, 6.2_

  - [ ]* 5.3 Write unit tests for edge cases
    - Test `speed_up` at `MAX_HZ` is a no-op
    - Test `slow_down` at `MIN_HZ` is a no-op
    - Test `reset` after multiple speed changes restores exact initial value
    - Test label shows "HALTED" when `running == false` regardless of pause state
    - _Requirements: 3.2, 4.2, 5.1, 7.2_

- [x] 6. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- All changes are in `src/viz_bevy/` and `src/bin/bevy_viz.rs` — simulation core is untouched
- `proptest` crate needed for property-based tests (add to `[dev-dependencies]`)
- Property tests validate `SimRateController` as a pure data struct — no Bevy harness required
