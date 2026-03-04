// WARM PATH: tick_simulation runs every FixedUpdate, advancing the grid.
// COLD PATH: input, camera, label systems run every Update frame.
// Allocation forbidden in tick_simulation. Standard rules for Update systems.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use crate::grid::actor::Actor;
use crate::grid::tick::TickOrchestrator;

use super::{color, normalize};
use super::editor::EditorState;
use super::resources::{
    ActiveOverlay, ActorInspector, BevyVizConfig, GridSprite, HoverTooltip, InfoPanel,
    InfoPanelVisible, MainCamera, OverlayLabel, PredationCounter, RateLabel, RenderState,
    ScaleBar, ScaleMaxLabel, SelectedActor, SimRateController, SimulationState, SingleTraitStats,
    StatsPanel, StatsPanelVisible, StatsTickCounter, TraitStats,
};
use super::setup::{build_scale_image, format_actor_info, format_trait_stats, overlay_label_text};

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
    mut counter: ResMut<PredationCounter>,
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
        &viz_config.init_config.chemical_species_configs,
    ) {
        Ok(predation_count) => {
            sim.tick += 1;
            counter.last_tick = predation_count;
            counter.total += predation_count as u64;
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
    selected: Res<SelectedActor>,
) {
    // Resolve the field slice and color function from the active overlay.
    #[allow(clippy::type_complexity)]
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

    // Highlight the selected actor's cell in cyan.
    if let Some(slot_index) = selected.0
        && let Some((_, actor)) = sim.grid.actors().iter().find(|(si, _)| *si == slot_index)
    {
        let offset = actor.cell_index * 4;
        if offset + 3 < render.pixel_buffer.len() {
            render.pixel_buffer[offset] = 0;       // R
            render.pixel_buffer[offset + 1] = 255; // G
            render.pixel_buffer[offset + 2] = 255; // B
            render.pixel_buffer[offset + 3] = 255; // A
        }
    }

    // Upload pixel buffer into the Bevy Image asset.
    let Ok(sprite) = query.single() else {
        return;
    };

    if let Some(image) = images.get_mut(&sprite.image)
        && let Some(ref mut data) = image.data
    {
        data.copy_from_slice(&render.pixel_buffer);
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
    mut selected: ResMut<SelectedActor>,
    editor: Res<EditorState>,
) {
    // In edit mode, the editor consumes H, digit 1-9, and Escape.
    // Allow Q, I, T to pass through.
    let edit_active = editor.active;

    // Escape: deselect actor first, exit only when nothing is selected.
    // Q always exits immediately.
    if keys.just_pressed(KeyCode::Escape) && !edit_active {
        if selected.0.is_some() {
            selected.0 = None;
        } else {
            exit.write(AppExit::Success);
        }
        return;
    }

    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }

    // Editor consumes overlay-switching keys when active.
    if edit_active {
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
        if **text != label {
            **text = label.clone();
        }
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
#[allow(clippy::too_many_arguments)]
pub fn camera_controls(
    mouse_wheel: Res<AccumulatedMouseScroll>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    config: Res<BevyVizConfig>,
    mut last_cursor_pos: Local<Option<Vec2>>,
    windows: Query<&Window>,
    editor: Res<EditorState>,
) {
    let Ok((mut transform, mut projection)) = camera_q.single_mut() else {
        return;
    };

    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    // ── Zoom via mouse wheel ───────────────────────────────────────
    // Multiplicative zoom with dampening for trackpad sensitivity.
    // Clamp raw delta to ±2.0 to prevent huge jumps from fast trackpad swipes.
    // In edit mode, scroll is consumed by the editor for intensity control.
    if mouse_wheel.delta.y != 0.0 && !editor.active {
        let clamped_delta = mouse_wheel.delta.y.clamp(-2.0, 2.0);
        let zoom_factor = 1.0 + (-clamped_delta * config.zoom_speed * 0.3);
        ortho.scale = (ortho.scale * zoom_factor)
            .clamp(config.zoom_min, config.zoom_max);
    }

    // ── Pan via WASD keys ──────────────────────────────────────────
    // Speed scales with current zoom level so panning feels consistent.
    let dt = time.delta_secs();
    let pan_pixels_per_sec = 300.0 * config.pan_speed * ortho.scale;
    let mut pan_delta = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        pan_delta.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        pan_delta.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        pan_delta.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        pan_delta.x += 1.0;
    }

    if pan_delta != Vec2::ZERO {
        let movement = pan_delta.normalize() * pan_pixels_per_sec * dt;
        transform.translation.x += movement.x;
        transform.translation.y += movement.y;
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



/// Map cursor screen position to a grid cell index.
///
/// Returns `Some(cell_index)` if the cursor is within grid bounds, `None` otherwise.
/// Shared by `update_hover_tooltip` and `select_actor_input` to avoid duplication.
///
/// Requirements: 3.5
pub(super) fn cursor_to_grid_cell(
    window: &Window,
    camera: &Camera,
    cam_global: &GlobalTransform,
    sprite_transform: &Transform,
    sprite: &Sprite,
    grid_width: u32,
    grid_height: u32,
) -> Option<usize> {
    let cursor_screen = window.cursor_position()?;
    let world_pos = camera.viewport_to_world_2d(cam_global, cursor_screen).ok()?;

    let sprite_size = sprite.custom_size.unwrap_or(Vec2::ONE);
    let sprite_origin = sprite_transform.translation.truncate() - sprite_size * 0.5;
    let local = world_pos - sprite_origin;

    let gx = local.x.floor() as i32;
    let gy = (sprite_size.y - local.y).floor() as i32;

    let w = grid_width as i32;
    let h = grid_height as i32;

    if gx < 0 || gy < 0 || gx >= w || gy >= h {
        return None;
    }

    Some((gy as usize) * (grid_width as usize) + (gx as usize))
}

/// Convert cursor screen position to grid (x, y) coordinates.
///
/// Returns `None` if the cursor is outside the grid bounds.
pub(super) fn cursor_to_grid_xy(
    window: &Window,
    camera: &Camera,
    cam_global: &GlobalTransform,
    sprite_transform: &Transform,
    sprite: &Sprite,
    grid_width: u32,
    grid_height: u32,
) -> Option<(u32, u32)> {
    let cursor_screen = window.cursor_position()?;
    let world_pos = camera.viewport_to_world_2d(cam_global, cursor_screen).ok()?;

    let sprite_size = sprite.custom_size.unwrap_or(Vec2::ONE);
    let sprite_origin = sprite_transform.translation.truncate() - sprite_size * 0.5;
    let local = world_pos - sprite_origin;

    let gx = local.x.floor() as i32;
    let gy = (sprite_size.y - local.y).floor() as i32;

    let w = grid_width as i32;
    let h = grid_height as i32;

    if gx < 0 || gy < 0 || gx >= w || gy >= h {
        return None;
    }

    Some((gx as u32, gy as u32))
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

    let cell_index = cursor_to_grid_cell(
        window,
        camera,
        cam_global,
        sprite_transform,
        sprite,
        sim.grid.width(),
        sim.grid.height(),
    );

    let Some(cell_index) = cell_index else {
        for mut text in &mut tooltip_q {
            if !text.is_empty() {
                **text = String::new();
            }
        }
        return;
    };

    let gx = (cell_index % sim.grid.width() as usize) as i32;
    let gy = (cell_index / sim.grid.width() as usize) as i32;

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
        if **text != label {
            **text = label.clone();
        }
    }
}

/// Handle left-click to select an actor on the grid.
///
/// COLD PATH: Runs every `Update` frame. On left-click, maps cursor to
/// grid cell via `cursor_to_grid_cell`, looks up occupancy, and writes
/// `SelectedActor`. Clicking an empty cell clears the selection.
///
/// Requirements: 3.1, 3.2, 3.5
pub fn select_actor_input(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sim: Res<SimulationState>,
    sprite_q: Query<(&Transform, &Sprite), With<GridSprite>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut selected: ResMut<SelectedActor>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_global)) = camera_q.single() else {
        return;
    };
    let Ok((sprite_transform, sprite)) = sprite_q.single() else {
        return;
    };

    let cell_index = cursor_to_grid_cell(
        window,
        camera,
        cam_global,
        sprite_transform,
        sprite,
        sim.grid.width(),
        sim.grid.height(),
    );

    match cell_index {
        Some(ci) => {
            selected.0 = sim.grid.occupancy().get(ci).copied().flatten();
        }
        None => {
            // Click outside grid bounds — ignore (don't clear selection).
        }
    }
}

/// Clear `SelectedActor` if the referenced slot no longer holds a living actor.
///
/// COLD PATH: Runs every `Update` frame. Checks whether the selected slot
/// index still maps to a non-inert actor in the registry. If the actor has
/// died, been removed, or gone inert, the selection is cleared.
///
/// Requirements: 3.4
pub fn clear_stale_selection(
    sim: Res<SimulationState>,
    mut selected: ResMut<SelectedActor>,
) {
    let Some(slot_index) = selected.0 else {
        return;
    };

    // Check if the slot still holds a living (non-inert) actor.
    let is_alive = sim
        .grid
        .actors()
        .iter()
        .find(|(si, _)| *si == slot_index)
        .is_some_and(|(_, actor)| !actor.inert);

    if !is_alive {
        selected.0 = None;
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

/// COLD PATH: Toggle stats panel visibility on `T` key press.
///
/// Follows the `info_panel_input` pattern: read key state, mutate resource.
/// Requirements: 2.1
pub fn stats_panel_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<StatsPanelVisible>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        visible.0 = !visible.0;
    }
}

/// COLD PATH: Sync stats panel text and visibility with `TraitStats` and
/// `StatsPanelVisible` resources.
///
/// Gated on `is_changed()` — only touches the entity when either resource
/// was mutated. Follows `update_info_panel` / `update_overlay_label` pattern.
///
/// Requirements: 2.1, 2.4, 2.5, 2.6
pub fn update_stats_panel(
    stats: Res<TraitStats>,
    predation: Res<PredationCounter>,
    visible: Res<StatsPanelVisible>,
    config: Res<BevyVizConfig>,
    mut query: Query<(&mut Text, &mut Visibility), With<StatsPanel>>,
) {
    if !stats.is_changed() && !visible.is_changed() && !predation.is_changed() {
        return;
    }

    let target_vis = if visible.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    let content = format_trait_stats(&stats, &predation, config.actor_config.as_ref());

    for (mut text, mut vis) in &mut query {
        if **text != content {
            **text = content.clone();
        }
        if *vis != target_vis {
            *vis = target_vis;
        }
    }
}

/// COLD PATH: Sync actor inspector panel text and visibility with
/// `SelectedActor` and `SimulationState`.
///
/// Hidden when no actor is selected. When `Some(slot_index)`, looks up
/// the actor in the registry and formats its full state. Gated on
/// `is_changed()` for both resources.
///
/// Requirements: 4.3, 4.4, 4.5, 4.6
pub fn update_actor_inspector(
    selected: Res<SelectedActor>,
    sim: Res<SimulationState>,
    mut query: Query<(&mut Text, &mut Visibility), With<ActorInspector>>,
) {
    if !selected.is_changed() && !sim.is_changed() {
        return;
    }

    let Some(slot_index) = selected.0 else {
        for (mut text, mut vis) in &mut query {
            if !text.is_empty() {
                **text = String::new();
            }
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    };

    // Look up the actor by slot index.
    let actor = sim
        .grid
        .actors()
        .iter()
        .find(|(si, _)| *si == slot_index)
        .map(|(_, a)| a);

    let Some(actor) = actor else {
        for (mut text, mut vis) in &mut query {
            if !text.is_empty() {
                **text = String::new();
            }
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    };

    let content = format_actor_info(actor, slot_index, sim.grid.width());

    for (mut text, mut vis) in &mut query {
        if **text != content {
            **text = content.clone();
        }
        if *vis != Visibility::Visible {
            *vis = Visibility::Visible;
        }
    }
}



// ── Trait Stats Computation ────────────────────────────────────────

/// Compute population statistics from an iterator of actors.
///
/// COLD PATH: Allocates four `Vec<f32>` buffers, sorts each, and derives
/// min/max/mean/percentiles via nearest-rank. Acceptable for per-tick
/// visualization work.
///
/// - Non-inert actors only. Inert actors are excluded.
/// - Zero living actors → `TraitStats { actor_count: 0, traits: None }`.
/// - One living actor → all stats equal to that actor's trait values.
///
/// Pure function — no Bevy dependencies, testable in isolation.
///
/// Requirements: 1.1, 1.3, 1.4, 1.5
pub fn compute_trait_stats_from_actors<'a>(
    actors: impl Iterator<Item = &'a Actor>,
    tick: u64,
) -> TraitStats {
    // Use iterator size hint to pre-allocate all 8 trait buffers, avoiding
    // incremental reallocation during collection. The lower bound is a
    // conservative estimate; the upper bound (if available) would be tighter,
    // but lower_bound alone eliminates most reallocations in practice.
    let (lower_bound, _) = actors.size_hint();
    let capacity = lower_bound;

    let mut consumption = Vec::with_capacity(capacity);
    let mut decay = Vec::with_capacity(capacity);
    let mut levy = Vec::with_capacity(capacity);
    let mut repro = Vec::with_capacity(capacity);
    let mut tumble = Vec::with_capacity(capacity);
    let mut repro_cost = Vec::with_capacity(capacity);
    let mut offspring = Vec::with_capacity(capacity);
    let mut mutation_rate = Vec::with_capacity(capacity);
    let mut kin_tolerance = Vec::with_capacity(capacity);
    let mut optimal_temp = Vec::with_capacity(capacity);
    let mut repro_cooldown = Vec::with_capacity(capacity);
    let mut kin_group_defense = Vec::with_capacity(capacity);
    let mut memory_capacity = Vec::with_capacity(capacity);
    let mut site_fidelity = Vec::with_capacity(capacity);
    let mut avoid_sensitivity = Vec::with_capacity(capacity);
    let mut energy = Vec::with_capacity(capacity);

    // Single-pass collection: iterate actors once, push all 11 trait values
    // per non-inert actor. This is explicit rather than relying on the
    // optimizer to fuse separate collection passes.
    for actor in actors {
        if actor.inert {
            continue;
        }
        consumption.push(actor.traits.consumption_rate);
        decay.push(actor.traits.base_energy_decay);
        levy.push(actor.traits.levy_exponent);
        repro.push(actor.traits.reproduction_threshold);
        tumble.push(actor.traits.max_tumble_steps as f32);
        repro_cost.push(actor.traits.reproduction_cost);
        offspring.push(actor.traits.offspring_energy);
        mutation_rate.push(actor.traits.mutation_rate);
        kin_tolerance.push(actor.traits.kin_tolerance);
        optimal_temp.push(actor.traits.optimal_temp);
        repro_cooldown.push(actor.traits.reproduction_cooldown as f32);
        kin_group_defense.push(actor.traits.kin_group_defense);
        memory_capacity.push(actor.traits.memory_capacity as f32);
        site_fidelity.push(actor.traits.site_fidelity_strength);
        avoid_sensitivity.push(actor.traits.avoidance_sensitivity);
        energy.push(actor.energy);
    }

    let actor_count = consumption.len();

    if actor_count == 0 {
        return TraitStats {
            actor_count: 0,
            tick,
            traits: None,
            energy_stats: None,
        };
    }

    let traits = [
        compute_single_stats(&mut consumption),
        compute_single_stats(&mut decay),
        compute_single_stats(&mut levy),
        compute_single_stats(&mut repro),
        compute_single_stats(&mut tumble),
        compute_single_stats(&mut repro_cost),
        compute_single_stats(&mut offspring),
        compute_single_stats(&mut mutation_rate),
        compute_single_stats(&mut kin_tolerance),
        compute_single_stats(&mut optimal_temp),
        compute_single_stats(&mut repro_cooldown),
        compute_single_stats(&mut kin_group_defense),
        compute_single_stats(&mut memory_capacity),
        compute_single_stats(&mut site_fidelity),
        compute_single_stats(&mut avoid_sensitivity),
    ];

    let energy_stats = Some(compute_single_stats(&mut energy));

    TraitStats {
        actor_count,
        tick,
        traits: Some(traits),
        energy_stats,
    }
}

/// Compute min/max/mean/percentiles for a single trait buffer.
///
/// Sorts the buffer in-place using `total_cmp` (NaN-safe).
/// Percentiles use nearest-rank (floor index).
///
/// Precondition: `values` is non-empty.
fn compute_single_stats(values: &mut [f32]) -> SingleTraitStats {
    let n = values.len();

    // Streaming pass: min, max, sum in O(n). No sort needed.
    let mut min = values[0];
    let mut max = values[0];
    let mut sum = values[0];
    for &v in &values[1..] {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        sum += v;
    }
    let mean = sum / n as f32;

    // Percentile indices — same formula as the original implementation.
    let p50_idx = (n - 1) * 50 / 100;
    let p25_idx = (n - 1) * 25 / 100;
    let p75_idx = (n - 1) * 75 / 100;

    // O(n) selection: compute p50 first, which partitions the slice such that
    // values[..p50_idx] <= values[p50_idx] <= values[p50_idx+1..].
    values.select_nth_unstable_by(p50_idx, f32::total_cmp);
    let p50 = values[p50_idx];

    // p25 on the left partition (indices 0..p50_idx). When p25_idx == p50_idx
    // (degenerate case, n < 4), the left partition is empty and we already
    // have the value at p50_idx.
    let p25 = if p25_idx < p50_idx {
        values[..p50_idx].select_nth_unstable_by(p25_idx, f32::total_cmp);
        values[p25_idx]
    } else {
        values[p25_idx]
    };

    // p75 on the right partition (indices p50_idx+1..). When p75_idx == p50_idx
    // (degenerate case), we already have the value.
    let p75 = if p75_idx > p50_idx {
        let right = &mut values[p50_idx + 1..];
        let local_idx = p75_idx - (p50_idx + 1);
        right.select_nth_unstable_by(local_idx, f32::total_cmp);
        right[local_idx]
    } else {
        values[p75_idx]
    };

    // Second pass: population variance. Acceptable on COLD path.
    let variance = values.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / n as f32;
    let std_dev = variance.sqrt();

    SingleTraitStats {
        min,
        max,
        mean,
        p25,
        p50,
        p75,
        std_dev,
    }
}

/// Bevy system: recompute `TraitStats` from the current simulation state.
///
/// COLD PATH: Runs in `FixedUpdate` after `tick_simulation`. Reads the
/// actor registry from `SimulationState`, writes the `TraitStats` resource.
/// Throttled by `StatsTickCounter` — skips recomputation when fewer than
/// `interval` ticks have elapsed since the last update.
///
/// Requirements: 1.1, 1.3, 1.4, 1.5, 2.2, 2.3, 2.6
pub fn compute_trait_stats(
    sim: Res<SimulationState>,
    mut stats: ResMut<TraitStats>,
    mut counter: ResMut<StatsTickCounter>,
) {
    // Throttle gate: skip recomputation when interval > 1 and not enough
    // ticks have elapsed. interval 0 or 1 means every-tick (no throttling).
    if counter.interval > 1 {
        counter.ticks_since_update += 1;
        if counter.ticks_since_update < counter.interval {
            return;
        }
        counter.ticks_since_update = 0;
    }

    let actors = sim.grid.actors().iter().map(|(_, actor)| actor);
    *stats = compute_trait_stats_from_actors(actors, sim.tick);
}
