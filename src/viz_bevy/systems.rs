// WARM PATH: tick_simulation runs every FixedUpdate, advancing the grid.
// COLD PATH: input, camera, label systems run every Update frame.
// Allocation forbidden in tick_simulation. Standard rules for Update systems.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use crate::grid::tick::TickOrchestrator;

use super::{color, normalize};
use super::resources::{
    ActiveOverlay, BevyVizConfig, GridSprite, MainCamera, OverlayLabel, RenderState,
    SimulationState,
};
use super::setup::overlay_label_text;

/// Advance the simulation by one tick.
///
/// Runs in `FixedUpdate`. Skips when `running == false` (halted due to
/// a prior tick error). On error, logs via `tracing::error!` and sets
/// `running = false` so subsequent invocations become no-ops.
///
/// Requirements: 2.2 (tick advancement), 2.4 (fixed timestep decoupling),
/// 2.5 (error halts tick).
pub fn tick_simulation(mut sim: ResMut<SimulationState>) {
    if !sim.running {
        return;
    }

    let sim = &mut *sim;
    match TickOrchestrator::step(&mut sim.grid, &sim.config) {
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
    normalize::normalize_field(field, &mut render.norm_buffer);

    // Color-map normalized values into the pre-allocated pixel buffer.
    color::fill_pixel_buffer(&render.norm_buffer, &mut render.pixel_buffer, color_fn);

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
