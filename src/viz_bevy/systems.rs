// WARM PATH: tick_simulation runs every FixedUpdate, advancing the grid.
// COLD PATH: input, camera, label systems run every Update frame.
// Allocation forbidden in tick_simulation. Standard rules for Update systems.

use bevy::prelude::*;

use crate::grid::tick::TickOrchestrator;

use super::resources::SimulationState;

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
