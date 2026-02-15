// COLD PATH: Bevy visualization binary entry point.
// Parses CLI args, constructs BevyVizConfig, builds and runs the Bevy app.
//
// Requirements: 10.1 (seed + config init), 10.3 (CLI seed argument).

use bevy::prelude::*;

use emergent_sovereignty::grid::config::GridConfig;
use emergent_sovereignty::grid::world_init::WorldInitConfig;
use emergent_sovereignty::viz_bevy::resources::{ActiveOverlay, BevyVizConfig};
use emergent_sovereignty::viz_bevy::BevyVizPlugin;

fn main() {
    // Accept an optional seed as the first CLI argument; default to 42.
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);

    let grid_config = GridConfig {
        width: 128,
        height: 128,
        num_chemicals: 1,
        diffusion_rate: 0.05,
        thermal_conductivity: 0.05,
        ambient_heat: 0.0,
        tick_duration: 1.0,
        num_threads: 4,
    };

    let config = BevyVizConfig {
        seed,
        grid_config,
        init_config: WorldInitConfig::default(),
        initial_overlay: ActiveOverlay::Chemical(0),
        tick_hz: 10.0,
        zoom_min: 0.1,
        zoom_max: 10.0,
        zoom_speed: 0.1,
        pan_speed: 1.0,
        color_scale_max: 10.0,
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Emergent Sovereignty — Grid Visualization".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(config)
        .add_plugins(BevyVizPlugin)
        .run();
}
