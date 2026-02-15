# Requirements Document

## Introduction

This document specifies the requirements for a Bevy-based 2D visualization frontend for the existing headless grid simulation engine. The visualization renders heat and chemical field data as a top-down heatmap texture, with keyboard-driven overlay switching, camera controls, and decoupled simulation/render timing. The core simulation crate remains engine-agnostic; the Bevy layer is a read-only consumer of simulation state.

## Glossary

- **Grid**: The top-level environment grid (`Grid` struct) owning all double-buffered field arrays, spatial partitions, and the source registry.
- **FieldBuffer**: A double-buffered contiguous `f32` array for a single physical field (heat or one chemical species). Exposes `read()` for the current state.
- **TickOrchestrator**: The per-tick execution driver that advances emission, diffusion, and heat radiation in deterministic order.
- **OverlayMode**: An enum selecting which field layer is visualized — `Heat` or `Chemical(usize)`.
- **Simulation_Adapter**: A Bevy resource wrapping the `Grid`, `GridConfig`, and tick counter, providing the interface between the engine-agnostic simulation and the Bevy ECS.
- **Pixel_Buffer**: A pre-allocated `Vec<u8>` (RGBA) matching grid dimensions, rewritten each frame from normalized field data and uploaded to a GPU texture.
- **Overlay_Label**: A Bevy UI text element displaying the name of the currently active overlay.
- **Camera_Rig**: A 2D orthographic camera entity supporting pan and zoom controls.
- **Fixed_Timestep**: A Bevy schedule configuration that advances the simulation at a constant rate independent of rendering frame rate.

## Requirements

### Requirement 1: Crate Isolation

**User Story:** As a simulation developer, I want the Bevy visualization to live in a separate module so that the core simulation crate never depends on Bevy.

#### Acceptance Criteria

1. THE Simulation_Adapter SHALL depend on the `emergent_sovereignty::grid` public API without requiring Bevy types in the simulation crate.
2. WHEN the Bevy visualization module is compiled, THE simulation crate SHALL compile independently with zero Bevy dependencies.
3. THE Bevy visualization module SHALL import `Grid`, `GridConfig`, `TickOrchestrator`, and `OverlayMode` from the simulation crate's public API.

### Requirement 2: Simulation Ownership and Tick Advancement

**User Story:** As a simulation developer, I want the Bevy app to own and advance the simulation on a fixed timestep so that rendering frame rate does not affect simulation determinism.

#### Acceptance Criteria

1. THE Simulation_Adapter SHALL own a `Grid` instance and a `GridConfig` instance as a Bevy resource.
2. WHEN the Fixed_Timestep fires, THE Simulation_Adapter SHALL call `TickOrchestrator::step` exactly once and increment the tick counter.
3. THE Fixed_Timestep interval SHALL be configurable at app startup.
4. WHILE the simulation is running, THE rendering frame rate SHALL have no effect on the number of ticks executed per wall-clock second.
5. IF `TickOrchestrator::step` returns an error, THEN THE Simulation_Adapter SHALL log the error via `tracing` and halt tick advancement.

### Requirement 3: Field Normalization

**User Story:** As a visualization developer, I want field values normalized to [0.0, 1.0] so that color mapping produces consistent visual output regardless of absolute magnitudes.

#### Acceptance Criteria

1. WHEN a field buffer is read for rendering, THE normalization function SHALL divide each value by the maximum value in the buffer, producing output in [0.0, 1.0].
2. WHEN the maximum value in a field buffer is near zero (absolute value < 1e-9), THE normalization function SHALL output 0.0 for every cell.
3. WHEN all values in a field buffer are identical and non-zero, THE normalization function SHALL output 1.0 for every cell.

### Requirement 4: Color Mapping

**User Story:** As a user, I want heat and chemical fields rendered with distinct color gradients so that I can visually distinguish field types and value magnitudes.

#### Acceptance Criteria

1. WHEN rendering the Heat overlay, THE color mapper SHALL interpolate across a blue→cyan→green→yellow→red gradient with stops at 0.0, 0.25, 0.50, 0.75, and 1.0.
2. WHEN rendering a Chemical overlay, THE color mapper SHALL interpolate a dark-green-to-bright-green gradient from normalized 0.0 to 1.0.
3. THE color mapper SHALL clamp input values outside [0.0, 1.0] to the nearest endpoint before mapping.
4. THE color mapper SHALL produce RGBA `[u8; 4]` output with alpha fixed at 255.

### Requirement 5: Texture-Based Rendering

**User Story:** As a user, I want the grid rendered as a GPU texture so that large grids display efficiently without per-cell sprite overhead.

#### Acceptance Criteria

1. THE renderer SHALL create a single Bevy `Image` with dimensions matching the grid width and height, using `Rgba8UnormSrgb` pixel format.
2. WHEN a new frame is rendered, THE renderer SHALL write normalized and color-mapped RGBA values into the Pixel_Buffer and upload the buffer to the GPU texture.
3. THE renderer SHALL display the texture on a screen-filling quad using a Bevy `Sprite` entity.
4. THE Pixel_Buffer SHALL be pre-allocated at startup with capacity `width * height * 4` bytes and reused each frame without reallocation.

### Requirement 6: Overlay Switching

**User Story:** As a user, I want to switch between heat and chemical overlays using keyboard keys so that I can inspect different field layers at runtime.

#### Acceptance Criteria

1. WHEN the user presses the `H` key, THE input system SHALL set the active OverlayMode to `Heat`.
2. WHEN the user presses a digit key `1` through `9`, THE input system SHALL set the active OverlayMode to `Chemical(digit - 1)`, provided the chemical species index is within the grid's `num_chemicals` range.
3. WHEN the user presses a digit key for a chemical species index outside the valid range, THE input system SHALL ignore the keypress and retain the current OverlayMode.
4. WHEN the active OverlayMode changes, THE Overlay_Label SHALL update its displayed text to reflect the new mode name.

### Requirement 7: Overlay Label Display

**User Story:** As a user, I want to see which overlay is currently active so that I can orient myself when switching between field views.

#### Acceptance Criteria

1. THE Overlay_Label SHALL be rendered as a Bevy UI text node positioned at the top-left corner of the screen.
2. WHEN the application starts, THE Overlay_Label SHALL display the name of the initial OverlayMode.
3. WHEN the OverlayMode changes, THE Overlay_Label SHALL update within the same frame.

### Requirement 8: Camera Controls

**User Story:** As a user, I want to pan and zoom the 2D view so that I can inspect specific regions of the grid at different scales.

#### Acceptance Criteria

1. THE Camera_Rig SHALL use a 2D orthographic projection.
2. WHEN the user scrolls the mouse wheel up, THE Camera_Rig SHALL decrease the orthographic scale (zoom in).
3. WHEN the user scrolls the mouse wheel down, THE Camera_Rig SHALL increase the orthographic scale (zoom out).
4. WHEN the user holds the middle mouse button and drags, THE Camera_Rig SHALL translate the camera position proportionally to the drag delta.
5. THE Camera_Rig SHALL clamp the orthographic scale to a configurable minimum and maximum range to prevent degenerate zoom levels.

### Requirement 9: Render-Loop Allocation Discipline

**User Story:** As a performance-conscious developer, I want the per-frame render path to perform zero heap allocations so that the visualization scales to large grids without GC-like stalls.

#### Acceptance Criteria

1. WHILE the render system executes, THE renderer SHALL reuse the pre-allocated Pixel_Buffer and normalization buffer without calling `Vec::push`, `Vec::resize`, or any allocating operation.
2. THE normalization buffer SHALL be pre-allocated at startup with capacity equal to the grid cell count.
3. WHEN writing pixel data, THE renderer SHALL index directly into the pre-allocated Pixel_Buffer using computed offsets rather than constructing intermediate collections.

### Requirement 10: Application Lifecycle

**User Story:** As a user, I want the Bevy app to initialize the simulation from a seed and configuration, and exit cleanly on quit.

#### Acceptance Criteria

1. WHEN the application starts, THE Simulation_Adapter SHALL initialize the Grid using `world_init::initialize` with a configurable seed and `GridConfig`.
2. WHEN the user presses `Escape` or `Q`, THE application SHALL exit the Bevy event loop cleanly.
3. THE application SHALL accept the simulation seed as a command-line argument, defaulting to 42 when no argument is provided.
