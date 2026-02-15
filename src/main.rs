// COLD PATH: Application entry point and render loop.
// Allocations, `anyhow`, and dynamic dispatch are permitted.

use std::thread;
use std::time::Duration;

use emergent_sovereignty::grid::config::GridConfig;
use emergent_sovereignty::grid::tick::TickOrchestrator;
use emergent_sovereignty::grid::world_init::{self, WorldInitConfig};
use emergent_sovereignty::grid::Grid;
use emergent_sovereignty::viz::renderer::Renderer;
use emergent_sovereignty::viz::{InputAction, OverlayMode, RendererConfig};

fn main() {
    // Accept an optional seed as the first CLI argument; default to 42.
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);

    let grid_config = GridConfig {
        width: 30,
        height: 30,
        num_chemicals: 1,
        diffusion_rate: 0.05,
        thermal_conductivity: 0.05,
        ambient_heat: 0.0,
        tick_duration: 1.0,
        num_threads: 4,
    };

    let init_config = WorldInitConfig::default();

    let viz_config = RendererConfig {
        frame_delay_ms: 50,
        initial_overlay: OverlayMode::Chemical(0),
    };

    let grid = match world_init::initialize(seed, grid_config.clone(), &init_config) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Fatal: world initialization failed: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = run_visualization(grid, &grid_config, viz_config) {
        eprintln!("Fatal: {e:#}");
        std::process::exit(1);
    }
}

/// Run the simulation with terminal visualization.
///
/// Accepts a pre-initialized Grid from `world_init::initialize`,
/// replacing the previous hardcoded source registration.
///
/// Requirements: 6.1 (seed via CLI), 6.2 (default WorldInitConfig),
/// 6.3 (Grid from world_init replaces hardcoded init).
fn run_visualization(
    mut grid: Grid,
    config: &GridConfig,
    viz_config: RendererConfig,
) -> anyhow::Result<()> {
    let frame_delay = Duration::from_millis(viz_config.frame_delay_ms);
    let mut renderer = Renderer::init(viz_config)?;

    // Run the tick loop. Capture the result so shutdown() always runs.
    let loop_result = tick_loop(&mut renderer, &mut grid, config, frame_delay);

    // Shutdown must run regardless of how the loop exited.
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
