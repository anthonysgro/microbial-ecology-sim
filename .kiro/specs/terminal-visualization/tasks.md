# Implementation Plan: Terminal Visualization

## Overview

Implement a COLD-path terminal renderer in `src/viz/` that reads `Grid` field buffers and draws character+color heatmaps via `crossterm`. Pure logic (glyph mapping, color mapping, normalization, stats, input mapping, clipping) is isolated from terminal I/O for testability. The render loop lives in `main.rs`.

## Tasks

- [x] 1. Create viz module structure and core types
  - [x] 1.1 Create `src/viz/mod.rs` with submodule declarations and `OverlayMode` enum with `label()` method, `RendererConfig` struct, and `InputAction` enum
    - _Requirements: 6.1, 6.2, 6.3, 6.5_

- [x] 2. Implement glyph mapping
  - [x] 2.1 Create `src/viz/glyph.rs` with `value_to_glyph(normalized: f32) -> char` using the threshold sequence: space < 0.01, `.` [0.01, 0.25), `:` [0.25, 0.50), `*` [0.50, 0.75), `#` [0.75, 1.0]. Also add `glyph_to_range(ch: char) -> (f32, f32)` returning the threshold range for round-trip verification.
    - _Requirements: 1.1, 1.4, 9.2_
  - [ ]* 2.2 Write property test: Glyph threshold correctness
    - **Property 1: Glyph threshold correctness**
    - **Validates: Requirements 1.1, 1.4**
  - [ ]* 2.3 Write property test: Glyph round-trip consistency
    - **Property 8: Glyph round-trip consistency**
    - **Validates: Requirements 9.2**

- [x] 3. Implement normalization
  - [x] 3.1 Create normalization function `normalize_field(raw: &[f32], out: &mut Vec<f32>) -> f32` in `src/viz/renderer.rs` (or a dedicated `src/viz/normalize.rs`). Divides by max, guards against near-zero max with epsilon 1e-9. When all values identical and non-zero, normalizes to 1.0.
    - _Requirements: 1.2, 1.3, 9.1, 9.3_
  - [ ]* 3.2 Write property test: Normalization bounds
    - **Property 2: Normalization bounds**
    - **Validates: Requirements 1.2**
  - [ ]* 3.3 Write unit tests for normalization edge cases
    - All-zero buffer → all 0.0 (Req 1.3)
    - Identical non-zero buffer → all 1.0 (Req 9.3)
    - _Requirements: 1.3, 9.3_

- [-] 4. Implement color mapping
  - [x] 4.1 Create `src/viz/color.rs` with `heat_color(normalized: f32) -> Color` (blue→cyan→green→yellow→red RGB interpolation), `moisture_bg_color(normalized: f32) -> Color` (single-hue blue gradient), and `chemical_color(normalized: f32) -> Color` (green-scale gradient)
    - _Requirements: 2.1, 2.2, 3.1, 3.2_
  - [ ]* 4.2 Write property test: Heat color gradient monotonicity
    - **Property 3: Heat color gradient monotonicity**
    - **Validates: Requirements 2.1**
  - [ ]* 4.3 Write property test: Moisture blue-channel dominance
    - **Property 4: Moisture blue-channel dominance**
    - **Validates: Requirements 3.2**

- [x] 5. Implement input handling
  - [x] 5.1 Create `src/viz/input.rs` with `map_key_event(event: KeyEvent, num_chemicals: usize) -> InputAction` mapping digit keys to chemical overlays (with bounds check), 'h' to Heat, 'm' to Moisture, 'q'/Esc to Quit, everything else to None
    - _Requirements: 5.3, 6.1, 6.2, 6.3, 6.4_
  - [ ]* 5.2 Write property test: Key-to-overlay mapping correctness
    - **Property 5: Key-to-overlay mapping correctness**
    - **Validates: Requirements 6.1, 6.4**
  - [ ]* 5.3 Write unit tests for specific key mappings
    - 'h' → Heat, 'm' → Moisture, 'q' → Quit, Esc → Quit (Req 5.3, 6.2, 6.3)
    - _Requirements: 5.3, 6.2, 6.3_

- [-] 6. Implement stats computation and formatting
  - [x] 6.1 Create `src/viz/stats.rs` with `FieldStats` struct, `compute_stats(buffer: &[f32], center_index: usize) -> FieldStats`, and `format_stats_bar(tick: u64, overlay: &OverlayMode, stats: &FieldStats) -> String` that includes tick, overlay label, total, min, max, center
    - _Requirements: 5.4, 6.5, 7.1, 7.2_
  - [ ]* 6.2 Write property test: Stats bar completeness
    - **Property 6: Stats bar completeness**
    - **Validates: Requirements 5.4, 6.5, 7.1**

- [ ] 7. Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Implement renderer with terminal I/O
  - [ ] 8.1 Create `src/viz/renderer.rs` with `Renderer` struct holding `RendererConfig`, `OverlayMode`, terminal dimensions, stdout handle, and pre-allocated `norm_buffer: Vec<f32>`. Implement `init()` (enter alt screen, raw mode, hide cursor, query terminal size), `shutdown()` (restore screen, show cursor, disable raw mode), `set_overlay()`, and viewport clipping logic: `render_width = min(grid.width(), terminal_width)`, `render_height = min(grid.height(), terminal_height - stats_lines)`.
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 8.1, 8.2, 8.3_
  - [ ] 8.2 Implement `render_frame(&mut self, grid: &Grid, tick: u64) -> anyhow::Result<()>` that: selects the field buffer based on current overlay mode, normalizes it, computes stats, moves cursor to origin, iterates over clipped rows/cols writing glyph+color per cell, then writes the stats bar below the grid
    - _Requirements: 4.1, 4.2, 5.1, 7.2, 7.3_
  - [ ] 8.3 Implement `poll_input(&mut self, num_chemicals: usize) -> anyhow::Result<InputAction>` using `crossterm::event::poll()` with zero timeout for non-blocking key reads, delegating to `map_key_event()`
    - _Requirements: 5.3, 6.1, 6.2, 6.3, 6.4_
  - [ ]* 8.4 Write property test: Viewport clipping
    - **Property 7: Viewport clipping**
    - **Validates: Requirements 8.2**

- [ ] 9. Wire render loop into main.rs
  - [ ] 9.1 Update `src/main.rs` to import the viz module, construct `RendererConfig` with a default frame delay (e.g., 50ms) and initial overlay `Chemical(0)`, call `Renderer::init()`, run the tick loop calling `TickOrchestrator::step()` then `render_frame()` then `poll_input()` then `sleep()`, and call `shutdown()` on exit. Ensure `shutdown()` runs on all exit paths (normal, error, quit key).
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ] 9.2 Add `pub mod viz;` to `src/lib.rs` (or keep viz private to the binary if not in lib.rs — wire appropriately)
    - _Requirements: N/A (project structure)_

- [ ] 10. Final checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- All visualization code is COLD-path — `anyhow`, allocations, and dynamic dispatch are permitted
- `crossterm 0.28` is already in `Cargo.toml`
- `proptest` is already in `dev-dependencies`
- Pure logic is isolated from terminal I/O for testability
- Property tests validate universal correctness; unit tests cover edge cases and specific examples
