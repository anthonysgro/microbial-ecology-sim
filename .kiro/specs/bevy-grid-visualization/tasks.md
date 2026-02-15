# Implementation Plan: Bevy Grid Visualization

## Overview

Incremental build-up of the `viz_bevy` module: pure functions first (normalize, color, pixel buffer), then Bevy resources and setup, then systems (tick, render, input, camera, label), wired together into a running app. Each step builds on the previous and is testable in isolation.

## Tasks

- [x] 1. Add Bevy dependency and create module skeleton
  - Add `bevy` to `[dependencies]` in `Cargo.toml` (use `default-features = false` with `bevy_core_pipeline`, `bevy_render`, `bevy_sprite`, `bevy_ui`, `bevy_text`, `bevy_winit`, `bevy_input`)
  - Create `src/viz_bevy/mod.rs` with submodule declarations: `resources`, `color`, `normalize`, `systems`, `setup`
  - Create empty files for each submodule
  - Add `pub mod viz_bevy;` to `src/lib.rs`
  - Verify compilation with `cargo check`
  - _Requirements: 1.1, 1.2, 1.3_

- [-] 2. Implement pure normalization function
  - [x] 2.1 Implement `normalize_field` in `src/viz_bevy/normalize.rs`
    - Signature: `pub fn normalize_field(raw: &[f32], out: &mut [f32]) -> f32`
    - Takes a pre-allocated `&mut [f32]` output slice (no Vec operations, no allocation)
    - Divide each value by max; handle near-zero max (all zeros) and uniform non-zero (all ones)
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ]* 2.2 Write property test for normalization
    - **Property 1: Normalization bounds**
    - **Validates: Requirements 3.1, 3.2, 3.3**

- [-] 3. Implement color mapping functions
  - [x] 3.1 Implement `heat_color_rgba` and `chemical_color_rgba` in `src/viz_bevy/color.rs`
    - `pub fn heat_color_rgba(normalized: f32) -> [u8; 4]` — blue→cyan→green→yellow→red gradient, alpha=255
    - `pub fn chemical_color_rgba(normalized: f32) -> [u8; 4]` — dark-green→bright-green, alpha=255
    - Clamp input to [0.0, 1.0] before mapping
    - Implement `pub fn fill_pixel_buffer(norm_buffer: &[f32], pixel_buffer: &mut [u8], color_fn: fn(f32) -> [u8; 4])` — writes RGBA into pre-allocated pixel buffer
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.2_

  - [ ]* 3.2 Write property tests for color mapping
    - **Property 2: Color mapper output invariants (alpha=255, clamping)**
    - **Validates: Requirements 4.3, 4.4**

  - [ ]* 3.3 Write property test for chemical green monotonicity
    - **Property 3: Chemical color green-channel monotonicity**
    - **Validates: Requirements 4.2**

  - [ ]* 3.4 Write property test for pixel buffer fill
    - **Property 4: Render pipeline pixel correctness**
    - **Validates: Requirements 5.2**

- [x] 4. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Define Bevy resources and marker components
  - [x] 5.1 Implement resources in `src/viz_bevy/resources.rs`
    - `SimulationState` resource: owns `Grid`, `GridConfig`, `tick: u64`, `running: bool`
    - `RenderState` resource: owns `pixel_buffer: Vec<u8>`, `norm_buffer: Vec<f32>`
    - `ActiveOverlay` resource enum: `Heat`, `Chemical(usize)`
    - `BevyVizConfig` resource: `seed`, `grid_config`, `init_config`, `initial_overlay`, `tick_hz`, `zoom_min`, `zoom_max`, `zoom_speed`, `pan_speed`
    - Marker components: `GridSprite`, `OverlayLabel`, `MainCamera`
    - _Requirements: 2.1, 9.2_

- [x] 6. Implement startup system
  - [x] 6.1 Implement `setup` function in `src/viz_bevy/setup.rs`
    - Initialize `Grid` via `world_init::initialize` using config from `BevyVizConfig`
    - Insert `SimulationState` resource with the initialized grid
    - Insert `RenderState` with pre-allocated buffers (`pixel_buffer`: `width*height*4`, `norm_buffer`: `width*height`)
    - Insert `ActiveOverlay` from config
    - Create Bevy `Image` (Rgba8UnormSrgb, grid dimensions, nearest-neighbor sampling)
    - Spawn `Camera2d` entity with `MainCamera` marker
    - Spawn `Sprite` entity with `GridSprite` marker, referencing the texture handle
    - Spawn UI text node at top-left with `OverlayLabel` marker, displaying initial overlay name
    - _Requirements: 5.1, 5.3, 5.4, 7.1, 7.2, 8.1, 9.2, 10.1_

- [-] 7. Implement simulation tick system
  - [-] 7.1 Implement `tick_simulation` in `src/viz_bevy/systems.rs`
    - Runs in `FixedUpdate` schedule
    - Skip if `running == false`
    - Call `TickOrchestrator::step(&mut grid, &config)`
    - Increment tick counter on success
    - On error: log via `tracing::error!`, set `running = false`
    - _Requirements: 2.2, 2.4, 2.5_

  - [ ]* 7.2 Write property test for tick counter advancement
    - **Property 9: Tick counter advancement**
    - **Validates: Requirements 2.2**

- [~] 8. Implement texture update system
  - [ ] 8.1 Implement `update_texture` in `src/viz_bevy/systems.rs`
    - Runs in `Update` schedule
    - Select field buffer based on `ActiveOverlay` (`read_heat()` or `read_chemical(species)`)
    - Call `normalize_field` into `RenderState.norm_buffer`
    - Call `fill_pixel_buffer` into `RenderState.pixel_buffer` with appropriate color function
    - Copy `pixel_buffer` into `image.data` via `copy_from_slice`
    - Zero per-frame allocations: all buffers pre-allocated
    - _Requirements: 5.2, 9.1, 9.3_

- [~] 9. Implement input handling system
  - [ ] 9.1 Implement `handle_input` in `src/viz_bevy/systems.rs`
    - Runs in `Update` schedule
    - `H` key → set `ActiveOverlay::Heat`
    - Digit `1`–`9` → set `ActiveOverlay::Chemical(digit - 1)` if index < `num_chemicals`
    - `Escape` or `Q` → send `AppExit` event
    - _Requirements: 6.1, 6.2, 6.3, 10.2_

  - [ ]* 9.2 Write property test for overlay key mapping
    - **Property 5: Overlay key mapping correctness**
    - **Validates: Requirements 6.2, 6.3**

- [~] 10. Implement overlay label update system
  - [ ] 10.1 Implement `update_overlay_label` in `src/viz_bevy/systems.rs`
    - Runs in `Update` schedule
    - Query `OverlayLabel` text entity
    - Set text to `"Heat"` or `"Chemical N"` based on `ActiveOverlay`
    - _Requirements: 6.4, 7.3_

  - [ ]* 10.2 Write property test for label-overlay sync
    - **Property 6: Label-overlay text sync**
    - **Validates: Requirements 6.4**

- [~] 11. Implement camera control system
  - [ ] 11.1 Implement `camera_controls` in `src/viz_bevy/systems.rs`
    - Runs in `Update` schedule
    - Mouse wheel up → decrease orthographic scale (zoom in)
    - Mouse wheel down → increase orthographic scale (zoom out)
    - Middle mouse button drag → translate camera position
    - Clamp scale to `[zoom_min, zoom_max]` from `BevyVizConfig`
    - _Requirements: 8.2, 8.3, 8.4, 8.5_

  - [ ]* 11.2 Write property test for zoom direction
    - **Property 7: Zoom direction correctness**
    - **Validates: Requirements 8.2, 8.3**

  - [ ]* 11.3 Write property test for zoom clamping
    - **Property 8: Zoom clamping invariant**
    - **Validates: Requirements 8.5**

- [~] 12. Wire plugin and app entry point
  - [ ] 12.1 Implement `BevyVizPlugin` in `src/viz_bevy/mod.rs`
    - Define a Bevy `Plugin` that registers all systems and schedules:
      - `Startup`: `setup`
      - `FixedUpdate`: `tick_simulation` (configure timestep from `BevyVizConfig::tick_hz`)
      - `Update`: `handle_input`, `update_texture`, `camera_controls`, `update_overlay_label`
    - _Requirements: 2.3, 2.4_

  - [ ] 12.2 Update `src/main.rs` or create a separate binary entry point
    - Parse CLI args (seed, optional `--bevy` flag or separate binary)
    - Construct `BevyVizConfig` with defaults
    - Build Bevy `App`, insert `BevyVizConfig` resource, add `BevyVizPlugin`, run
    - _Requirements: 10.1, 10.3_

- [~] 13. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Pure functions (normalize, color, pixel buffer) are tested independently of Bevy
- Property tests use `proptest` (already in dev-dependencies)
- The simulation crate remains unchanged — no Bevy types leak into `src/grid/`
