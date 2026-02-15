/// Actor system functions: sensing, metabolism, movement, deferred removal.
///
/// All functions are free (stateless), matching the existing pattern
/// (`run_emission`, `run_diffusion`, `run_heat`). Each operates on
/// borrowed slices and registry references — no owned state, no
/// dynamic dispatch, no heap allocation.

use crate::grid::actor::{ActorId, ActorRegistry};
use crate::grid::actor_config::ActorConfig;
use crate::grid::error::TickError;

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

// WARM PATH: Executes once per tick over the actor list.
// No heap allocation. No dynamic dispatch. Sequential iteration.

/// Execute the metabolism phase for all active Actors.
///
/// For each Actor in deterministic slot-index order:
/// 1. Compute `consumed = min(consumption_rate, chemical_read[cell_index])`.
/// 2. Subtract `consumed` from `chemical_write[cell_index]`, clamping to 0.0.
/// 3. Update actor energy: `+= consumed * energy_conversion_factor - base_energy_decay`.
/// 4. If energy <= 0.0, push the `ActorId` into `removal_buffer` for deferred removal.
/// 5. Validate actor energy for NaN/Inf — return `TickError::NumericalError` on detection.
///
/// The caller is responsible for copying `chemical_read` → `chemical_write`
/// before invoking this function (same pattern as emission).
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry (energy is updated in-place).
/// * `chemical_read` — read buffer for chemical species 0, length = cell_count.
/// * `chemical_write` — write buffer for chemical species 0, pre-initialized from read buffer.
/// * `config` — actor configuration (consumption_rate, energy_conversion_factor, base_energy_decay).
/// * `removal_buffer` — pre-allocated buffer for dead actor ids. Cleared before use.
pub fn run_actor_metabolism(
    actors: &mut ActorRegistry,
    chemical_read: &[f32],
    chemical_write: &mut [f32],
    config: &ActorConfig,
    removal_buffer: &mut Vec<ActorId>,
) -> Result<(), TickError> {
    removal_buffer.clear();

    for (id, actor) in actors.iter_mut_with_ids() {
        let ci = actor.cell_index;
        let available = chemical_read[ci];

        // Consume the lesser of the configured rate and what's available.
        let consumed = config.consumption_rate.min(available);

        // Subtract from write buffer, clamp to non-negative.
        chemical_write[ci] -= consumed;
        if chemical_write[ci] < 0.0 {
            chemical_write[ci] = 0.0;
        }

        // Energy balance: gain from consumption, lose basal decay.
        actor.energy += consumed * config.energy_conversion_factor - config.base_energy_decay;

        // Validate for NaN/Inf before death check.
        if actor.energy.is_nan() || actor.energy.is_infinite() {
            return Err(TickError::NumericalError {
                system: "actor_metabolism",
                cell_index: ci,
                field: "energy",
                value: actor.energy,
            });
        }

        // Mark for deferred removal if energy depleted.
        if actor.energy <= 0.0 {
            removal_buffer.push(id);
        }
    }

    Ok(())
}

// WARM PATH: Executes once per tick over the actor list.
// No heap allocation. No dynamic dispatch. Sequential iteration.

/// Execute the movement phase for all active Actors.
///
/// For each Actor in deterministic slot-index order:
/// 1. Read the movement target from `movement_targets[slot_index]`.
/// 2. If `Some(target)`: check `occupancy[target]`.
///    - Unoccupied → update occupancy (clear old cell, set new cell),
///      update `actor.cell_index`.
///    - Occupied → skip (actor stays in place).
/// 3. If `None`: actor stays in place.
///
/// Lower slot indices are processed first, granting them movement
/// priority when multiple actors target the same cell.
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry (cell_index updated in-place).
/// * `occupancy` — mutable occupancy map, length = cell_count.
/// * `movement_targets` — pre-computed targets indexed by slot index.
pub fn run_actor_movement(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    movement_targets: &[Option<usize>],
) {
    for (slot_index, actor) in actors.iter_mut() {
        let target = match movement_targets.get(slot_index).copied().flatten() {
            Some(t) => t,
            None => continue,
        };

        // Target cell occupied → skip. Lower slot indices already claimed it.
        if occupancy[target].is_some() {
            continue;
        }

        // Move: clear old occupancy, set new occupancy, update actor position.
        let old_cell = actor.cell_index;
        occupancy[old_cell] = None;
        occupancy[target] = Some(slot_index);
        actor.cell_index = target;
    }
}

// WARM PATH: Executes once per tick after metabolism completes.
// No heap allocation. No dynamic dispatch.

/// Remove all Actors marked for death during the metabolism phase.
///
/// Sorts the removal buffer by slot index (ascending) for deterministic
/// removal order, then calls `ActorRegistry::remove` for each entry,
/// clearing the corresponding occupancy map slot. The buffer is cleared
/// after all removals complete.
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry.
/// * `occupancy` — mutable occupancy map, length = cell_count.
/// * `removal_buffer` — buffer of `ActorId`s populated by metabolism.
///   Cleared after processing.
pub fn run_deferred_removal(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    removal_buffer: &mut Vec<ActorId>,
) -> Result<(), crate::grid::actor::ActorError> {
    // Sort by slot index (ascending) for deterministic removal order.
    removal_buffer.sort_unstable_by_key(|id| id.index);

    for &id in removal_buffer.iter() {
        actors.remove(id, occupancy)?;
    }

    removal_buffer.clear();
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::actor::{Actor, ActorRegistry};
    use crate::grid::actor_config::ActorConfig;

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

    // ── Metabolism tests ──────────────────────────────────────────────

    fn default_config() -> ActorConfig {
        ActorConfig {
            consumption_rate: 2.0,
            energy_conversion_factor: 1.5,
            base_energy_decay: 0.5,
            initial_energy: 10.0,
            initial_actor_capacity: 8,
        }
    }

    /// Basic metabolism: actor consumes available chemical, gains energy,
    /// loses basal decay. Chemical write buffer decreases accordingly.
    #[test]
    fn metabolism_basic_energy_balance() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 1, energy: 10.0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let config = default_config(); // rate=2.0, factor=1.5, decay=0.5
        let chemical_read = vec![0.0, 5.0, 0.0, 0.0]; // 5.0 at cell 1
        let mut chemical_write = chemical_read.clone();
        let mut removal_buffer = Vec::new();

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &config, &mut removal_buffer,
        ).unwrap();

        // consumed = min(2.0, 5.0) = 2.0
        // energy delta = 2.0 * 1.5 - 0.5 = 2.5
        // new energy = 10.0 + 2.5 = 12.5
        let actor = registry.iter().next().unwrap().1;
        assert!((actor.energy - 12.5).abs() < f32::EPSILON);
        // chemical_write[1] = 5.0 - 2.0 = 3.0
        assert!((chemical_write[1] - 3.0).abs() < f32::EPSILON);
        assert!(removal_buffer.is_empty());
    }

    /// When available chemical < consumption_rate, consume only what's there.
    #[test]
    fn metabolism_partial_consumption() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 0, energy: 10.0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let config = ActorConfig {
            consumption_rate: 5.0,
            energy_conversion_factor: 1.0,
            base_energy_decay: 0.0,
            initial_energy: 10.0,
            initial_actor_capacity: 4,
        };
        let chemical_read = vec![1.5, 0.0, 0.0, 0.0]; // only 1.5 available
        let mut chemical_write = chemical_read.clone();
        let mut removal_buffer = Vec::new();

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &config, &mut removal_buffer,
        ).unwrap();

        // consumed = min(5.0, 1.5) = 1.5
        let actor = registry.iter().next().unwrap().1;
        assert!((actor.energy - 11.5).abs() < f32::EPSILON);
        assert!(chemical_write[0] < f32::EPSILON); // clamped to 0.0
    }

    /// Actor with insufficient energy after metabolism is marked for removal.
    #[test]
    fn metabolism_dead_actor_pushed_to_removal() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        // Low energy actor — decay will kill it.
        let actor = Actor { cell_index: 0, energy: 0.1 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let config = ActorConfig {
            consumption_rate: 1.0,
            energy_conversion_factor: 0.0, // no energy from consumption
            base_energy_decay: 1.0,        // heavy decay
            initial_energy: 10.0,
            initial_actor_capacity: 4,
        };
        let chemical_read = vec![0.0; 4]; // nothing to eat
        let mut chemical_write = chemical_read.clone();
        let mut removal_buffer = Vec::new();

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &config, &mut removal_buffer,
        ).unwrap();

        // energy = 0.1 + 0.0 * 0.0 - 1.0 = -0.9 → dead
        assert_eq!(removal_buffer.len(), 1);
    }

    /// NaN energy triggers TickError::NumericalError.
    #[test]
    fn metabolism_nan_energy_returns_error() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 0, energy: f32::NAN };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let config = default_config();
        let chemical_read = vec![1.0; 4];
        let mut chemical_write = chemical_read.clone();
        let mut removal_buffer = Vec::new();

        let result = run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &config, &mut removal_buffer,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            TickError::NumericalError { system, field, .. } => {
                assert_eq!(system, "actor_metabolism");
                assert_eq!(field, "energy");
            }
        }
    }
}
