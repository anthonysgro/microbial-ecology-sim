/// Actor system functions: sensing, metabolism, movement, deferred removal.
///
/// All functions are free (stateless), matching the existing pattern
/// (`run_emission`, `run_diffusion`, `run_heat`). Each operates on
/// borrowed slices and registry references — no owned state, no
/// dynamic dispatch, no heap allocation.

use crate::grid::actor::ActorRegistry;

// WARM PATH: Executes once per tick over the actor list.
// No heap allocation. No dynamic dispatch. Sequential iteration.

/// Compute movement targets for all active Actors based on local
/// chemical gradients (Von Neumann neighborhood, species 0).
///
/// For each Actor in deterministic slot-index order:
/// 1. Read chemical concentration at the Actor's cell and its four
///    orthogonal neighbors from `chemical_read`.
/// 2. Compute the gradient (neighbor − current) for each in-bounds
///    neighbor. Out-of-bounds neighbors are treated as 0.0 concentration.
/// 3. Select the neighbor with the maximum positive gradient. Ties are
///    broken by direction priority: North, South, West, East (first
///    encountered wins).
/// 4. Write `Some(target_cell_index)` into `movement_targets[slot_index]`,
///    or `None` if no neighbor has a positive gradient.
///
/// # Arguments
///
/// * `actors` — shared reference to the actor registry (read-only).
/// * `chemical_read` — read buffer for chemical species 0, length = cell_count.
/// * `grid_width` — number of columns in the grid.
/// * `grid_height` — number of rows in the grid.
/// * `movement_targets` — pre-allocated buffer indexed by slot index.
///   Entries for inactive slots are left untouched.
pub fn run_actor_sensing(
    actors: &ActorRegistry,
    chemical_read: &[f32],
    grid_width: u32,
    grid_height: u32,
    movement_targets: &mut [Option<usize>],
) {
    let w = grid_width as usize;
    let h = grid_height as usize;

    for (slot_index, actor) in actors.iter() {
        let ci = actor.cell_index;
        let x = ci % w;
        let y = ci / w;
        let current_val = chemical_read[ci];

        let mut best_gradient: f32 = 0.0;
        let mut best_target: Option<usize> = None;

        // North (y - 1)
        if y > 0 {
            let ni = (y - 1) * w + x;
            let gradient = chemical_read[ni] - current_val;
            if gradient > best_gradient {
                best_gradient = gradient;
                best_target = Some(ni);
            }
        }

        // South (y + 1)
        if y + 1 < h {
            let ni = (y + 1) * w + x;
            let gradient = chemical_read[ni] - current_val;
            if gradient > best_gradient {
                best_gradient = gradient;
                best_target = Some(ni);
            }
        }

        // West (x - 1)
        if x > 0 {
            let ni = y * w + (x - 1);
            let gradient = chemical_read[ni] - current_val;
            if gradient > best_gradient {
                best_gradient = gradient;
                best_target = Some(ni);
            }
        }

        // East (x + 1)
        if x + 1 < w {
            let ni = y * w + (x + 1);
            let gradient = chemical_read[ni] - current_val;
            if gradient > best_gradient {
                // Final direction — no need to update best_gradient.
                best_target = Some(ni);
            }
        }

        movement_targets[slot_index] = best_target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::actor::{Actor, ActorRegistry};

    /// 3×3 grid, actor at center (1,1). Highest concentration at North (0,1).
    /// Expected: movement target = North cell index.
    #[test]
    fn sensing_selects_max_gradient_neighbor() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        // Actor at center cell (index 4 on a 3×3 grid: x=1, y=1)
        let actor = Actor { cell_index: 4, energy: 10.0 };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        // Chemical buffer: center=1.0, north=5.0, south=2.0, west=3.0, east=4.0
        // Indices: (0,0)=0 (1,0)=1 (2,0)=2
        //          (0,1)=3 (1,1)=4 (2,1)=5
        //          (0,2)=6 (1,2)=7 (2,2)=8
        // North of (1,1) is (1,0) = index 1
        // South of (1,1) is (1,2) = index 7
        // West  of (1,1) is (0,1) = index 3
        // East  of (1,1) is (2,1) = index 5
        let mut chemical = vec![0.0; 9];
        chemical[4] = 1.0; // center
        chemical[1] = 5.0; // north — highest gradient (5.0 - 1.0 = 4.0)
        chemical[7] = 2.0; // south
        chemical[3] = 3.0; // west
        chemical[5] = 4.0; // east

        let mut targets = vec![None; 1];
        run_actor_sensing(&registry, &chemical, 3, 3, &mut targets);

        assert_eq!(targets[0], Some(1), "should select north (index 1) as max gradient");
    }

    /// Actor at corner (0,0) on a 3×3 grid. Only South and East are in-bounds.
    /// Out-of-bounds neighbors treated as 0.0.
    #[test]
    fn sensing_boundary_cell_treats_oob_as_zero() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 0, energy: 10.0 };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        // Center (0,0) = 2.0, South (0,1) = index 3 = 5.0, East (1,0) = index 1 = 3.0
        let mut chemical = vec![0.0; 9];
        chemical[0] = 2.0;
        chemical[3] = 5.0; // south
        chemical[1] = 3.0; // east

        let mut targets = vec![None; 1];
        run_actor_sensing(&registry, &chemical, 3, 3, &mut targets);

        assert_eq!(targets[0], Some(3), "should select south (index 3) as max gradient");
    }

    /// No neighbor has a positive gradient → movement target is None.
    #[test]
    fn sensing_no_positive_gradient_stays() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0 };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        // Center has the highest value — all gradients are negative.
        let mut chemical = vec![0.0; 9];
        chemical[4] = 10.0;
        chemical[1] = 2.0;
        chemical[7] = 3.0;
        chemical[3] = 1.0;
        chemical[5] = 4.0;

        let mut targets = vec![None; 1];
        run_actor_sensing(&registry, &chemical, 3, 3, &mut targets);

        assert_eq!(targets[0], None, "no positive gradient → stay in place");
    }

    /// Tie-breaking: North and East have equal gradient. North is checked
    /// first in iteration order (N, S, W, E), so North wins.
    #[test]
    fn sensing_tie_breaks_by_direction_priority() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0 };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        let mut chemical = vec![0.0; 9];
        chemical[4] = 1.0; // center
        chemical[1] = 5.0; // north — gradient 4.0
        chemical[5] = 5.0; // east  — gradient 4.0 (tie)

        let mut targets = vec![None; 1];
        run_actor_sensing(&registry, &chemical, 3, 3, &mut targets);

        // North is checked before East, and we use strict `>`, so North wins the tie.
        assert_eq!(targets[0], Some(1), "tie-break: north wins over east");
    }
}
