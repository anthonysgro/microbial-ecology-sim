// HOT PATH: Executes per tick, orchestrating all environmental systems.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

use crate::grid::config::GridConfig;
use crate::grid::diffusion::run_diffusion;
use crate::grid::error::TickError;
use crate::grid::evaporation::run_evaporation;
use crate::grid::heat::run_heat;
use crate::grid::Grid;

/// Scan a write buffer for NaN or infinity values.
///
/// Returns `Err(TickError::NumericalError)` on the first invalid value
/// found, preserving the read buffer as a last-known-good state.
#[inline]
fn validate_buffer(
    buffer: &[f32],
    system: &'static str,
    field: &'static str,
) -> Result<(), TickError> {
    for (cell_index, &value) in buffer.iter().enumerate() {
        if value.is_nan() || value.is_infinite() {
            return Err(TickError::NumericalError {
                system,
                cell_index,
                field,
                value,
            });
        }
    }
    Ok(())
}

/// Drives the per-tick execution sequence.
///
/// Runs each environmental system in order with NaN/infinity validation
/// and buffer swaps between them:
///
/// 1. Diffusion  → validate chemical write buffers → swap chemicals
/// 2. Heat       → validate heat write buffer      → swap heat
/// 3. Evaporation → validate moisture write buffer  → swap moisture
///
/// If validation fails, the tick halts immediately. The read buffer
/// retains the last valid state for diagnostics.
///
/// # Requirements
/// 9.1 — sequential system execution order
/// 9.2 — buffer swap between systems
/// 9.4 — NaN/infinity detection before swap
pub struct TickOrchestrator;

impl TickOrchestrator {
    pub fn step(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError> {
        // Phase 1: Chemical diffusion
        run_diffusion(grid, config)?;
        for species in 0..config.num_chemicals {
            let write_buf = grid
                .write_chemical(species)
                .expect("species index validated by config.num_chemicals");
            let field_name = match species {
                0 => "chemical_0",
                1 => "chemical_1",
                2 => "chemical_2",
                3 => "chemical_3",
                _ => "chemical_N",
            };
            validate_buffer(write_buf, "diffusion", field_name)?;
        }
        grid.swap_chemicals();

        // Phase 2: Heat radiation
        run_heat(grid, config)?;
        validate_buffer(grid.write_heat(), "heat", "heat")?;
        grid.swap_heat();

        // Phase 3: Moisture evaporation
        run_evaporation(grid, config)?;
        validate_buffer(grid.write_moisture(), "evaporation", "moisture")?;
        grid.swap_moisture();

        Ok(())
    }
}
