// HOT PATH: Executes per tick over all grid cells for each chemical species.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

use crate::grid::config::GridConfig;
use crate::grid::error::TickError;
use crate::grid::Grid;

/// Apply exponential decay to all chemical species.
///
/// For each species with a non-zero decay rate:
/// 1. Copy read buffer → write buffer
/// 2. Multiply each cell in the write buffer by `(1.0 - decay_rate)`
/// 3. Clamp to >= 0.0
///
/// Species with `decay_rate == 0.0` are skipped entirely (no copy, no write).
/// Caller is responsible for validation and swap after this function returns.
///
/// # Requirements
/// 2.1 — `concentration *= (1.0 - decay_rate)` per species
/// 2.4 — deterministic species-index iteration order
/// 2.5 — skip species with rate == 0.0
/// 2.6 — clamp to >= 0.0
pub fn run_decay(
    grid: &mut Grid,
    config: &GridConfig,
    decay_rates: &[f32],
) -> Result<(), TickError> {
    for species in 0..config.num_chemicals {
        let rate = decay_rates[species];

        // Skip species with zero decay — no copy, no write, no cost.
        if rate == 0.0 {
            continue;
        }

        let factor = 1.0 - rate;

        // Copy read → write, then apply decay in-place on the write buffer.
        if let Some(buf) = grid.chemical_buffer_mut(species) {
            buf.copy_read_to_write();
        }

        if let Ok(write) = grid.write_chemical(species) {
            for val in write.iter_mut() {
                *val *= factor;
                // Clamp to >= 0.0 to prevent negative values from
                // floating-point rounding. Compiles to maxss/fmax.
                if *val < 0.0 {
                    *val = 0.0;
                }
            }
        }
    }

    Ok(())
}
