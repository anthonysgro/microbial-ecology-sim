// COLD PATH: Editor systems run only on user input events, never per-tick.
// Allocation permitted. Dynamic dispatch permitted.
//
// Modal world editor for the Bevy visualization layer. Provides brush-based
// painting of grid fields, actor placement, eraser, and pattern stamping.
// All mutation functions are pure (no Bevy dependency) to enable property-based
// testing without a Bevy App harness.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use smallvec::SmallVec;

use crate::grid::Grid;
use crate::grid::actor::{Actor, ActorError, HeritableTraits};
use crate::io::snapshot::{
    self, ActorSnapshot, BrainSnapshot, Pattern, PatternActor, SnapshotError,
};

use super::resources::{
    BevyVizConfig, GridSprite, MainCamera, SimRateController, SimulationState,
};
use super::systems::{cursor_to_grid_cell, cursor_to_grid_xy};

// ── Types ──────────────────────────────────────────────────────────

/// Brush type selection for the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushType {
    Heat,
    Chemical(usize),
    Actor,
    Eraser,
}

/// Modal editor state. Inserted as a Bevy Resource.
/// COLD: mutated only on user key/mouse events.
#[derive(Resource)]
pub struct EditorState {
    /// Whether the editor is active (Edit_Mode).
    pub active: bool,
    /// Current brush type selection.
    pub brush_type: BrushType,
    /// Brush intensity (value written to cells).
    pub intensity: f32,
    /// Brush radius in cells. 0 = single cell.
    pub radius: u32,
    /// Whether simulation was running before entering edit mode.
    pub was_running: bool,
    /// Whether simulation was paused before entering edit mode.
    pub was_paused: bool,
    /// Active rectangular selection (start_cell, end_cell) for pattern ops.
    pub selection: Option<(usize, usize)>,
    /// Loaded pattern awaiting stamp placement.
    pub pending_pattern: Option<Pattern>,
    /// Transient HUD message with expiry frame count.
    pub hud_message: Option<(String, u64)>,
    /// Frame counter for HUD message expiry.
    pub frame_count: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active: false,
            brush_type: BrushType::Heat,
            intensity: 1.0,
            radius: 0,
            was_running: false,
            was_paused: false,
            selection: None,
            pending_pattern: None,
            hud_message: None,
            frame_count: 0,
        }
    }
}

// ── Marker Components ──────────────────────────────────────────────

/// Marker for the Editor HUD text entity.
#[derive(Component)]
pub struct EditorHud;

/// Marker for the stamp preview overlay entity.
#[derive(Component)]
pub struct StampPreview;

// ── Pure Functions ─────────────────────────────────────────────────

/// Result of toggling edit mode on or off.
pub struct ToggleResult {
    pub active: bool,
    pub running: bool,
    pub paused: bool,
    pub was_running: bool,
    pub was_paused: bool,
}

/// Toggle edit mode. Pure function — no Bevy dependency.
///
/// On activate: stores current running/paused, sets running=false, paused=true.
/// On deactivate: restores was_running/was_paused.
pub fn toggle_edit_mode(
    currently_active: bool,
    running: bool,
    paused: bool,
    was_running: bool,
    was_paused: bool,
) -> ToggleResult {
    if currently_active {
        // Deactivate: restore previous simulation state.
        ToggleResult {
            active: false,
            running: was_running,
            paused: was_paused,
            was_running,
            was_paused,
        }
    } else {
        // Activate: suspend simulation, store current state.
        ToggleResult {
            active: true,
            running: false,
            paused: true,
            was_running: running,
            was_paused: paused,
        }
    }
}

/// Map a key code to a BrushType. Pure function.
///
/// H → Heat, digit d (1-9) where d-1 < num_chemicals → Chemical(d-1),
/// P → Actor, X → Eraser. Returns None for invalid/unrecognized keys.
pub fn select_brush_type(key: BrushKey, num_chemicals: usize) -> Option<BrushType> {
    match key {
        BrushKey::H => Some(BrushType::Heat),
        BrushKey::P => Some(BrushType::Actor),
        BrushKey::X => Some(BrushType::Eraser),
        BrushKey::Digit(d) => {
            let species = d as usize - 1;
            if species < num_chemicals {
                Some(BrushType::Chemical(species))
            } else {
                None
            }
        }
    }
}

/// Abstraction over key codes for testability without Bevy KeyCode dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushKey {
    H,
    P,
    X,
    /// Digit 1-9.
    Digit(u8),
}

/// Scroll direction for intensity adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// Adjust brush intensity. Pure function.
///
/// Step = 0.1 without Shift, 1.0 with Shift. Clamps to [0.0, 100.0].
pub fn adjust_intensity(current: f32, direction: ScrollDirection, shift_held: bool) -> f32 {
    let step = if shift_held { 1.0 } else { 0.1 };
    let delta = match direction {
        ScrollDirection::Up => step,
        ScrollDirection::Down => -step,
    };
    (current + delta).clamp(0.0, 100.0)
}

/// Direction for radius adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusDirection {
    Increase,
    Decrease,
}

/// Adjust brush radius. Pure function. Clamps to [0, 20].
pub fn adjust_radius(current: u32, direction: RadiusDirection) -> u32 {
    match direction {
        RadiusDirection::Increase => current.saturating_add(1).min(20),
        RadiusDirection::Decrease => current.saturating_sub(1),
    }
}

/// Compute the set of cell indices affected by a square brush.
///
/// Returns all cells within a square of side `(2*radius+1)` centered on
/// `(cx, cy)`, clipped to grid bounds. Row-major index = y * width + x.
pub fn compute_brush_cells(
    grid_width: u32,
    grid_height: u32,
    cx: u32,
    cy: u32,
    radius: u32,
) -> SmallVec<[usize; 64]> {
    let mut cells = SmallVec::new();

    let x_min = cx.saturating_sub(radius);
    let y_min = cy.saturating_sub(radius);
    let x_max = (cx + radius).min(grid_width - 1);
    let y_max = (cy + radius).min(grid_height - 1);

    for y in y_min..=y_max {
        for x in x_min..=x_max {
            cells.push((y as usize) * (grid_width as usize) + (x as usize));
        }
    }

    cells
}

/// Apply a heat or chemical brush to a single cell, writing to both buffers.
///
/// For Heat: writes intensity to both read and write buffers at cell_index.
/// For Chemical(species): writes intensity to both buffers of that species.
/// Actor and Eraser brush types are no-ops here (handled separately).
pub fn apply_brush_to_cell(
    grid: &mut Grid,
    cell_index: usize,
    brush_type: BrushType,
    intensity: f32,
) {
    match brush_type {
        BrushType::Heat => {
            grid.heat_buffer_mut().write_both(cell_index, intensity);
        }
        BrushType::Chemical(species) => {
            if let Some(buf) = grid.chemical_buffer_mut(species) {
                buf.write_both(cell_index, intensity);
            }
        }
        BrushType::Actor | BrushType::Eraser => {}
    }
}

/// Erase all field values and remove any actor at a cell.
///
/// Sets heat to 0.0 in both buffers, all chemical concentrations to 0.0
/// in both buffers. If the cell is occupied, removes the actor via
/// `Grid::remove_actor`.
///
/// Returns `Ok(())` on success, or an `ActorError` if actor removal fails
/// (should not happen in practice if occupancy is consistent).
pub fn apply_eraser_to_cell(grid: &mut Grid, cell_index: usize) -> Result<(), ActorError> {
    // Zero heat in both buffers.
    grid.heat_buffer_mut().write_both(cell_index, 0.0);

    // Zero all chemical species in both buffers.
    let num_chem = grid.num_chemicals();
    for species in 0..num_chem {
        if let Some(buf) = grid.chemical_buffer_mut(species) {
            buf.write_both(cell_index, 0.0);
        }
    }

    // Remove actor if cell is occupied.
    if let Some(slot_index) = grid.occupancy()[cell_index]
        && let Some(actor_id) = grid.actors().actor_id_for_slot(slot_index)
    {
        grid.remove_actor(actor_id)?;
    }

    Ok(())
}

/// Apply a pattern to the grid at the given top-left position.
///
/// Writes heat and chemical values to both buffers for in-bounds cells.
/// Spawns actors at absolute positions, skipping occupied cells.
/// Clips the pattern to the grid boundary.
///
/// Returns the number of actors successfully placed.
pub fn apply_pattern_to_grid(
    pattern: &Pattern,
    top_left_x: u32,
    top_left_y: u32,
    grid: &mut Grid,
) -> usize {
    let grid_w = grid.width();
    let grid_h = grid.height();
    let pat_w = pattern.width;
    let pat_h = pattern.height;

    // Determine the clipped region within the grid.
    let x_end = (top_left_x + pat_w).min(grid_w);
    let y_end = (top_left_y + pat_h).min(grid_h);

    // Write field values.
    for gy in top_left_y..y_end {
        for gx in top_left_x..x_end {
            let px = (gx - top_left_x) as usize;
            let py = (gy - top_left_y) as usize;
            let pat_idx = py * (pat_w as usize) + px;
            let grid_idx = (gy as usize) * (grid_w as usize) + (gx as usize);

            // Heat
            if let Some(&heat_val) = pattern.heat.get(pat_idx) {
                grid.heat_buffer_mut().write_both(grid_idx, heat_val);
            }

            // Chemicals
            let num_chem = pattern.num_chemicals.min(grid.num_chemicals());
            for species in 0..num_chem {
                if let Some(chem_buf) = pattern.chemicals.get(species)
                    && let Some(&chem_val) = chem_buf.get(pat_idx)
                    && let Some(grid_buf) = grid.chemical_buffer_mut(species)
                {
                    grid_buf.write_both(grid_idx, chem_val);
                }
            }
        }
    }

    // Spawn actors at absolute positions, skipping occupied cells.
    let mut placed = 0;
    for pat_actor in &pattern.actors {
        let abs_x = top_left_x + pat_actor.rel_x;
        let abs_y = top_left_y + pat_actor.rel_y;

        // Skip out-of-bounds actors.
        if abs_x >= grid_w || abs_y >= grid_h {
            continue;
        }

        let cell_index = (abs_y as usize) * (grid_w as usize) + (abs_x as usize);

        // Skip occupied cells.
        if grid.occupancy()[cell_index].is_some() {
            continue;
        }

        let actor = Actor {
            cell_index,
            energy: pat_actor.energy,
            inert: false,
            tumble_direction: 0,
            tumble_remaining: 0,
            traits: pat_actor.traits,
            cooldown_remaining: 0,
        };

        if grid.add_actor(actor).is_ok() {
            placed += 1;
        }
    }

    placed
}

// ── Bevy Systems ───────────────────────────────────────────────────

/// HUD message duration in frames (~3 seconds at 60fps).
const HUD_MESSAGE_FRAMES: u64 = 180;

/// Handle editor keyboard input: mode toggle, brush selection, intensity,
/// radius, snapshot save/load, pattern copy/paste, selection, and escape.
///
/// COLD PATH: Runs every `Update` frame but only mutates state on key events.
#[allow(clippy::too_many_arguments)]
pub fn editor_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_wheel: Res<AccumulatedMouseScroll>,
    mut editor: ResMut<EditorState>,
    mut sim: ResMut<SimulationState>,
    mut rate: ResMut<SimRateController>,
    viz_config: Res<BevyVizConfig>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sprite_q: Query<(&Transform, &Sprite), With<GridSprite>>,
) {
    editor.frame_count += 1;

    // ── E key: toggle edit mode ────────────────────────────────────
    if keys.just_pressed(KeyCode::KeyE) {
        let result = toggle_edit_mode(
            editor.active,
            sim.running,
            rate.paused,
            editor.was_running,
            editor.was_paused,
        );
        editor.active = result.active;
        editor.was_running = result.was_running;
        editor.was_paused = result.was_paused;
        sim.running = result.running;
        rate.paused = result.paused;

        if editor.active {
            editor.selection = None;
            editor.pending_pattern = None;
        }
        return;
    }

    if !editor.active {
        return;
    }

    let num_chemicals = sim.config.num_chemicals;

    // ── Brush type selection ───────────────────────────────────────
    let brush_key = if keys.just_pressed(KeyCode::KeyH) {
        Some(BrushKey::H)
    } else if keys.just_pressed(KeyCode::KeyP) {
        Some(BrushKey::P)
    } else if keys.just_pressed(KeyCode::KeyX) {
        Some(BrushKey::X)
    } else {
        let digit_keys = [
            (KeyCode::Digit1, 1u8),
            (KeyCode::Digit2, 2),
            (KeyCode::Digit3, 3),
            (KeyCode::Digit4, 4),
            (KeyCode::Digit5, 5),
            (KeyCode::Digit6, 6),
            (KeyCode::Digit7, 7),
            (KeyCode::Digit8, 8),
            (KeyCode::Digit9, 9),
        ];
        digit_keys
            .iter()
            .find(|(kc, _)| keys.just_pressed(*kc))
            .map(|(_, d)| BrushKey::Digit(*d))
    };

    if let Some(key) = brush_key
        && let Some(bt) = select_brush_type(key, num_chemicals)
    {
        editor.brush_type = bt;
        // Exit stamp mode when switching brush type.
        editor.pending_pattern = None;
    }

    // ── Intensity adjustment via scroll wheel ──────────────────────
    if mouse_wheel.delta.y != 0.0 {
        let direction = if mouse_wheel.delta.y > 0.0 {
            ScrollDirection::Up
        } else {
            ScrollDirection::Down
        };
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        editor.intensity = adjust_intensity(editor.intensity, direction, shift);
    }

    // ── Radius adjustment via bracket keys ─────────────────────────
    if keys.just_pressed(KeyCode::BracketLeft) {
        editor.radius = adjust_radius(editor.radius, RadiusDirection::Decrease);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        editor.radius = adjust_radius(editor.radius, RadiusDirection::Increase);
    }

    // ── Escape: exit stamp mode or clear selection ─────────────────
    if keys.just_pressed(KeyCode::Escape) {
        if editor.pending_pattern.is_some() {
            editor.pending_pattern = None;
        } else {
            editor.selection = None;
        }
        return;
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // ── Ctrl+S: save snapshot ──────────────────────────────────────
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        let frame = editor.frame_count;
        match save_snapshot(&sim, &viz_config) {
            Ok(path) => {
                editor.hud_message = Some((
                    format!("Snapshot saved: {path}"),
                    frame + HUD_MESSAGE_FRAMES,
                ));
            }
            Err(e) => {
                editor.hud_message = Some((
                    format!("Save failed: {e}"),
                    frame + HUD_MESSAGE_FRAMES,
                ));
            }
        }
        return;
    }

    // ── Ctrl+L: load snapshot ──────────────────────────────────────
    if ctrl && keys.just_pressed(KeyCode::KeyL) {
        let frame = editor.frame_count;
        match load_snapshot(&mut sim) {
            Ok(msg) => {
                editor.hud_message = Some((msg, frame + HUD_MESSAGE_FRAMES));
            }
            Err(e) => {
                editor.hud_message = Some((
                    format!("Load failed: {e}"),
                    frame + HUD_MESSAGE_FRAMES,
                ));
            }
        }
        return;
    }

    // ── Ctrl+C: copy selection to pattern ──────────────────────────
    if ctrl && keys.just_pressed(KeyCode::KeyC) {
        let frame = editor.frame_count;
        if let Some((start, end)) = editor.selection {
            match copy_selection_to_pattern(&sim.grid, start, end) {
                Ok(path) => {
                    editor.hud_message = Some((
                        format!("Pattern saved: {path}"),
                        frame + HUD_MESSAGE_FRAMES,
                    ));
                }
                Err(e) => {
                    editor.hud_message = Some((
                        format!("Copy failed: {e}"),
                        frame + HUD_MESSAGE_FRAMES,
                    ));
                }
            }
        } else {
            editor.hud_message = Some((
                "No selection — Shift+drag to select".to_string(),
                frame + HUD_MESSAGE_FRAMES,
            ));
        }
        return;
    }

    // ── Ctrl+V: load pattern for stamping ──────────────────────────
    if ctrl && keys.just_pressed(KeyCode::KeyV) {
        let frame = editor.frame_count;
        match load_pattern_for_stamp(num_chemicals) {
            Ok(pattern) => {
                editor.pending_pattern = Some(pattern);
                editor.hud_message = Some((
                    "Pattern loaded — click to stamp, Esc to cancel".to_string(),
                    frame + HUD_MESSAGE_FRAMES,
                ));
            }
            Err(e) => {
                editor.hud_message = Some((
                    format!("Paste failed: {e}"),
                    frame + HUD_MESSAGE_FRAMES,
                ));
            }
        }
        return;
    }

    // ── Shift+left-click-drag: rectangular selection ───────────────
    if shift && mouse.just_pressed(MouseButton::Left) {
        let Ok(window) = windows.single() else { return };
        let Ok((camera, cam_global)) = camera_q.single() else { return };
        let Ok((sprite_tf, sprite)) = sprite_q.single() else { return };

        if let Some(cell) = cursor_to_grid_cell(
            window, camera, cam_global, sprite_tf, sprite,
            sim.grid.width(), sim.grid.height(),
        ) {
            // Start selection: both corners at the same cell.
            editor.selection = Some((cell, cell));
        }
    } else if shift && mouse.pressed(MouseButton::Left) {
        // Extend selection to current cursor position.
        if let Some((start, _)) = editor.selection {
            let Ok(window) = windows.single() else { return };
            let Ok((camera, cam_global)) = camera_q.single() else { return };
            let Ok((sprite_tf, sprite)) = sprite_q.single() else { return };

            if let Some(cell) = cursor_to_grid_cell(
                window, camera, cam_global, sprite_tf, sprite,
                sim.grid.width(), sim.grid.height(),
            ) {
                editor.selection = Some((start, cell));
            }
        }
    }
}

// ── Snapshot / Pattern I/O helpers ─────────────────────────────────

/// Save the current world state to `snapshots/tick_{N}.snap`.
fn save_snapshot(
    sim: &SimulationState,
    viz_config: &BevyVizConfig,
) -> Result<String, SnapshotError> {
    let grid = &sim.grid;
    let heat_read = grid.read_heat();

    let mut chem_reads: Vec<&[f32]> = Vec::with_capacity(sim.config.num_chemicals);
    for species in 0..sim.config.num_chemicals {
        match grid.read_chemical(species) {
            Ok(buf) => chem_reads.push(buf),
            Err(_) => break,
        }
    }

    let sources: Vec<_> = grid.sources().iter().cloned().collect();

    let brains = grid.brains();
    let actors: Vec<ActorSnapshot> = grid
        .actors()
        .iter()
        .map(|(slot_idx, actor)| {
            let brain = &brains[slot_idx];
            let entries: Vec<_> = brain.entries[..brain.len as usize].to_vec();
            ActorSnapshot {
                cell_index: actor.cell_index,
                energy: actor.energy,
                inert: actor.inert,
                tumble_direction: actor.tumble_direction,
                tumble_remaining: actor.tumble_remaining,
                traits: actor.traits,
                cooldown_remaining: actor.cooldown_remaining,
                brain: BrainSnapshot {
                    entries,
                    head: brain.head,
                    len: brain.len,
                },
            }
        })
        .collect();

    let bytes = snapshot::serialize_snapshot(
        &sim.config,
        viz_config.actor_config.as_ref(),
        sim.tick,
        heat_read,
        &chem_reads,
        &sources,
        &actors,
    )?;

    std::fs::create_dir_all("snapshots")?;
    let filename = format!("snapshots/tick_{}.snap", sim.tick);
    std::fs::write(&filename, &bytes)?;
    Ok(filename)
}

/// Load the most recent snapshot from `snapshots/` directory.
///
/// Reads the newest `.snap` file by name (lexicographic sort).
/// Validates grid dimensions match the current grid before restoring.
fn load_snapshot(sim: &mut SimulationState) -> Result<String, SnapshotError> {
    let dir = std::fs::read_dir("snapshots").map_err(SnapshotError::Io)?;

    let mut snap_files: Vec<_> = dir
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "snap")
        })
        .collect();

    if snap_files.is_empty() {
        return Err(SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no .snap files in snapshots/ directory",
        )));
    }

    snap_files.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

    let path = snap_files[0].path();
    let bytes = std::fs::read(&path)?;
    let data = snapshot::deserialize_snapshot(&bytes)?;

    // Validate dimensions match.
    if data.grid_config.width != sim.config.width
        || data.grid_config.height != sim.config.height
    {
        return Err(SnapshotError::DimensionMismatch {
            snap_w: data.grid_config.width,
            snap_h: data.grid_config.height,
            grid_w: sim.config.width,
            grid_h: sim.config.height,
        });
    }

    // Restore heat buffer.
    let grid = &mut sim.grid;
    for (i, &val) in data.heat.iter().enumerate() {
        grid.heat_buffer_mut().write_both(i, val);
    }

    // Restore chemical buffers.
    let num_chem = data.chemicals.len().min(grid.num_chemicals());
    for species in 0..num_chem {
        if let Some(buf) = grid.chemical_buffer_mut(species) {
            for (i, &val) in data.chemicals[species].iter().enumerate() {
                buf.write_both(i, val);
            }
        }
    }

    // Clear existing actors.
    let actor_ids: Vec<_> = grid
        .actors()
        .iter()
        .filter_map(|(slot_idx, _)| grid.actors().actor_id_for_slot(slot_idx))
        .collect();
    for id in actor_ids {
        let _ = grid.remove_actor(id);
    }

    // Restore actors from snapshot.
    for snap_actor in &data.actors {
        let actor = Actor {
            cell_index: snap_actor.cell_index,
            energy: snap_actor.energy,
            inert: snap_actor.inert,
            tumble_direction: snap_actor.tumble_direction,
            tumble_remaining: snap_actor.tumble_remaining,
            traits: snap_actor.traits,
            cooldown_remaining: snap_actor.cooldown_remaining,
        };
        if let Ok(actor_id) = grid.add_actor(actor) {
            // Restore brain state.
            let slot_idx = actor_id.index;
            let brain = &mut grid.brains_mut()[slot_idx];
            brain.head = snap_actor.brain.head;
            brain.len = snap_actor.brain.len;
            for (i, entry) in snap_actor.brain.entries.iter().enumerate() {
                if i < brain.entries.len() {
                    brain.entries[i] = *entry;
                }
            }
        }
    }

    sim.tick = data.tick;

    let display_path = path.display().to_string();
    Ok(format!("Loaded: {display_path} (tick {})", data.tick))
}

/// Copy a rectangular selection from the grid to a pattern file.
///
/// `start` and `end` are cell indices defining opposite corners of the
/// selection rectangle. The function normalizes them to (top_left, bottom_right).
fn copy_selection_to_pattern(
    grid: &Grid,
    start: usize,
    end: usize,
) -> Result<String, SnapshotError> {
    let w = grid.width() as usize;

    let (sx, sy) = (start % w, start / w);
    let (ex, ey) = (end % w, end / w);

    let x_min = sx.min(ex) as u32;
    let y_min = sy.min(ey) as u32;
    let x_max = sx.max(ex) as u32;
    let y_max = sy.max(ey) as u32;

    let pat_w = x_max - x_min + 1;
    let pat_h = y_max - y_min + 1;
    let pat_cells = (pat_w as usize) * (pat_h as usize);
    let num_chem = grid.num_chemicals();

    let mut heat = Vec::with_capacity(pat_cells);
    let mut chemicals: Vec<Vec<f32>> = (0..num_chem)
        .map(|_| Vec::with_capacity(pat_cells))
        .collect();
    let mut actors = Vec::new();

    let heat_read = grid.read_heat();

    for gy in y_min..=y_max {
        for gx in x_min..=x_max {
            let grid_idx = (gy as usize) * w + (gx as usize);
            heat.push(heat_read[grid_idx]);

            for (species, chem_vec) in chemicals.iter_mut().enumerate().take(num_chem) {
                if let Ok(chem_buf) = grid.read_chemical(species) {
                    chem_vec.push(chem_buf[grid_idx]);
                }
            }

            if let Some(slot_idx) = grid.occupancy()[grid_idx]
                && let Some(actor) = grid.actors().get_by_slot(slot_idx)
            {
                actors.push(PatternActor {
                    rel_x: gx - x_min,
                    rel_y: gy - y_min,
                    energy: actor.energy,
                    traits: actor.traits,
                });
            }
        }
    }

    let pattern = Pattern {
        width: pat_w,
        height: pat_h,
        num_chemicals: num_chem,
        heat,
        chemicals,
        actors,
    };

    let bytes = snapshot::serialize_pattern(&pattern)?;

    std::fs::create_dir_all("patterns")?;
    let filename = format!(
        "patterns/region_{}x{}_at_{},{}.pat",
        pat_w, pat_h, x_min, y_min
    );
    std::fs::write(&filename, &bytes)?;
    Ok(filename)
}

/// Load the most recent pattern file from `patterns/` directory.
///
/// Validates that the pattern's chemical species count does not exceed
/// the grid's `num_chemicals`.
fn load_pattern_for_stamp(num_chemicals: usize) -> Result<Pattern, SnapshotError> {
    let dir = std::fs::read_dir("patterns").map_err(SnapshotError::Io)?;

    let mut pat_files: Vec<_> = dir
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "pat")
        })
        .collect();

    if pat_files.is_empty() {
        return Err(SnapshotError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no .pat files in patterns/ directory",
        )));
    }

    // Sort descending by filename to get the most recent.
    pat_files.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

    let path = pat_files[0].path();
    let bytes = std::fs::read(&path)?;
    let pattern = snapshot::deserialize_pattern(&bytes)?;

    if pattern.num_chemicals > num_chemicals {
        return Err(SnapshotError::ChemicalMismatch {
            pattern_count: pattern.num_chemicals,
            grid_count: num_chemicals,
        });
    }

    Ok(pattern)
}

// ── Paint System ───────────────────────────────────────────────────

/// Apply brush strokes on left-click or left-drag in edit mode.
///
/// COLD PATH: Runs every `Update` frame but only mutates grid on mouse input.
pub fn editor_paint(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
    mut sim: ResMut<SimulationState>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sprite_q: Query<(&Transform, &Sprite), With<GridSprite>>,
) {
    if !editor.active {
        return;
    }

    // Don't paint when Shift is held (selection mode) or Ctrl is held (shortcuts).
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if shift || ctrl {
        return;
    }

    // Only paint on left-click or left-drag.
    let clicking = mouse.just_pressed(MouseButton::Left) || mouse.pressed(MouseButton::Left);
    if !clicking {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_global)) = camera_q.single() else { return };
    let Ok((sprite_tf, sprite)) = sprite_q.single() else { return };

    let grid_w = sim.grid.width();
    let grid_h = sim.grid.height();

    // ── Pattern stamp mode: apply on click ─────────────────────────
    if editor.pending_pattern.is_some() && mouse.just_pressed(MouseButton::Left) {
        if let Some((gx, gy)) = cursor_to_grid_xy(
            window, camera, cam_global, sprite_tf, sprite, grid_w, grid_h,
        ) {
            // Take the pattern out temporarily to avoid borrow conflict.
            let pattern = editor.pending_pattern.take().expect("checked above");
            let placed = apply_pattern_to_grid(&pattern, gx, gy, &mut sim.grid);
            let frame = editor.frame_count;
            editor.hud_message = Some((
                format!("Stamped pattern ({placed} actors placed)"),
                frame + HUD_MESSAGE_FRAMES,
            ));
            // Pattern is consumed after stamping; exit stamp mode.
        }
        return;
    }

    // ── Normal brush painting ──────────────────────────────────────
    let Some((gx, gy)) = cursor_to_grid_xy(
        window, camera, cam_global, sprite_tf, sprite, grid_w, grid_h,
    ) else {
        return;
    };

    let cells = compute_brush_cells(grid_w, grid_h, gx, gy, editor.radius);
    let brush_type = editor.brush_type;
    let intensity = editor.intensity;

    match brush_type {
        BrushType::Heat | BrushType::Chemical(_) => {
            for &cell_idx in &cells {
                apply_brush_to_cell(&mut sim.grid, cell_idx, brush_type, intensity);
            }
        }
        BrushType::Actor => {
            if sim.grid.actor_config().is_none() {
                let frame = editor.frame_count;
                editor.hud_message = Some((
                    "Cannot place actors: ActorConfig not present".to_string(),
                    frame + HUD_MESSAGE_FRAMES,
                ));
                return;
            }
            let seed_traits = HeritableTraits::from_config(
                sim.grid.actor_config().expect("checked above"),
            );
            for &cell_idx in &cells {
                if sim.grid.occupancy()[cell_idx].is_some() {
                    continue; // Skip occupied cells.
                }
                let actor = Actor {
                    cell_index: cell_idx,
                    energy: intensity,
                    inert: false,
                    tumble_direction: 0,
                    tumble_remaining: 0,
                    traits: seed_traits,
                    cooldown_remaining: 0,
                };
                let _ = sim.grid.add_actor(actor);
            }
        }
        BrushType::Eraser => {
            for &cell_idx in &cells {
                let _ = apply_eraser_to_cell(&mut sim.grid, cell_idx);
            }
        }
    }
}

// ── HUD System ─────────────────────────────────────────────────────

/// Update the editor HUD text: brush type, intensity, radius, messages.
///
/// Spawns the HUD entity on first edit mode activation. Hides it when
/// edit mode is deactivated.
///
/// COLD PATH: Runs every `Update` frame, string formatting only when active.
pub fn editor_update_hud(
    mut commands: Commands,
    mut editor: ResMut<EditorState>,
    mut hud_q: Query<(Entity, &mut Text, &mut Visibility), With<EditorHud>>,
    mut spawned: Local<bool>,
) {
    if !editor.active {
        // Hide HUD if it exists.
        for (_, _, mut vis) in hud_q.iter_mut() {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    }

    // Clear expired HUD messages.
    if let Some((_, expiry)) = &editor.hud_message
        && editor.frame_count >= *expiry
    {
        editor.hud_message = None;
    }

    // Build HUD text.
    let brush_label = match editor.brush_type {
        BrushType::Heat => "Heat".to_string(),
        BrushType::Chemical(s) => format!("Chemical {}", s + 1),
        BrushType::Actor => "Actor".to_string(),
        BrushType::Eraser => "Eraser".to_string(),
    };

    let stamp_info = if editor.pending_pattern.is_some() {
        " [STAMP MODE]"
    } else {
        ""
    };

    let selection_info = if let Some((s, e)) = editor.selection {
        format!("  Sel: {s}..{e}")
    } else {
        String::new()
    };

    let message = editor
        .hud_message
        .as_ref()
        .map(|(msg, _)| format!("\n{msg}"))
        .unwrap_or_default();

    let hud_text = format!(
        "EDIT MODE  |  Brush: {brush_label}  |  Intensity: {:.1}  |  Radius: {}{stamp_info}{selection_info}{message}",
        editor.intensity, editor.radius,
    );

    if *spawned {
        // Update existing HUD entity.
        for (_, mut text, mut vis) in hud_q.iter_mut() {
            if *vis != Visibility::Visible {
                *vis = Visibility::Visible;
            }
            if **text != hud_text {
                **text = hud_text.clone();
            }
        }
    } else {
        // Spawn HUD entity.
        commands.spawn((
            Text::new(hud_text),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(Color::srgba(1.0, 1.0, 0.2, 1.0)),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(40.0),
                right: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            EditorHud,
        ));
        *spawned = true;
    }
}

// ── Stamp Preview System ───────────────────────────────────────────

/// Render a translucent preview overlay showing which cells the brush
/// or pattern stamp will affect.
///
/// COLD PATH: Runs every `Update` frame. Spawns/despawns preview sprites
/// as needed. Uses a simple approach: one sprite per preview cell.
pub fn editor_stamp_preview(
    mut commands: Commands,
    editor: Res<EditorState>,
    sim: Res<SimulationState>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sprite_q: Query<(&Transform, &Sprite), With<GridSprite>>,
    preview_q: Query<Entity, With<StampPreview>>,
) {
    // Despawn all existing preview entities each frame.
    for entity in preview_q.iter() {
        commands.entity(entity).despawn();
    }

    if !editor.active {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_global)) = camera_q.single() else { return };
    let Ok((sprite_tf, grid_sprite)) = sprite_q.single() else { return };

    let grid_w = sim.grid.width();
    let grid_h = sim.grid.height();

    let Some((gx, gy)) = cursor_to_grid_xy(
        window, camera, cam_global, sprite_tf, grid_sprite, grid_w, grid_h,
    ) else {
        return; // Cursor outside grid — hide preview.
    };

    let sprite_size = grid_sprite.custom_size.unwrap_or(Vec2::ONE);
    let sprite_origin = sprite_tf.translation.truncate() - sprite_size * 0.5;

    let preview_color = Color::srgba(0.3, 0.8, 1.0, 0.3);

    // Determine which cells to highlight.
    let cells: SmallVec<[usize; 64]> = if let Some(ref pattern) = editor.pending_pattern {
        // Pattern stamp preview: show the pattern footprint.
        let mut c = SmallVec::new();
        for py in 0..pattern.height {
            for px in 0..pattern.width {
                let abs_x = gx + px;
                let abs_y = gy + py;
                if abs_x < grid_w && abs_y < grid_h {
                    c.push((abs_y as usize) * (grid_w as usize) + (abs_x as usize));
                }
            }
        }
        c
    } else {
        // Normal brush preview.
        compute_brush_cells(grid_w, grid_h, gx, gy, editor.radius)
    };

    // Spawn a translucent sprite for each preview cell.
    for &cell_idx in &cells {
        let cx = (cell_idx % grid_w as usize) as f32;
        // Grid y is top-down in cell index, but world y is bottom-up.
        let cy = (grid_h as f32) - 1.0 - (cell_idx / grid_w as usize) as f32;

        let world_x = sprite_origin.x + cx + 0.5;
        let world_y = sprite_origin.y + cy + 0.5;

        commands.spawn((
            Sprite {
                color: preview_color,
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            Transform::from_xyz(world_x, world_y, 1.0),
            StampPreview,
        ));
    }
}
