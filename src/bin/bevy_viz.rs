// COLD PATH: Bevy visualization binary entry point.
// Parses CLI args, loads TOML config, builds and runs the Bevy app.

use bevy::prelude::*;

use emergent_sovereignty::io::cli::parse_cli_args;
use emergent_sovereignty::io::config_file::{
    load_bevy_config, validate_world_config, BevyWorldConfig,
};
use emergent_sovereignty::viz_bevy::resources::{ActiveOverlay, BevyVizConfig};
use emergent_sovereignty::viz_bevy::BevyVizPlugin;

fn main() {
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal: {e:#}");
            std::process::exit(1);
        }
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

fn load_config() -> anyhow::Result<BevyVizConfig> {
    // 1. Parse CLI arguments.
    let cli = parse_cli_args()?;

    // 2. Load config file if provided, otherwise use compiled defaults.
    let mut bevy_config = match cli.config_path {
        Some(ref path) => load_bevy_config(path)?,
        None => BevyWorldConfig::default(),
    };

    // 3. Apply CLI seed override (positional seed takes precedence over TOML).
    if let Some(seed) = cli.seed_override {
        bevy_config.world.seed = seed;
    }

    // 4. Validate cross-field invariants on the world config portion.
    validate_world_config(&bevy_config.world)?;

    // 5. Construct BevyVizConfig from the loaded + validated config.
    let viz = BevyVizConfig {
        seed: bevy_config.world.seed,
        grid_config: bevy_config.world.grid,
        init_config: bevy_config.world.world_init,
        actor_config: bevy_config.world.actor,
        initial_overlay: ActiveOverlay::Chemical(0),
        tick_hz: bevy_config.bevy.tick_hz,
        zoom_min: bevy_config.bevy.zoom_min,
        zoom_max: bevy_config.bevy.zoom_max,
        zoom_speed: bevy_config.bevy.zoom_speed,
        pan_speed: bevy_config.bevy.pan_speed,
        color_scale_max: bevy_config.bevy.color_scale_max,
    };

    Ok(viz)
}
