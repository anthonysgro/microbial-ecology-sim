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
/// Pre-computed grid parameters passed to per-cell diffusion.
/// Avoids passing many scalar arguments through the hot loop.
struct DiffuseParams {
    width: u32,
    height: u32,
    rate: f32,
    dt: f32,
}

/// Compute the diffused value for a single cell in a single chemical species.
///
/// Discrete Laplacian with 8-connectivity and open boundary (missing
/// neighbors have zero concentration).
#[inline]
fn diffuse_cell(read: &[f32], idx: usize, x: u32, y: u32, p: &DiffuseParams) -> f32 {
    let current = read[idx];
    let mut flow_sum: f32 = 0.0;

    for &(dx, dy) in &NEIGHBOR_OFFSETS {
        let nx = x as i64 + dx;
        let ny = y as i64 + dy;

        // Open boundary: out-of-bounds neighbors contribute zero.
        let neighbor_conc =
            if nx >= 0 && ny >= 0 && (nx as u32) < p.width && (ny as u32) < p.height {
                read[(ny as u32 as usize) * (p.width as usize) + (nx as u32 as usize)]
            } else {
                0.0
            };

        flow_sum += neighbor_conc - current;
    }

    current + p.rate * flow_sum * p.dt
}

/// Run chemical diffusion for all species.
///
/// Reads concentrations from the read buffer, computes the discrete
/// Laplacian with 8-connectivity, and writes updated values to the
/// write buffer. Open boundary condition: missing neighbors contribute
/// zero concentration.
///
/// Parallelized over spatial partitions via `rayon`.
///
/// # Requirements
/// 3.1 — reads from read buffer, writes to write buffer
/// 3.2 — net flow scaled by diffusion_rate
/// 3.3 — open boundary (zero concentration for missing neighbors)
/// 3.4 — mass conservation (discrete Laplacian preserves total mass
///        for interior cells; boundary cells leak to zero)
/// 3.5 — data parallelism via spatial partitions
pub fn run_diffusion(
    grid: &mut Grid,
    config: &GridConfig,
    diffusion_rates: &[f32],
) -> Result<(), TickError> {
    // Clone partitions to release the borrow on grid before we take
    // mutable references to the chemical buffers.
    let partitions = grid.partitions().to_vec();

    #[allow(clippy::needless_range_loop)]
    for species in 0..config.num_chemicals {
        let rate = diffusion_rates[species];

        // Skip species with zero diffusion — no read, no write, no cost.
        // Requirement 6.3.
        if rate == 0.0 {
            continue;
        }

        let params = DiffuseParams {
            width: config.width,
            height: config.height,
            rate,
            dt: config.tick_duration,
        };

        let (read, write) = grid.read_write_chemical(species)
            .expect("species index validated by config.num_chemicals");

        let write_ptr = SendPtr(write.as_mut_ptr());
        let write_len = write.len();

        partitions.par_iter().for_each(|partition| {
            let ptr = write_ptr.ptr();
            for y in partition.start_row..partition.end_row {
                for x in partition.start_col..partition.end_col {
                    let idx = (y as usize) * (params.width as usize) + (x as usize);
                    let new_val = diffuse_cell(read, idx, x, y, &params);

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
    }

    Ok(())
}
