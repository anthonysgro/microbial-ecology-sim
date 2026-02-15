//! Actor system functions: sensing, metabolism, movement, deferred removal.
//!
//! All functions are free (stateless), matching the existing pattern
//! (`run_emission`, `run_diffusion`, `run_heat`). Each operates on
//! borrowed slices and registry references — no owned state, no
//! dynamic dispatch, no heap allocation.

use crate::grid::actor::{ActorId, ActorRegistry};
use crate::grid::actor_config::ActorConfig;
use crate::grid::error::TickError;
use rand::Rng;

/// Sample a step count from the discrete power-law distribution.
/// `P(steps = k) ∝ k^(-α)`, clamped to `[1, max_steps]`.
/// Uses inverse transform sampling: `steps = floor(u^(-1/(α-1)))`, `u ~ Uniform(0,1)`.
///
/// # Preconditions
/// - `alpha > 1.0` (enforced by config validation)
/// - `max_steps >= 1` (enforced by config validation)
pub(crate) fn sample_tumble_steps(rng: &mut impl Rng, alpha: f32, max_steps: u16) -> u16 {
    let u: f32 = rng.random_range(0.0_f32..1.0_f32);
    // Floor to EPSILON to avoid 0.0^negative → infinity.
    let u = u.max(f32::EPSILON);
    let exponent = -1.0 / (alpha - 1.0);
    let raw = u.powf(exponent).floor() as u32;
    raw.clamp(1, max_steps as u32) as u16
}

/// Convert a tumble direction (0=N, 1=S, 2=W, 3=E) to a target cell index.
/// Returns `None` if the direction would go out of bounds.
pub(crate) fn direction_to_target(cell_index: usize, direction: u8, w: usize, h: usize) -> Option<usize> {
    let x = cell_index % w;
    let y = cell_index / w;
    match direction {
        0 if y > 0 => Some((y - 1) * w + x),      // North
        1 if y + 1 < h => Some((y + 1) * w + x),   // South
        2 if x > 0 => Some(y * w + (x - 1)),        // West
        3 if x + 1 < w => Some(y * w + (x + 1)),    // East
        _ => None,                                    // Out of bounds or invalid direction
    }
}

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
///   Compute movement targets for all active Actors based on local
///   chemical gradients (Von Neumann neighborhood, species 0) and
///   Lévy flight tumble state.
///
/// For each Actor in deterministic slot-index order:
/// 1. Compute the metabolic break-even concentration from `config`.
/// 2. Check Von Neumann neighbors for concentrations above break-even
///    with a positive gradient relative to the current cell.
/// 3. If a profitable gradient exists: follow it, reset tumble state.
/// 4. If no gradient and actor is mid-tumble (`tumble_remaining > 0`):
///    continue in `tumble_direction`, decrement remaining.
/// 5. If no gradient and not tumbling: sample a new tumble direction
///    and step count from the power-law distribution.
/// 6. Boundary hits (direction_to_target returns None) reset the tumble.
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry (tumble state updated in-place).
/// * `chemical_read` — read buffer for chemical species 0, length = cell_count.
/// * `grid_width` — number of columns in the grid.
/// * `grid_height` — number of rows in the grid.
/// * `movement_targets` — pre-allocated buffer indexed by slot index.
///   Entries for inactive slots are left untouched.
/// * `config` — actor configuration (break-even params, Lévy flight params).
/// * `rng` — per-tick deterministic RNG for tumble sampling.
pub fn run_actor_sensing(
    actors: &mut ActorRegistry,
    chemical_read: &[f32],
    grid_width: u32,
    grid_height: u32,
    movement_targets: &mut [Option<usize>],
    config: &ActorConfig,
    rng: &mut impl Rng,
) {
    let w = grid_width as usize;
    let h = grid_height as usize;

    for (slot_index, actor) in actors.iter_mut() {
        if actor.inert {
            movement_targets[slot_index] = None;
            continue;
        }
        let ci = actor.cell_index;
        let x = ci % w;
        let y = ci / w;
        let current_val = chemical_read[ci];

        // Break-even concentration: below this, consumption costs more energy than it yields.
        // Per-actor: uses the actor's heritable base_energy_decay trait.
        // Precondition: energy_conversion_factor > extraction_cost (enforced by config validation).
        let break_even = actor.traits.base_energy_decay / (config.energy_conversion_factor - config.extraction_cost);

        // Scan Von Neumann neighbors for the best above-threshold positive gradient.
        // Direction priority: N, S, W, E (first wins ties via strict `>`).
        let mut best_gradient: f32 = 0.0;
        let mut best_target: Option<usize> = None;

        // North (y - 1)
        if y > 0 {
            let ni = (y - 1) * w + x;
            let nval = chemical_read[ni];
            if nval > break_even {
                let gradient = nval - current_val;
                if gradient > best_gradient {
                    best_gradient = gradient;
                    best_target = Some(ni);
                }
            }
        }

        // South (y + 1)
        if y + 1 < h {
            let ni = (y + 1) * w + x;
            let nval = chemical_read[ni];
            if nval > break_even {
                let gradient = nval - current_val;
                if gradient > best_gradient {
                    best_gradient = gradient;
                    best_target = Some(ni);
                }
            }
        }

        // West (x - 1)
        if x > 0 {
            let ni = y * w + (x - 1);
            let nval = chemical_read[ni];
            if nval > break_even {
                let gradient = nval - current_val;
                if gradient > best_gradient {
                    best_gradient = gradient;
                    best_target = Some(ni);
                }
            }
        }

        // East (x + 1)
        if x + 1 < w {
            let ni = y * w + (x + 1);
            let nval = chemical_read[ni];
            if nval > break_even {
                let gradient = nval - current_val;
                if gradient > best_gradient {
                    best_target = Some(ni);
                }
            }
        }

        if best_target.is_some() {
            // Gradient found — follow it, cancel any active tumble.
            actor.tumble_remaining = 0;
            movement_targets[slot_index] = best_target;
        } else if actor.tumble_remaining > 0 {
            // Mid-tumble, no gradient — continue in tumble_direction.
            let target = direction_to_target(ci, actor.tumble_direction, w, h);
            if target.is_none() {
                // Hit grid boundary — end tumble.
                actor.tumble_remaining = 0;
            } else {
                actor.tumble_remaining -= 1;
            }
            movement_targets[slot_index] = target;
        } else {
            // No gradient, not tumbling — initiate new Lévy flight tumble.
            actor.tumble_direction = rng.random_range(0u8..4u8);
            actor.tumble_remaining = sample_tumble_steps(rng, actor.traits.levy_exponent, actor.traits.max_tumble_steps);
            let target = direction_to_target(ci, actor.tumble_direction, w, h);
            if target.is_none() {
                // Facing a boundary — end tumble immediately.
                actor.tumble_remaining = 0;
            } else {
                actor.tumble_remaining -= 1;
            }
            movement_targets[slot_index] = target;
        }
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

        if actor.inert {
            // Inert actors: no chemical consumption, only basal decay.
            actor.energy -= actor.traits.base_energy_decay;

            if actor.energy.is_nan() || actor.energy.is_infinite() {
                return Err(TickError::NumericalError {
                    system: "actor_metabolism",
                    cell_index: ci,
                    field: "energy",
                    value: actor.energy,
                });
            }

            // Schedule removal when energy falls to or below removal_threshold.
            if actor.energy <= config.removal_threshold {
                removal_buffer.push(id);
            }
        } else {
            // Active actors: demand-driven consumption and energy balance.
            let available = chemical_read[ci];
            let headroom = (config.max_energy - actor.energy).max(0.0);
            let max_useful = headroom / (config.energy_conversion_factor - config.extraction_cost);
            let consumed = actor.traits.consumption_rate.min(available).min(max_useful);

            chemical_write[ci] -= consumed;
            if chemical_write[ci] < 0.0 {
                chemical_write[ci] = 0.0;
            }

            actor.energy += consumed * (config.energy_conversion_factor - config.extraction_cost) - actor.traits.base_energy_decay;

            if actor.energy.is_nan() || actor.energy.is_infinite() {
                return Err(TickError::NumericalError {
                    system: "actor_metabolism",
                    cell_index: ci,
                    field: "energy",
                    value: actor.energy,
                });
            }

            // Safety clamp: floating-point arithmetic may marginally exceed max_energy.
            // Placed after NaN/Inf check because f32::min swallows NaN.
            actor.energy = actor.energy.min(config.max_energy);

            // Transition to inert instead of immediate removal.
            if actor.energy <= 0.0 {
                actor.inert = true;
            }
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
    actor_config: &ActorConfig,
) -> Result<(), TickError> {
    let base = actor_config.base_movement_cost;
    let reference = actor_config.reference_energy;
    let floor = base * 0.1;

    for (slot_index, actor) in actors.iter_mut() {
        // Inert actors do not move.
        if actor.inert {
            continue;
        }

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

        // Deduct energy-proportional movement cost after successful move.
        let proportional = base * (actor.energy / reference);
        let actual = if proportional > floor { proportional } else { floor };
        actor.energy -= actual;

        if actor.energy.is_nan() || actor.energy.is_infinite() {
            return Err(TickError::NumericalError {
                system: "actor_movement",
                cell_index: actor.cell_index,
                field: "energy",
                value: actor.energy,
            });
        }

        // Movement-induced energy depletion → inert transition.
        if actor.energy <= 0.0 {
            actor.inert = true;
        }
    }

    Ok(())
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

// WARM PATH: Executes once per tick over the actor list.
// No heap allocation (spawn_buffer pre-allocated). No dynamic dispatch.

/// Execute the reproduction phase for all active Actors (binary fission).
///
/// For each Actor in deterministic slot-index order:
/// 1. Skip if inert or energy < reproduction_threshold.
/// 2. Scan Von Neumann neighbors (N, S, W, E) for the first unoccupied cell
///    that is also not already claimed in the spawn buffer.
/// 3. Deduct reproduction_cost from parent energy.
/// 4. Push (target_cell, offspring_energy) to spawn_buffer.
///
/// The spawn buffer is cleared at the start. Offspring are not inserted into
/// the registry until `run_deferred_spawn` processes the buffer.
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry (parent energy updated in-place).
/// * `occupancy` — read-only occupancy map, length = cell_count.
/// * `config` — actor configuration (reproduction_threshold, reproduction_cost, offspring_energy).
/// * `spawn_buffer` — pre-allocated buffer for deferred spawn requests. Cleared before use.
/// * `w` — grid width in cells.
/// * `h` — grid height in cells.
pub fn run_actor_reproduction(
    actors: &mut ActorRegistry,
    occupancy: &[Option<usize>],
    _config: &ActorConfig,
    spawn_buffer: &mut Vec<(usize, f32, crate::grid::actor::HeritableTraits)>,
    w: usize,
    h: usize,
) -> Result<(), TickError> {
    spawn_buffer.clear();

    for (_id, actor) in actors.iter_mut_with_ids() {
        // Skip inert actors regardless of energy.
        if actor.inert {
            continue;
        }
        // Skip actors below per-actor reproduction threshold.
        if actor.energy < actor.traits.reproduction_threshold {
            continue;
        }
        // Energy conservation: the parent must have enough energy to cover
        // both the fission cost and the offspring's starting energy. Without
        // this, independent mutation of reproduction_cost and offspring_energy
        // can create net-positive energy reproduction (energy printing press).
        if actor.energy < actor.traits.reproduction_cost + actor.traits.offspring_energy {
            continue;
        }

        // Scan N(0), S(1), W(2), E(3) for the first available cell.
        let mut target_cell: Option<usize> = None;
        for dir in 0..4u8 {
            if let Some(candidate) = direction_to_target(actor.cell_index, dir, w, h) {
                // Check occupancy map (read-only snapshot from before reproduction).
                if occupancy[candidate].is_some() {
                    continue;
                }
                // Check spawn buffer for collisions with earlier spawns this tick.
                if spawn_buffer.iter().any(|&(cell, _, _)| cell == candidate) {
                    continue;
                }
                target_cell = Some(candidate);
                break;
            }
        }

        let Some(cell) = target_cell else {
            // All neighbors occupied or out of bounds — reproduction blocked.
            continue;
        };

        // Deduct reproduction cost from parent.
        actor.energy -= actor.traits.reproduction_cost;

        // NaN/Inf check on parent energy after deduction.
        if actor.energy.is_nan() || actor.energy.is_infinite() {
            return Err(TickError::NumericalError {
                system: "actor_reproduction",
                cell_index: actor.cell_index,
                field: "energy",
                value: actor.energy,
            });
        }

        spawn_buffer.push((cell, actor.traits.offspring_energy, actor.traits));
    }

    Ok(())
}

/// Process the spawn buffer: insert offspring Actors into the registry
/// and update the occupancy map.
///
/// Iterates the spawn buffer in insertion order (deterministic — matches
/// the slot-index order from `run_actor_reproduction`). Each offspring is
/// constructed with a clean initial state: not inert, no tumble.
/// Parent traits are cloned and mutated with a deterministic per-offspring
/// RNG derived from `seed`, `tick`, and spawn buffer index.
///
/// # Arguments
///
/// * `actors` — mutable reference to the actor registry.
/// * `occupancy` — mutable occupancy map, length = cell_count.
/// * `spawn_buffer` — buffer of (cell_index, energy, parent_traits) tuples. Cleared after processing.
/// * `cell_count` — total number of grid cells (for bounds validation).
/// * `config` — actor configuration (mutation parameters and clamp ranges).
/// * `seed` — simulation master seed for deterministic mutation RNG derivation.
/// * `tick` — current tick number for deterministic mutation RNG derivation.
pub fn run_deferred_spawn(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    spawn_buffer: &mut Vec<(usize, f32, crate::grid::actor::HeritableTraits)>,
    cell_count: usize,
    config: &ActorConfig,
    seed: u64,
    tick: u64,
) -> Result<(), TickError> {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    for (i, &(cell_index, energy, parent_traits)) in spawn_buffer.iter().enumerate() {
        let mut offspring_traits = parent_traits;
        // Deterministic per-offspring seed: combines master seed, tick, and
        // spawn buffer index. The wrapping_mul constant is the LCG multiplier
        // from Knuth's MMIX, providing good bit mixing.
        let offspring_seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(tick)
            .wrapping_add(i as u64);
        let mut mutation_rng = ChaCha8Rng::seed_from_u64(offspring_seed);
        offspring_traits.mutate(config, &mut mutation_rng);

        let offspring = crate::grid::actor::Actor {
            cell_index,
            energy,
            inert: false,
            tumble_direction: 0,
            tumble_remaining: 0,
            traits: offspring_traits,
        };
        actors.add(offspring, cell_count, occupancy).map_err(|e| {
            TickError::NumericalError {
                system: "actor_deferred_spawn",
                cell_index,
                field: "occupancy",
                value: match e {
                    crate::grid::actor::ActorError::CellOccupied { .. } => f32::NAN,
                    crate::grid::actor::ActorError::CellOutOfBounds { .. } => f32::INFINITY,
                    crate::grid::actor::ActorError::InvalidActorId { .. } => f32::NAN,
                },
            }
        })?;
    }
    spawn_buffer.clear();
    Ok(())
}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::actor::{Actor, ActorRegistry, HeritableTraits};
    use crate::grid::actor_config::ActorConfig;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// 3×3 grid, actor at center (1,1). Highest concentration at North (0,1).
    /// Expected: movement target = North cell index.
    #[test]
    fn sensing_selects_max_gradient_neighbor() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()) };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        let config = default_config();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Chemical buffer: center=1.0, north=5.0, south=2.0, west=3.0, east=4.0
        // break_even = 0.5 / (1.5 - 0.0) = 0.333 — all neighbors above threshold.
        let mut chemical = vec![0.0; 9];
        chemical[4] = 1.0; // center
        chemical[1] = 5.0; // north — highest gradient (5.0 - 1.0 = 4.0)
        chemical[7] = 2.0; // south
        chemical[3] = 3.0; // west
        chemical[5] = 4.0; // east

        let mut targets = vec![None; 1];
        run_actor_sensing(&mut registry, &chemical, 3, 3, &mut targets, &config, &mut rng);

        assert_eq!(targets[0], Some(1), "should select north (index 1) as max gradient");
    }

    /// Actor at corner (0,0) on a 3×3 grid. Only South and East are in-bounds.
    /// Out-of-bounds neighbors not evaluated. In-bounds neighbors above break-even
    /// with positive gradient are selected.
    #[test]
    fn sensing_boundary_cell_treats_oob_as_zero() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 0, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()) };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        let config = default_config();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Center (0,0) = 2.0, South (0,1) = index 3 = 5.0, East (1,0) = index 1 = 3.0
        let mut chemical = vec![0.0; 9];
        chemical[0] = 2.0;
        chemical[3] = 5.0; // south
        chemical[1] = 3.0; // east

        let mut targets = vec![None; 1];
        run_actor_sensing(&mut registry, &chemical, 3, 3, &mut targets, &config, &mut rng);

        assert_eq!(targets[0], Some(3), "should select south (index 3) as max gradient");
    }

    /// No neighbor has concentration above break-even with positive gradient.
    /// Actor should enter tumble mode and receive a movement target from the
    /// sampled tumble direction.
    #[test]
    fn sensing_no_positive_gradient_initiates_tumble() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()) };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        let config = default_config();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Center has the highest value — all gradients are negative.
        // All neighbors below break-even (0.333) except center.
        let mut chemical = vec![0.0; 9];
        chemical[4] = 10.0;
        // Neighbors at 0.0 — below break-even, no positive gradient.

        let mut targets = vec![None; 1];
        run_actor_sensing(&mut registry, &chemical, 3, 3, &mut targets, &config, &mut rng);

        // Actor at center of 3×3 grid — all four directions are in-bounds,
        // so tumble should produce Some(target).
        assert!(targets[0].is_some(), "tumble should produce a movement target");
        // Verify tumble state was set on the actor.
        let actor = registry.iter().next().unwrap().1;
        assert!(actor.tumble_direction < 4, "tumble direction must be 0..4");
    }

    /// Tie-breaking: North and East have equal gradient. North is checked
    /// first in iteration order (N, S, W, E), so North wins.
    #[test]
    fn sensing_tie_breaks_by_direction_priority() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()) };
        let _id = registry.add(actor, 9, &mut occupancy).unwrap();

        let config = default_config();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let mut chemical = vec![0.0; 9];
        chemical[4] = 1.0; // center
        chemical[1] = 5.0; // north — gradient 4.0
        chemical[5] = 5.0; // east  — gradient 4.0 (tie)

        let mut targets = vec![None; 1];
        run_actor_sensing(&mut registry, &chemical, 3, 3, &mut targets, &config, &mut rng);

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
            max_energy: 1000.0,
            initial_actor_capacity: 8,
            base_movement_cost: 0.5,
            reference_energy: 25.0,
            removal_threshold: -5.0,
            extraction_cost: 0.0,
            levy_exponent: 1.5,
            max_tumble_steps: 20,
            reproduction_threshold: 20.0,
            reproduction_cost: 12.0,
            offspring_energy: 10.0,
            ..ActorConfig::default()
        }
    }

    /// Basic metabolism: actor consumes available chemical, gains energy,
    /// loses basal decay. Chemical write buffer decreases accordingly.
    #[test]
    fn metabolism_basic_energy_balance() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        let config = default_config(); // rate=2.0, factor=1.5, decay=0.5
        let actor = Actor { cell_index: 1, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config) };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

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

        let config = ActorConfig {
            consumption_rate: 5.0,
            energy_conversion_factor: 1.0,
            base_energy_decay: 0.0,
            initial_energy: 10.0,
            max_energy: 1000.0,
            initial_actor_capacity: 4,
            base_movement_cost: 0.5,
            reference_energy: 25.0,
            removal_threshold: -5.0,
            extraction_cost: 0.0,
            levy_exponent: 1.5,
            max_tumble_steps: 20,
            reproduction_threshold: 20.0,
            reproduction_cost: 12.0,
            offspring_energy: 10.0,
            ..ActorConfig::default()
        };
        let actor = Actor { cell_index: 0, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config) };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

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

    /// Actor with insufficient energy after metabolism becomes inert (not removed).
    #[test]
    fn metabolism_dead_actor_becomes_inert() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);

        let config = ActorConfig {
            consumption_rate: 1.0,
            energy_conversion_factor: 0.0, // no energy from consumption
            base_energy_decay: 1.0,        // heavy decay
            initial_energy: 10.0,
            max_energy: 1000.0,
            initial_actor_capacity: 4,
            base_movement_cost: 0.5,
            reference_energy: 25.0,
            removal_threshold: -5.0,
            extraction_cost: 0.0,
            levy_exponent: 1.5,
            max_tumble_steps: 20,
            reproduction_threshold: 20.0,
            reproduction_cost: 12.0,
            offspring_energy: 10.0,
            ..ActorConfig::default()
        };
        // Low energy actor — decay will push it to zero.
        let actor = Actor { cell_index: 0, energy: 0.1, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config) };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();
        let chemical_read = vec![0.0; 4]; // nothing to eat
        let mut chemical_write = chemical_read.clone();
        let mut removal_buffer = Vec::new();

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &config, &mut removal_buffer,
        ).unwrap();

        // energy = 0.1 + 0.0 * 0.0 - 1.0 = -0.9 → inert, not removed
        let actor = registry.iter().next().unwrap().1;
        assert!(actor.inert);
        assert!(removal_buffer.is_empty());
    }

    /// NaN energy triggers TickError::NumericalError.
    #[test]
    fn metabolism_nan_energy_returns_error() {
        let mut occupancy = vec![None; 4];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 0, energy: f32::NAN, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()) };
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
