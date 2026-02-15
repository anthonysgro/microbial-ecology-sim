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

        // Startup: initialize simulation, spawn entities.
        app.add_systems(Startup, setup::setup);

        // FixedUpdate: advance simulation by one tick.
        app.add_systems(FixedUpdate, systems::tick_simulation);

        // Update: input, texture upload, camera, label — all run every frame.
        app.add_systems(
            Update,
            (
                systems::handle_input,
                systems::rate_control_input,
                systems::update_texture,
                systems::camera_controls,
                systems::update_overlay_label,
                systems::update_rate_label,
                systems::update_hover_tooltip,
                systems::update_scale_bar,
            ),
        );
    }
}
