// COLD PATH: Application entry point and render loop.
// Allocations, `anyhow`, and dynamic dispatch are permitted.

use std::thread;
use std::time::Duration;

use emergent_sovereignty::grid::config::{CellDefaults, GridConfig};
use emergent_sovereignty::grid::tick::TickOrchestrator;
use emergent_sovereignty::grid::Grid;
use emergent_sovereignty::viz::renderer::Renderer;
use emergent_sovereignty::viz::{InputAction, OverlayMode, RendererConfig};

fn main() {
    let config = GridConfig {
        width: 10,
        height: 10,
        num_chemicals: 1,
        diffusion_rate: 0.05,
        thermal_conductivity: 0.05,
        evaporation_coefficient: 0.01,
        ambient_heat: 0.0,
        tick_duration: 1.0,
        num_threads: 4,
    };

    let defaults = CellDefaults {
        chemical_concentrations: vec![0.0],
        heat: 0.0,
        moisture: 1.0,
    };

    let viz_config = RendererConfig {
        frame_delay_ms: 50,
        initial_overlay: OverlayMode::Chemical(0),
    };

    if let Err(e) = run_visualization(config, defaults, viz_config) {
        eprintln!("Fatal: {e:#}");
        std::process::exit(1);
    }
}

/// Run the simulation with terminal visualization.
///
/// Guarantees `Renderer::shutdown()` executes on all exit paths:
/// normal quit, simulation error, or render error.
///
/// Requirements: 5.1 (frame per tick), 5.2 (configurable delay),
/// 5.3 (quit on q/Esc), 5.4 (tick in stats bar).
fn run_visualization(
    config: GridConfig,
    defaults: CellDefaults,
    viz_config: RendererConfig,
) -> anyhow::Result<()> {
    let mut grid = Grid::new(config.clone(), defaults)?;

    // Seed a chemical hot spot in the center.
    let center = grid.index(5, 5)?;
    grid.write_chemical(0)?[center] = 100.0;
    grid.swap_chemicals();

    // Seed a heat source in the center.
    grid.write_heat()[center] = 50.0;
    grid.swap_heat();

    let frame_delay = Duration::from_millis(viz_config.frame_delay_ms);
    let mut renderer = Renderer::init(viz_config)?;

    // Run the tick loop. Capture the result so shutdown() always runs.
    let loop_result = tick_loop(&mut renderer, &mut grid, &config, frame_delay);

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
