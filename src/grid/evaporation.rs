// HOT PATH: Executes per tick over all grid cells.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

use crate::grid::config::GridConfig;
use crate::grid::error::TickError;
use crate::grid::Grid;
use rayon::prelude::*;

/// Wrapper to send a raw mutable pointer across thread boundaries.
///
/// # Safety
///
/// The caller must guarantee that concurrent writes through this pointer
/// target non-overlapping index ranges. This is upheld by the spatial
/// partition invariant: partitions are disjoint, so no two threads write
/// to the same cell index.
struct SendPtr(*mut f32);

// SAFETY: Partitions are non-overlapping (Property 10 / Requirement 7.4),
// so concurrent writes through this pointer never alias. Each thread
// writes exclusively to its own partition's cell indices.
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

impl SendPtr {
    #[inline]
    fn ptr(&self) -> *mut f32 {
        self.0
    }
}

/// Run moisture evaporation across the entire grid.
///
/// Per-cell formula (no neighbor interaction):
///   `loss = evaporation_coefficient × heat × moisture × tick_duration`
///   `new_moisture = max(0.0, moisture - loss)`
///
/// Reads heat and moisture from the read buffers, writes updated moisture
/// to the write buffer. Parallelized over spatial partitions via `rayon`.
///
/// # Requirements
/// 5.1 — reduces moisture based on heat and evaporation coefficient
/// 5.2 — loss = coeff × heat × moisture × dt
/// 5.3 — clamp moisture to zero (never negative)
/// 5.4 — data parallelism via spatial partitions
pub fn run_evaporation(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError> {
    let coeff = config.evaporation_coefficient;
    let dt = config.tick_duration;
    let width = config.width;

    // Clone partitions to release the borrow on grid before we take
    // mutable references to the moisture buffer.
    let partitions = grid.partitions().to_vec();

    // Heat is read-only; moisture needs read + write.
    // Combined accessor avoids borrow-checker conflict from separate
    // immutable (heat) and mutable (moisture) borrows on Grid.
    let (heat_read, moisture_read, moisture_write) = grid.heat_read_moisture_rw();

    let write_ptr = SendPtr(moisture_write.as_mut_ptr());
    let write_len = moisture_write.len();

    partitions.par_iter().for_each(|partition| {
        let ptr = write_ptr.ptr();
        for y in partition.start_row..partition.end_row {
            for x in partition.start_col..partition.end_col {
                let idx = (y as usize) * (width as usize) + (x as usize);

                let h = heat_read[idx];
                let m = moisture_read[idx];
                let loss = coeff * h * m * dt;
                // Clamp to zero: moisture can never go negative.
                let new_m = (m - loss).max(0.0);

                // SAFETY: `idx` is within [0, write_len) because x < width
                // and y < height. Partitions are disjoint (Requirement 7.4),
                // so no two threads write to the same idx.
                debug_assert!(idx < write_len);
                unsafe {
                    ptr.add(idx).write(new_m);
                }
            }
        }
    });

    Ok(())
}
