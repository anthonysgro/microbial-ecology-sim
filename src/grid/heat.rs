// HOT PATH: Executes per tick over all grid cells.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

use crate::grid::config::GridConfig;
use crate::grid::error::TickError;
use crate::grid::Grid;
use rayon::prelude::*;

/// 8-connectivity neighbor offsets: (dx, dy).
const NEIGHBOR_OFFSETS: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

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

/// Pre-computed grid parameters passed to per-cell heat radiation.
/// Avoids passing many scalar arguments through the hot loop.
struct HeatParams {
    width: u32,
    height: u32,
    conductivity: f32,
    dt: f32,
    ambient: f32,
}

/// Compute the radiated heat value for a single cell.
///
/// Discrete Laplacian with 8-connectivity. Boundary condition: missing
/// neighbors use `ambient_heat` instead of zero (contrast with diffusion).
#[inline]
fn radiate_cell(read: &[f32], idx: usize, x: u32, y: u32, p: &HeatParams) -> f32 {
    let current = read[idx];
    let mut flow_sum: f32 = 0.0;

    for &(dx, dy) in &NEIGHBOR_OFFSETS {
        let nx = x as i64 + dx;
        let ny = y as i64 + dy;

        // Ambient boundary: out-of-bounds neighbors contribute ambient_heat.
        let neighbor_heat =
            if nx >= 0 && ny >= 0 && (nx as u32) < p.width && (ny as u32) < p.height {
                read[(ny as u32 as usize) * (p.width as usize) + (nx as u32 as usize)]
            } else {
                p.ambient
            };

        flow_sum += neighbor_heat - current;
    }

    current + p.conductivity * flow_sum * p.dt
}

/// Run heat radiation across the entire grid.
///
/// Reads heat values from the read buffer, computes the discrete
/// Laplacian with 8-connectivity, and writes updated values to the
/// write buffer. Ambient boundary condition: missing neighbors use
/// `config.ambient_heat`.
///
/// Parallelized over spatial partitions via `rayon`.
///
/// # Requirements
/// 4.1 — reads from read buffer, writes to write buffer
/// 4.2 — net flow scaled by thermal_conductivity
/// 4.3 — ambient boundary (missing neighbors use config.ambient_heat)
/// 4.4 — energy conservation with ambient accounting
/// 4.5 — data parallelism via spatial partitions
pub fn run_heat(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError> {
    let params = HeatParams {
        width: config.width,
        height: config.height,
        conductivity: config.thermal_conductivity,
        dt: config.tick_duration,
        ambient: config.ambient_heat,
    };

    // Clone partitions to release the borrow on grid before we take
    // mutable references to the heat buffer.
    let partitions = grid.partitions().to_vec();

    let (read, write) = grid.read_write_heat();
    let write_ptr = SendPtr(write.as_mut_ptr());
    let write_len = write.len();

    partitions.par_iter().for_each(|partition| {
        let ptr = write_ptr.ptr();
        for y in partition.start_row..partition.end_row {
            for x in partition.start_col..partition.end_col {
                let idx = (y as usize) * (params.width as usize) + (x as usize);
                let new_val = radiate_cell(read, idx, x, y, &params);

                // SAFETY: `idx` is within [0, write_len) because x < width
                // and y < height. Partitions are disjoint (Requirement 7.4),
                // so no two threads write to the same idx.
                debug_assert!(idx < write_len);
                unsafe {
                    ptr.add(idx).write(new_val);
                }
            }
        }
    });

    Ok(())
}
