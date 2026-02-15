# Requirements Document

## Introduction

Terminal-based real-time visualization for the emergent-sovereignty environment grid simulation. The Renderer reads grid field buffers (chemicals, heat, moisture) after each simulation tick and draws a character/color heatmap to the terminal using ANSI escape codes via `crossterm`. This is COLD-path, application-boundary code — allocations, `anyhow`, and dynamic dispatch are permitted. The visualization lives outside the core simulation grid module (`src/viz/` or `src/main.rs`).

## Glossary

- **Grid**: The `Grid` struct owning all double-buffered SoA field arrays (chemicals, heat, moisture) with read accessors `read_heat()`, `read_moisture()`, `read_chemical(species)`.
- **Renderer**: The visualization module responsible for converting grid field data into terminal output.
- **Field_Layer**: A selectable data source for visualization — one of: a specific chemical species index, heat, or moisture.
- **Cell_Glyph**: The character used to represent a single grid cell's value in the terminal (e.g., `.`, `:`, `*`, `#`).
- **Color_Map**: A mapping from a normalized field value in `[0.0, 1.0]` to an ANSI terminal color.
- **Tick_Orchestrator**: The `TickOrchestrator::step()` function that advances the simulation by one tick.
- **Render_Frame**: A single complete redraw of the grid visualization in the terminal.
- **Stats_Bar**: A line of aggregated statistics displayed alongside the grid (totals, min, max, center cell value).
- **Overlay_Mode**: The currently active Field_Layer being visualized.

## Requirements

### Requirement 1: Value-to-Glyph Mapping

**User Story:** As a developer, I want grid cell values mapped to distinct ASCII characters, so that I can visually distinguish concentration levels without color support.

#### Acceptance Criteria

1. WHEN the Renderer maps a normalized field value to a Cell_Glyph, THE Renderer SHALL use the threshold sequence: ` ` (space) for values below 0.01, `.` for [0.01, 0.25), `:` for [0.25, 0.50), `*` for [0.50, 0.75), `#` for [0.75, 1.0].
2. WHEN the Renderer normalizes a raw field value, THE Renderer SHALL divide the raw value by the maximum value in the current field buffer, producing a result in [0.0, 1.0].
3. IF the maximum value in a field buffer is zero or below a minimum epsilon (1e-9), THEN THE Renderer SHALL treat all cells as having normalized value 0.0.
4. THE Renderer SHALL produce exactly one Cell_Glyph per grid cell with no ambiguity — every normalized value maps to exactly one glyph.

### Requirement 2: Heat Color Mapping

**User Story:** As a developer, I want heat values rendered with a blue-to-red ANSI color gradient, so that I can visually identify thermal hotspots and cold zones.

#### Acceptance Criteria

1. WHEN the Renderer maps a normalized heat value to a Color_Map entry, THE Renderer SHALL interpolate from blue (cold, value near 0.0) through cyan, green, yellow, to red (hot, value near 1.0).
2. WHEN the terminal does not support 256-color or truecolor, THE Renderer SHALL fall back to the 16 standard ANSI colors using the nearest match.
3. THE Color_Map SHALL produce a deterministic color for any given normalized value — the same input always yields the same output.

### Requirement 3: Moisture Visualization

**User Story:** As a developer, I want moisture values rendered with a distinct visual encoding, so that I can differentiate moisture from heat and chemical overlays.

#### Acceptance Criteria

1. WHEN the Overlay_Mode is set to moisture, THE Renderer SHALL display moisture values using background color shading (dark-to-bright blue gradient) combined with the Cell_Glyph for the normalized value.
2. THE Renderer SHALL keep moisture visualization visually distinct from heat visualization by using a single-hue blue palette for moisture versus the multi-hue palette for heat.

### Requirement 4: In-Place Terminal Rendering

**User Story:** As a developer, I want the grid redrawn in place each tick without scrolling, so that I get a smooth animation effect in the terminal.

#### Acceptance Criteria

1. WHEN a Render_Frame begins, THE Renderer SHALL move the cursor to the top-left origin of the grid area using ANSI escape sequences (via `crossterm`).
2. THE Renderer SHALL overwrite the previous frame's content in place rather than appending new lines.
3. WHEN the Renderer starts for the first time, THE Renderer SHALL enter an alternate screen buffer and hide the cursor.
4. WHEN the Renderer shuts down, THE Renderer SHALL restore the original screen buffer, show the cursor, and reset all terminal attributes.

### Requirement 5: Tick-Driven Render Loop

**User Story:** As a developer, I want the visualization to update after each simulation tick with a configurable delay, so that I can control animation speed.

#### Acceptance Criteria

1. WHEN the Tick_Orchestrator completes a tick, THE Renderer SHALL draw one Render_Frame reflecting the updated grid state.
2. THE Renderer SHALL accept a configurable sleep duration (in milliseconds) between frames to control animation speed.
3. WHEN the user presses 'q' or Escape during the render loop, THE Renderer SHALL exit the loop cleanly and restore terminal state.
4. THE Renderer SHALL display the current tick number in the Stats_Bar.

### Requirement 6: Overlay Mode Switching

**User Story:** As a developer, I want to switch between chemical, heat, and moisture overlays at runtime, so that I can inspect different fields without restarting.

#### Acceptance Criteria

1. WHEN the user presses '1' through '9', THE Renderer SHALL switch the Overlay_Mode to the chemical species at that index (1-indexed), provided the species exists.
2. WHEN the user presses 'h', THE Renderer SHALL switch the Overlay_Mode to heat.
3. WHEN the user presses 'm', THE Renderer SHALL switch the Overlay_Mode to moisture.
4. IF the user presses a chemical species key that exceeds the configured number of species, THEN THE Renderer SHALL ignore the keypress and retain the current Overlay_Mode.
5. THE Renderer SHALL display the name of the active Overlay_Mode in the Stats_Bar.

### Requirement 7: Live Statistics Display

**User Story:** As a developer, I want aggregated field statistics displayed alongside the grid, so that I can monitor simulation dynamics numerically.

#### Acceptance Criteria

1. WHEN a Render_Frame is drawn, THE Stats_Bar SHALL display: the current tick number, the active Overlay_Mode name, the field total, the field minimum, the field maximum, and the center cell value.
2. THE Renderer SHALL compute statistics from the same field buffer snapshot used for the current Render_Frame to maintain consistency.
3. THE Stats_Bar SHALL be rendered below the grid area without overlapping grid content.

### Requirement 8: Grid Dimension Support

**User Story:** As a developer, I want the visualization to handle grids of varying sizes, so that I can test small and large simulations.

#### Acceptance Criteria

1. THE Renderer SHALL render grids of any dimensions supported by the Grid struct (minimum 1×1).
2. WHEN the grid dimensions exceed the terminal viewport, THE Renderer SHALL clip the rendered output to the visible terminal area rather than wrapping or corrupting the display.
3. THE Renderer SHALL query the terminal size at startup and use it to determine the maximum renderable grid area.

### Requirement 9: Normalization Correctness

**User Story:** As a developer, I want field values normalized consistently, so that the visualization accurately represents the simulation state.

#### Acceptance Criteria

1. THE Renderer SHALL normalize field values per-frame using the current field buffer's maximum, ensuring the visualization adapts to changing value ranges across ticks.
2. FOR ALL valid field buffers, normalizing then mapping to a Cell_Glyph then mapping back to the threshold range SHALL produce a range that contains the original normalized value (round-trip consistency of the mapping).
3. WHEN all values in a field buffer are identical and non-zero, THE Renderer SHALL normalize all cells to 1.0.
