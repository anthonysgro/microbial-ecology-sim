// WARM PATH: tick_simulation runs every FixedUpdate, advancing the grid.
// COLD PATH: input, camera, label systems run every Update frame.
// Allocation forbidden in tick_simulation. Standard rules for Update systems.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use crate::grid::tick::TickOrchestrator;

use super::{color, normalize};
use super::resources::{
    ActiveOverlay, BevyVizConfig, GridSprite, HoverTooltip, InfoPanel, InfoPanelVisible,
    MainCamera, OverlayLabel, RateLabel, RenderState, ScaleBar, ScaleMaxLabel,
    SimRateController, SimulationState,
};
use super::setup::{build_scale_image, overlay_label_text};

/// Advance the simulation by one tick.
///
/// Runs in `FixedUpdate`. Skips when:
///   1. `SimulationState.running == false` (error-halted), OR
///   2. `SimRateController.paused == true` (user-paused)
///
/// On error, logs via `tracing::error!` and sets `running = false` so
/// subsequent invocations become no-ops.
///
/// Requirements: 2.2, 2.3, 2.4, 7.1
pub fn tick_simulation(
    mut sim: ResMut<SimulationState>,
    rate: Res<SimRateController>,
    viz_config: Res<BevyVizConfig>,
) {
    if !sim.running || rate.paused {
        return;
    }

    let sim = &mut *sim;
    match TickOrchestrator::step(
        &mut sim.grid,
        &sim.config,
        sim.tick,
        &viz_config.init_config.heat_source_config,
        &viz_config.init_config.chemical_source_config,
    ) {
        Ok(()) => {
            sim.tick += 1;
        }
        Err(err) => {
            error!("tick {} failed: {err}", sim.tick);
            sim.running = false;
        }
    }
}

/// Update the GPU texture from the current simulation field data.
///
/// WARM PATH: Runs every `Update` frame. Zero per-frame allocations —
/// all buffers are pre-allocated in `RenderState`.
///
/// 1. Select field buffer based on `ActiveOverlay` (heat or chemical species).
/// 2. Normalize into `RenderState.norm_buffer`.
/// 3. Color-map into `RenderState.pixel_buffer`.
/// 4. Copy pixel buffer into the Bevy `Image` data.
///
/// Requirements: 5.2 (texture upload), 9.1 (buffer reuse), 9.3 (direct indexing).
pub fn update_texture(
    sim: Res<SimulationState>,
    overlay: Res<ActiveOverlay>,
    mut render: ResMut<RenderState>,
    mut images: ResMut<Assets<Image>>,
    query: Query<&Sprite, With<GridSprite>>,
    config: Res<BevyVizConfig>,
) {
    // Resolve the field slice and color function from the active overlay.
    let (field, color_fn): (&[f32], fn(f32) -> [u8; 4]) = match *overlay {
        ActiveOverlay::Heat => (sim.grid.read_heat(), color::heat_color_rgba),
        ActiveOverlay::Chemical(species) => {
            match sim.grid.read_chemical(species) {
                Ok(slice) => (slice, color::chemical_color_rgba),
                Err(err) => {
                    warn!("overlay chemical species {species} unavailable: {err}");
                    return;
                }
            }
        }
    };

    let render = &mut *render;

    // Normalize field values into the pre-allocated norm buffer.
    normalize::normalize_field(field, &mut render.norm_buffer, config.color_scale_max);

    // Color-map normalized values into the pre-allocated pixel buffer.
    color::fill_pixel_buffer(&render.norm_buffer, &mut render.pixel_buffer, color_fn);

    // Overlay actors as white pixels on occupied cells.
    let occupancy = sim.grid.occupancy();
    for (cell_index, slot) in occupancy.iter().enumerate() {
        if slot.is_some() {
            let offset = cell_index * 4;
            if offset + 3 < render.pixel_buffer.len() {
                render.pixel_buffer[offset] = 255;     // R
                render.pixel_buffer[offset + 1] = 255; // G
                render.pixel_buffer[offset + 2] = 255; // B
                render.pixel_buffer[offset + 3] = 255; // A
            }
        }
    }

    // Upload pixel buffer into the Bevy Image asset.
    let Ok(sprite) = query.single() else {
        return;
    };

    if let Some(image) = images.get_mut(&sprite.image) {
        if let Some(ref mut data) = image.data {
            data.copy_from_slice(&render.pixel_buffer);
        }
    }
}

/// Handle keyboard input for overlay switching and application exit.
///
/// COLD PATH: Runs every `Update` frame. Reads keyboard state, updates
/// `ActiveOverlay` on H/digit keys, sends `AppExit` on Escape/Q.
///
/// Key mapping:
/// - `H` → `ActiveOverlay::Heat`
/// - `1`–`9` → `ActiveOverlay::Chemical(digit - 1)` if index < num_chemicals
/// - `Escape` / `Q` → `AppExit`
///
/// Requirements: 6.1 (H key), 6.2 (digit keys), 6.3 (out-of-range ignored), 10.2 (quit).
pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<ActiveOverlay>,
    sim: Res<SimulationState>,
    mut exit: EventWriter<AppExit>,
) {
    // Quit on Escape or Q.
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }

    // H → Heat overlay.
    if keys.just_pressed(KeyCode::KeyH) {
        *overlay = ActiveOverlay::Heat;
        return;
    }

    // Digit 1–9 → Chemical overlay (if species index is valid).
    let num_chemicals = sim.config.num_chemicals;
    let digit_keys: [(KeyCode, usize); 9] = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
    ];

    for (key, species) in digit_keys {
        if keys.just_pressed(key) && species < num_chemicals {
            *overlay = ActiveOverlay::Chemical(species);
            return;
        }
    }
}

/// Read rate-control keys, mutate `SimRateController` and `Time<Fixed>`.
///
/// COLD PATH: Runs every `Update` frame. Only performs work when a
/// rate-control key is pressed (Space, Up, Down, R).
///
/// Key bindings:
///   Space      → toggle pause
///   Up Arrow   → speed up (×2, clamped to `MAX_HZ`)
///   Down Arrow → slow down (÷2, clamped to `MIN_HZ`)
///   R          → reset to initial rate
///
/// When the tick rate changes, the `Time<Fixed>` timestep is updated
/// in the same frame so the new rate takes effect immediately.
///
/// Requirements: 2.1, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2
pub fn rate_control_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut rate: ResMut<SimRateController>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    let mut rate_changed = false;

    if keys.just_pressed(KeyCode::Space) {
        rate.toggle_pause();
    }

    if keys.just_pressed(KeyCode::ArrowUp) {
        rate.speed_up();
        rate_changed = true;
    }

    if keys.just_pressed(KeyCode::ArrowDown) {
        rate.slow_down();
        rate_changed = true;
    }

    if keys.just_pressed(KeyCode::KeyR) {
        rate.reset();
        rate_changed = true;
    }

    if rate_changed {
        let period = std::time::Duration::from_secs_f64(1.0 / rate.tick_hz);
        fixed_time.set_timestep(period);
    }
}



/// Sync the overlay label text with the current `ActiveOverlay` value.
///
/// COLD PATH: Runs every `Update` frame. Only mutates the `Text` component
/// when the overlay has actually changed (Bevy's change detection on
/// `Res<ActiveOverlay>` gates the query).
///
/// Requirements: 6.4 (label updates on overlay change), 7.3 (same-frame update).
pub fn update_overlay_label(
    overlay: Res<ActiveOverlay>,
    mut query: Query<&mut Text, With<OverlayLabel>>,
) {
    if !overlay.is_changed() {
        return;
    }

    let label = overlay_label_text(&overlay);
    for mut text in &mut query {
        **text = label.clone();
    }
}

/// Format the rate label text from simulation state.
///
/// Pure function — no Bevy dependencies, testable in isolation.
///
/// Display logic:
///   - Error-halted (`running == false`): `"HALTED"`
///   - User-paused: `"{tick_hz:.1} Hz — PAUSED"`
///   - Running: `"{tick_hz:.1} Hz"`
///
/// Requirements: 6.1, 7.2
pub fn format_rate_label(tick_hz: f64, paused: bool, running: bool) -> String {
    if !running {
        "HALTED".to_string()
    } else if paused {
        format!("{tick_hz:.1} Hz \u{2014} PAUSED")
    } else {
        format!("{tick_hz:.1} Hz")
    }
}

/// Sync the rate label text with `SimRateController` and `SimulationState`.
///
/// COLD PATH: Runs every `Update` frame. Only mutates the `Text` component
/// when either resource has changed (Bevy change detection gates the update).
///
/// Requirements: 6.1, 6.2, 7.2
pub fn update_rate_label(
    rate: Res<SimRateController>,
    sim: Res<SimulationState>,
    mut query: Query<&mut Text, With<RateLabel>>,
) {
    if !rate.is_changed() && !sim.is_changed() {
        return;
    }

    let label = format_rate_label(rate.tick_hz, rate.paused, sim.running);
    for mut text in &mut query {
        **text = label.clone();
    }
}

/// Handle mouse wheel zoom and middle-button pan for the 2D camera.
///
/// COLD PATH: Runs every `Update` frame. Reads accumulated mouse scroll
/// for zoom and middle-button drag for panning. Clamps orthographic scale
/// to `[zoom_min, zoom_max]` from `BevyVizConfig`.
///
/// Zoom: scroll up (positive y) → decrease scale (zoom in),
///        scroll down (negative y) → increase scale (zoom out).
/// Pan:  middle mouse button held → translate camera by cursor delta
///        scaled by current projection scale and `pan_speed`.
///
/// Requirements: 8.2 (zoom in), 8.3 (zoom out), 8.4 (pan), 8.5 (clamp).
pub fn camera_controls(
    mouse_wheel: Res<AccumulatedMouseScroll>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    config: Res<BevyVizConfig>,
    mut last_cursor_pos: Local<Option<Vec2>>,
    windows: Query<&Window>,
) {
    let Ok((mut transform, mut projection)) = camera_q.single_mut() else {
        return;
    };

    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    // ── Zoom via mouse wheel ───────────────────────────────────────
    // Multiplicative zoom: scroll up (positive y) → zoom in (decrease scale).
    if mouse_wheel.delta.y != 0.0 {
        let zoom_factor = 1.0 + (-mouse_wheel.delta.y * config.zoom_speed);
        ortho.scale = (ortho.scale * zoom_factor)
            .clamp(config.zoom_min, config.zoom_max);
    }

    // ── Pan via middle mouse button drag ───────────────────────────
    let Ok(window) = windows.single() else {
        *last_cursor_pos = None;
        return;
    };

    if mouse.pressed(MouseButton::Middle) {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Some(prev) = *last_cursor_pos {
                let delta = cursor_pos - prev;
                // Screen-space delta → world-space translation.
                // Negate x so dragging right moves the view right (camera left).
                // Negate y because screen y is top-down, world y is bottom-up.
                transform.translation.x -= delta.x * config.pan_speed * ortho.scale;
                transform.translation.y += delta.y * config.pan_speed * ortho.scale;
            }
            *last_cursor_pos = Some(cursor_pos);
        }
    } else {
        *last_cursor_pos = None;
    }
}



/// Update the hover tooltip with the raw field value under the cursor.
///
/// COLD PATH: Runs every `Update` frame. Uses Bevy's `Camera` viewport
/// projection to convert cursor screen position to world coordinates,
/// then maps to a grid cell and displays the raw field value.
pub fn update_hover_tooltip(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sim: Res<SimulationState>,
    overlay: Res<ActiveOverlay>,
    sprite_q: Query<(&Transform, &Sprite), With<GridSprite>>,
    mut tooltip_q: Query<&mut Text, With<HoverTooltip>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_global)) = camera_q.single() else {
        return;
    };
    let Ok((sprite_transform, sprite)) = sprite_q.single() else {
        return;
    };

    let Some(cursor_screen) = window.cursor_position() else {
        for mut text in &mut tooltip_q {
            **text = String::new();
        }
        return;
    };

    // Use Bevy's built-in viewport_to_world_2d for accurate projection.
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_global, cursor_screen) else {
        for mut text in &mut tooltip_q {
            **text = String::new();
        }
        return;
    };

    // World position → grid cell.
    let sprite_size = sprite.custom_size.unwrap_or(Vec2::ONE);
    let sprite_origin = sprite_transform.translation.truncate() - sprite_size * 0.5;
    let local = world_pos - sprite_origin;

    let gx = local.x.floor() as i32;
    let gy = (sprite_size.y - local.y).floor() as i32; // flip y: world y-up → row-major top-down

    let width = sim.grid.width() as i32;
    let height = sim.grid.height() as i32;

    if gx < 0 || gy < 0 || gx >= width || gy >= height {
        for mut text in &mut tooltip_q {
            **text = String::new();
        }
        return;
    }

    let cell_index = (gy as usize) * (width as usize) + (gx as usize);

    let raw_value = match *overlay {
        ActiveOverlay::Heat => sim.grid.read_heat().get(cell_index).copied(),
        ActiveOverlay::Chemical(species) => sim
            .grid
            .read_chemical(species)
            .ok()
            .and_then(|buf| buf.get(cell_index).copied()),
    };

    let label = match raw_value {
        Some(v) => {
            let mut s = format!("({gx}, {gy}): {v:.4}");
            // If an actor occupies this cell, append its energy.
            if let Some(slot_index) = sim.grid.occupancy().get(cell_index).copied().flatten() {
                // Find the actor by scanning — slot_index maps to the registry slot.
                let energy = sim
                    .grid
                    .actors()
                    .iter()
                    .find(|(si, _)| *si == slot_index)
                    .map(|(_, actor)| actor.energy);
                if let Some(e) = energy {
                    s.push_str(&format!("  |  actor energy: {e:.2}"));
                }
            }
            s
        }
        None => String::new(),
    };

    for mut text in &mut tooltip_q {
        **text = label.clone();
    }
}

/// Rebuild the color scale bar image when the overlay changes.
///
/// COLD PATH: Only runs when `ActiveOverlay` changes. Regenerates the
/// gradient texture to match the current overlay's color function.
pub fn update_scale_bar(
    overlay: Res<ActiveOverlay>,
    config: Res<BevyVizConfig>,
    mut images: ResMut<Assets<Image>>,
    mut scale_q: Query<&mut ImageNode, With<ScaleBar>>,
    mut max_label_q: Query<&mut Text, With<ScaleMaxLabel>>,
) {
    if !overlay.is_changed() {
        return;
    }

    let scale_image = build_scale_image(20, 256, &overlay);
    let handle = images.add(scale_image);

    for mut image_node in &mut scale_q {
        image_node.image = handle.clone();
    }

    // Update the max label in case color_scale_max changed (future-proofing).
    for mut text in &mut max_label_q {
        **text = format!("{:.1}", config.color_scale_max);
    }
}

/// COLD PATH: Toggle info panel visibility on `I` key press.
///
/// Follows the `rate_control_input` pattern: read key state, mutate resource.
/// Requirements: 1.1, 1.3
pub fn info_panel_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<InfoPanelVisible>,
) {
    if keys.just_pressed(KeyCode::KeyI) {
        visible.0 = !visible.0;
    }
}

/// COLD PATH: Sync info panel entity visibility with the `InfoPanelVisible` resource.
///
/// Gated on `is_changed()` — only touches the entity when the resource
/// was mutated (i.e. on `I` key press). Follows `update_overlay_label` pattern.
/// Requirements: 1.1
pub fn update_info_panel(
    visible: Res<InfoPanelVisible>,
    mut query: Query<&mut Visibility, With<InfoPanel>>,
) {
    if !visible.is_changed() {
        return;
    }

    let target = if visible.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    for mut vis in &mut query {
        *vis = target;
    }
}
