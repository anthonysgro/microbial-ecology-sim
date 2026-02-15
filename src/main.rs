// COLD PATH: Application entry point and render loop.
// Allocations, `anyhow`, and dynamic dispatch are permitted.

use std::thread;
use std::time::Duration;

use emergent_sovereignty::grid::config::GridConfig;
use emergent_sovereignty::grid::tick::TickOrchestrator;
use emergent_sovereignty::grid::world_init;
use emergent_sovereignty::grid::Grid;
use emergent_sovereignty::io::cli::parse_cli_args;
use emergent_sovereignty::io::config_file::{load_world_config, validate_world_config, WorldConfig};
use emergent_sovereignty::viz::renderer::Renderer;
use emergent_sovereignty::viz::{InputAction, OverlayMode, RendererConfig};

fn main() {
    if let Err(e) = run() {
        eprintln!("Fatal: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    // 1. Parse CLI arguments.
    let cli = parse_cli_args()?;

    // 2. Load config file if provided, otherwise use compiled defaults.
    let mut config = match cli.config_path {
        Some(ref path) => load_world_config(path)?,
        None => WorldConfig::default(),
    };

    // 3. Apply CLI seed override (positional seed takes precedence over TOML).
    if let Some(seed) = cli.seed_override {
        config.seed = seed;
    }

    // 4. Validate cross-field invariants.
    validate_world_config(&config)?;

    // 5. Initialize the world from the validated config.
    let grid = world_init::initialize(
        config.seed,
        config.grid.clone(),
        &config.world_init,
        config.actor,
    )?;

    let viz_config = RendererConfig {
        frame_delay_ms: 50,
        initial_overlay: OverlayMode::Chemical(0),
    };

    run_visualization(grid, &config.grid, viz_config)
}

/// Run the simulation with terminal visualization.
fn run_visualization(
    mut grid: Grid,
    config: &GridConfig,
    viz_config: RendererConfig,
) -> anyhow::Result<()> {
    let frame_delay = Duration::from_millis(viz_config.frame_delay_ms);
    let mut renderer = Renderer::init(viz_config)?;

    let loop_result = tick_loop(&mut renderer, &mut grid, config, frame_delay);

    renderer.shutdown()?;

    loop_result
}

/// Inner tick loop extracted so the caller can guarantee cleanup.
fn tick_loop(
    renderer: &mut Renderer,
    grid: &mut Grid,
    config: &GridConfig,
    frame_delay: Duration,
) -> anyhow::Result<()> {
    let mut tick: u64 = 0;

    loop {
        TickOrchestrator::step(grid, config)?;
        tick += 1;

        renderer.render_frame(grid, tick)?;

        match renderer.poll_input(config.num_chemicals)? {
            InputAction::Quit => break,
            InputAction::SwitchOverlay(mode) => renderer.set_overlay(mode),
            InputAction::None => {}
        }

        thread::sleep(frame_delay);
    }

    Ok(())
}
