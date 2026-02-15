// Bevy-based 2D grid visualization plugin.
//
// COLD: Plugin registration runs once at app build time.
// Systems registered here are classified individually (see systems.rs).

use bevy::prelude::*;

use std::time::Duration;

use resources::BevyVizConfig;

pub mod color;
pub mod normalize;
pub mod resources;
pub mod setup;
pub mod systems;

/// Bevy plugin that wires the grid visualization systems into the app.
///
/// Expects a `BevyVizConfig` resource to be inserted before this plugin
/// is added. The plugin reads `tick_hz` from the config to derive the
/// `FixedUpdate` timestep.
///
/// Requirements: 2.3 (configurable timestep), 2.4 (fixed timestep decoupling).
pub struct BevyVizPlugin;

impl Plugin for BevyVizPlugin {
    fn build(&self, app: &mut App) {
        // Read tick_hz from the already-inserted BevyVizConfig resource.
        let tick_hz = app
            .world()
            .get_resource::<BevyVizConfig>()
            .map(|c| c.tick_hz)
            .unwrap_or(10.0);

        // Configure the FixedUpdate timestep from tick_hz.
        let timestep = Duration::from_secs_f64(1.0 / tick_hz);
        app.insert_resource(Time::<Fixed>::from_duration(timestep));

        // Cap the virtual clock's max delta to prevent FixedUpdate death spirals.
        // At high tick rates (512+ Hz), a single slow frame can accumulate enough
        // time debt to trigger dozens of catch-up ticks, which makes the next frame
        // even slower — a positive feedback loop. Capping max_delta to ~4 frames
        // worth of wall time (66ms ≈ 15fps floor) means Bevy will drop sim ticks
        // rather than spiral. The simulation slows down gracefully instead of
        // locking up.
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_millis(66));

        // Startup: initialize simulation, spawn entities.
        app.add_systems(Startup, setup::setup);

        // FixedUpdate: advance simulation by one tick.
        app.add_systems(
            FixedUpdate,
            (
                systems::tick_simulation,
                systems::compute_trait_stats.after(systems::tick_simulation),
            ),
        );

        // Update: input, texture upload, camera, label — all run every frame.
        app.add_systems(
            Update,
            (
                systems::handle_input,
                systems::rate_control_input,
                systems::select_actor_input.before(systems::update_texture),
                systems::clear_stale_selection,
                systems::update_actor_inspector,
                systems::update_texture,
                systems::camera_controls,
                systems::update_overlay_label,
                systems::update_rate_label,
                systems::update_hover_tooltip,
                systems::update_scale_bar,
                systems::info_panel_input,
                systems::update_info_panel,
                systems::stats_panel_input,
                systems::update_stats_panel,
            ),
        );
    }
}
