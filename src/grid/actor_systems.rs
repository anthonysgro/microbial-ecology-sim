//! Actor system functions: sensing, metabolism, movement, deferred removal.
//!
//! All functions are free (stateless), matching the existing pattern
//! (`run_emission`, `run_diffusion`, `run_heat`). Each operates on
//! borrowed slices and registry references — no owned state, no
//! dynamic dispatch, no heap allocation.

use crate::grid::actor::{ActorId, ActorRegistry, HeritableTraits};
use crate::grid::actor_config::ActorConfig;
use crate::grid::brain::{brain_empty, brain_write, genome_hash, Brain, MemoryEntry, MemoryOutcome};
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

/// Number of heritable traits used in genetic distance computation.
const TRAIT_COUNT: usize = 13;

/// Compute normalized Euclidean distance between two heritable trait vectors.
///
/// Each of the 9 traits is normalized to [0, 1] using its configured clamp bounds:
///   `normalized = (value - min) / (max - min)`
/// If `max == min` for a trait, both actors contribute 0.0 for that dimension.
///
/// Returns `euclidean_distance / sqrt(TRAIT_COUNT)`, guaranteed in [0.0, 1.0].
///
/// Pure function. No side effects. No heap allocation.
#[inline]
pub(crate) fn genetic_distance(a: &HeritableTraits, b: &HeritableTraits, config: &ActorConfig) -> f32 {
    // Trait values paired with their (min, max) clamp bounds.
    let traits: [(f32, f32, f32, f32); TRAIT_COUNT] = [
        (a.consumption_rate,        b.consumption_rate,        config.trait_consumption_rate_min,        config.trait_consumption_rate_max),
        (a.base_energy_decay,       b.base_energy_decay,       config.trait_base_energy_decay_min,       config.trait_base_energy_decay_max),
        (a.levy_exponent,           b.levy_exponent,           config.trait_levy_exponent_min,           config.trait_levy_exponent_max),
        (a.reproduction_threshold,  b.reproduction_threshold,  config.trait_reproduction_threshold_min,  config.trait_reproduction_threshold_max),
        (a.max_tumble_steps as f32, b.max_tumble_steps as f32, config.trait_max_tumble_steps_min as f32, config.trait_max_tumble_steps_max as f32),
        (a.reproduction_cost,       b.reproduction_cost,       config.trait_reproduction_cost_min,       config.trait_reproduction_cost_max),
        (a.offspring_energy,        b.offspring_energy,        config.trait_offspring_energy_min,        config.trait_offspring_energy_max),
        (a.mutation_rate,           b.mutation_rate,           config.trait_mutation_rate_min,           config.trait_mutation_rate_max),
        (a.kin_tolerance,           b.kin_tolerance,           config.trait_kin_tolerance_min,           config.trait_kin_tolerance_max),
        (a.kin_group_defense,       b.kin_group_defense,       config.trait_kin_group_defense_min,       config.trait_kin_group_defense_max),
        (a.optimal_temp,            b.optimal_temp,            config.trait_optimal_temp_min,            config.trait_optimal_temp_max),
        (a.reproduction_cooldown as f32, b.reproduction_cooldown as f32, config.trait_reproduction_cooldown_min as f32, config.trait_reproduction_cooldown_max as f32),
        (a.memory_capacity as f32, b.memory_capacity as f32, config.trait_memory_capacity_min as f32, config.trait_memory_capacity_max as f32),
    ];

    let mut sum_sq: f32 = 0.0;
    for (val_a, val_b, min, max) in traits {
        let range = max - min;
        if range == 0.0 {
            // Zero-range trait: both actors contribute 0.0 difference.
            continue;
        }
        let norm_a = (val_a - min) / range;
        let norm_b = (val_b - min) / range;
        let diff = norm_a - norm_b;
        sum_sq += diff * diff;
    }

    (sum_sq.sqrt()) / (TRAIT_COUNT as f32).sqrt()
}

/// Compute the thermal fitness factor for an actor.
///
/// Returns a value in [0.0, 1.0]:
///   - 1.0 when cell_heat == optimal_temp (zero mismatch)
///   - Decays toward 0.0 as |cell_heat - optimal_temp| increases
///   - Exactly 1.0 when width == 0.0 (mechanic disabled)
///
/// Formula: exp(-mismatch² / (2 * width²))
///
/// HOT PATH: No allocation, no branching beyond the width==0 guard.
/// Deterministic for identical inputs.
#[inline]
pub(crate) fn thermal_fitness(cell_heat: f32, optimal_temp: f32, width: f32) -> f32 {
    if width == 0.0 {
        return 1.0;
    }
    let delta = cell_heat - optimal_temp;
    (-delta * delta / (2.0 * width * width)).exp()
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
        // With metabolic scaling, effective_conversion = (ecf - ec) * metabolic_ratio, so
        // break_even = base_energy_decay / effective_conversion simplifies to
        // reference_metabolic_rate / (ecf - ec), independent of individual actor metabolic rate.
        // Precondition: energy_conversion_factor > extraction_cost (enforced by config validation).
        let break_even = config.reference_metabolic_rate / (config.energy_conversion_factor - config.extraction_cost);

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
    heat_read: &[f32],
    config: &ActorConfig,
    removal_buffer: &mut Vec<ActorId>,
    brains: &mut [Brain],
    tick: u64,
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
            let metabolic_ratio =
                actor.traits.base_energy_decay / config.reference_metabolic_rate;
            let effective_conversion =
                (config.energy_conversion_factor - config.extraction_cost) * metabolic_ratio;

            let available = chemical_read[ci];
            let headroom = (config.max_energy - actor.energy).max(0.0);
            let max_useful = headroom / effective_conversion;
            let consumed = actor.traits.consumption_rate.min(available).min(max_useful);

            chemical_write[ci] -= consumed;
            if chemical_write[ci] < 0.0 {
                chemical_write[ci] = 0.0;
            }

            let fitness = thermal_fitness(
                heat_read[ci],
                actor.traits.optimal_temp,
                config.thermal_fitness_width,
            );

            let delta = heat_read[ci] - actor.traits.optimal_temp;
            let thermal_cost = config.thermal_sensitivity * delta * delta;

            // Reproductive readiness cost: continuous metabolic drain for maintaining
            // reproductive machinery. Scales with investment and inversely with cooldown.
            let reproductive_investment =
                actor.traits.reproduction_cost + actor.traits.offspring_energy;
            let cooldown_factor =
                1.0 / (actor.traits.reproduction_cooldown.max(1) as f32);
            let readiness_cost = config.readiness_sensitivity * reproductive_investment
                * cooldown_factor
                / config.reference_cooldown;

            // Cognitive cost: per-tick energy drain proportional to memory capacity.
            let cognitive_cost =
                config.cognitive_cost_per_slot * actor.traits.memory_capacity as f32;

            actor.energy += consumed * effective_conversion * fitness
                - actor.traits.base_energy_decay
                - thermal_cost
                - readiness_cost
                - cognitive_cost;

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

            // Write food memory entry when the actor consumed chemical.
            if consumed > 0.0 {
                let slot_index = id.index;
                let entry = MemoryEntry {
                    tick,
                    cell_index: ci as u32,
                    genome_hash: 0,
                    outcome: MemoryOutcome::Food,
                };
                brain_write(
                    &mut brains[slot_index],
                    entry,
                    actor.traits.memory_capacity,
                );
            }

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
    heat_read: &[f32],
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

        // Deduct energy-proportional movement cost after successful move,
        // scaled inversely by metabolic ratio so high-metabolism actors move cheaper.
        // Thermal fitness degrades movement efficiency: cost is divided by capped fitness.
        let metabolic_ratio = actor.traits.base_energy_decay / actor_config.reference_metabolic_rate;
        let fitness = thermal_fitness(
            heat_read[target],
            actor.traits.optimal_temp,
            actor_config.thermal_fitness_width,
        );
        let capped_fitness = fitness.max(1.0 / actor_config.thermal_movement_cap);
        let proportional = base * (actor.energy / reference) / metabolic_ratio / capped_fitness;
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
        // Cooldown gate: if still on cooldown, decrement and skip.
        if actor.cooldown_remaining > 0 {
            actor.cooldown_remaining -= 1;
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

        // Energy conservation: deduct both the entropy cost (reproduction_cost)
        // and the energy transferred to the offspring (offspring_energy).
        // Invariant: parent_before = parent_after + reproduction_cost + offspring_energy
        actor.energy -= actor.traits.reproduction_cost + actor.traits.offspring_energy;

        // NaN/Inf check on parent energy after deduction.
        if actor.energy.is_nan() || actor.energy.is_infinite() {
            return Err(TickError::NumericalError {
                system: "actor_reproduction",
                cell_index: actor.cell_index,
                field: "energy",
                value: actor.energy,
            });
        }

        // Set cooldown on parent after successful fission.
        actor.cooldown_remaining = actor.traits.reproduction_cooldown;

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
    brains: &mut Vec<Brain>,
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
            cooldown_remaining: 0,
        };
        let id = actors.add(offspring, cell_count, occupancy).map_err(|e| {
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

        // Initialize offspring brain: empty buffer, no inherited memories.
        // If the slot was reused from the free list, overwrite the stale brain.
        // If a new slot was appended, grow the brains Vec to match.
        let slot = id.index;
        if slot >= brains.len() {
            brains.resize_with(slot + 1, brain_empty);
        }
        brains[slot] = brain_empty();
    }
    spawn_buffer.clear();
    Ok(())
}

// WARM PATH: Executes once per tick over the actor list.
// No heap allocation (SmallVec inline, removal_buffer pre-allocated).
// No dynamic dispatch.

/// Execute contact predation for all active actors in deterministic order.
///
/// Two-pass approach to avoid mutable aliasing:
/// - Pass 1 (read-only): iterate actors by ascending slot index, scan
///   4-neighborhood, collect predation events into a stack-local SmallVec.
/// - Pass 2 (mutate): apply events — add energy to predator, mark prey
///   inert, queue prey for deferred removal.
///
/// Each actor participates in at most one predation event per tick
/// (either as predator or as prey). Determinism is guaranteed by
/// ascending slot-index iteration order and first-match-wins semantics.
pub fn run_contact_predation(
    actors: &mut ActorRegistry,
    occupancy: &[Option<usize>],
    config: &ActorConfig,
    removal_buffer: &mut Vec<ActorId>,
    w: usize,
    h: usize,
    rng: &mut impl Rng,
    brains: &mut [Brain],
    tick: u64,
) -> Result<usize, TickError> {
    use smallvec::SmallVec;

    // (predator_slot, prey_slot, energy_gain)
    let mut events: SmallVec<[(usize, usize, f32); 64]> = SmallVec::new();

    // Track which slots have already been claimed as predator or prey
    // this tick. Indexed by slot index. Reuses stack for small registries.
    let slot_count = actors.slot_count();
    let mut participated: SmallVec<[bool; 256]> = SmallVec::new();
    participated.resize(slot_count, false);

    // Pass 1: collect predation events (read-only iteration).
    for (slot_idx, actor) in actors.iter() {
        if actor.inert || participated[slot_idx] {
            continue;
        }

        for dir in 0..4u8 {
            let neighbor_cell = match direction_to_target(actor.cell_index, dir, w, h) {
                Some(c) => c,
                None => continue,
            };

            let neighbor_slot = match occupancy[neighbor_cell] {
                Some(s) => s,
                None => continue,
            };

            if participated[neighbor_slot] {
                continue;
            }

            let neighbor = match actors.get_by_slot(neighbor_slot) {
                Some(a) => a,
                None => continue,
            };

            if neighbor.inert {
                continue;
            }

            // Energy dominance: this actor must have strictly more energy.
            if actor.energy <= neighbor.energy {
                continue;
            }

            // Kin recognition: genetic distance must meet predator's threshold.
            let dist = genetic_distance(&actor.traits, &neighbor.traits, config);
            if dist < actor.traits.kin_tolerance {
                continue;
            }

            // Group defense: prey's allied neighbors reduce predation success.
            // ally_defense_sum ∈ [0.0, 3.0], success_probability ∈ [0.25, 1.0].
            let ally_defense_sum = sum_allied_defense(
                neighbor.cell_index,
                &neighbor.traits,
                slot_idx,
                occupancy,
                actors,
                config,
                w,
                h,
            );
            let success_probability = 1.0 / (1.0 + ally_defense_sum);
            let roll: f32 = rng.random::<f32>();
            if roll >= success_probability {
                // Predation failed — predator participated but prey remains eligible.
                participated[slot_idx] = true;
                break;
            }

            // Predation succeeds — record event.
            let metabolic_ratio =
                actor.traits.base_energy_decay / config.reference_metabolic_rate;
            let effective_absorption =
                (config.absorption_efficiency * metabolic_ratio).min(1.0);
            let gained = neighbor.energy * effective_absorption;
            events.push((slot_idx, neighbor_slot, gained));
            participated[slot_idx] = true;
            participated[neighbor_slot] = true;
            break; // one predation per predator per tick
        }
    }

    // Pass 2: apply predation events.
    for &(predator_slot, prey_slot, gained) in &events {
        // Apply energy gain to predator, clamped to max_energy.
        if let Some(predator) = actors.get_mut_by_slot(predator_slot) {
            predator.energy = (predator.energy + gained).min(config.max_energy);

            if predator.energy.is_nan() || predator.energy.is_infinite() {
                return Err(TickError::NumericalError {
                    system: "contact_predation",
                    cell_index: predator.cell_index,
                    field: "energy",
                    value: predator.energy,
                });
            }
        }

        // Mark prey inert and queue for removal.
        if let Some(prey) = actors.get_mut_by_slot(prey_slot) {
            prey.inert = true;
        }
        if let Some(prey_id) = actors.actor_id_for_slot(prey_slot) {
            removal_buffer.push(prey_id);
        }
    }

    // Pass 3: write memory entries for both predator and prey.
    // Separate pass because brain_write needs the actor's traits (for genome_hash)
    // and cell_index, which requires immutable borrows that conflict with pass 2's
    // mutable borrows.
    for &(predator_slot, prey_slot, _) in &events {
        let predator_ci = actors.get_by_slot(predator_slot).map(|a| a.cell_index as u32);
        let predator_cap = actors.get_by_slot(predator_slot).map(|a| a.traits.memory_capacity);
        let predator_traits = actors.get_by_slot(predator_slot).map(|a| a.traits);
        let prey_ci = actors.get_by_slot(prey_slot).map(|a| a.cell_index as u32);
        let prey_cap = actors.get_by_slot(prey_slot).map(|a| a.traits.memory_capacity);
        let prey_traits = actors.get_by_slot(prey_slot).map(|a| a.traits);

        // Predator remembers successful hunt.
        if let (Some(ci), Some(cap), Some(prey_t)) = (predator_ci, predator_cap, prey_traits) {
            brain_write(
                &mut brains[predator_slot],
                MemoryEntry {
                    tick,
                    cell_index: ci,
                    genome_hash: genome_hash(&prey_t),
                    outcome: MemoryOutcome::PredationSuccess,
                },
                cap,
            );
        }

        // Prey remembers threat (still in registry, marked inert but not yet removed).
        if let (Some(ci), Some(cap), Some(pred_t)) = (prey_ci, prey_cap, predator_traits) {
            brain_write(
                &mut brains[prey_slot],
                MemoryEntry {
                    tick,
                    cell_index: ci,
                    genome_hash: genome_hash(&pred_t),
                    outcome: MemoryOutcome::PredationThreat,
                },
                cap,
            );
        }
    }

    Ok(events.len())
}
/// Sum the `kin_group_defense` trait values of non-inert actors in the prey's
/// Von Neumann 4-neighborhood (excluding the predator) whose genetic distance
/// to the prey is below the prey's `kin_tolerance`.
///
/// Returns the sum as f32. Maximum possible value is 3.0 (3 allies × max 1.0 each).
/// Pure function. No heap allocation. Stack-only.
///
/// WARM PATH: Called once per predation attempt. At most 3 occupancy lookups +
/// 3 genetic distance computations per call.
fn sum_allied_defense(
    prey_cell: usize,
    prey_traits: &HeritableTraits,
    predator_slot: usize,
    occupancy: &[Option<usize>],
    actors: &ActorRegistry,
    config: &ActorConfig,
    w: usize,
    h: usize,
) -> f32 {
    let mut defense_sum: f32 = 0.0;
    for dir in 0..4u8 {
        let neighbor_cell = match direction_to_target(prey_cell, dir, w, h) {
            Some(c) => c,
            None => continue,
        };
        let neighbor_slot = match occupancy[neighbor_cell] {
            Some(s) => s,
            None => continue,
        };
        if neighbor_slot == predator_slot {
            continue;
        }
        let neighbor = match actors.get_by_slot(neighbor_slot) {
            Some(a) => a,
            None => continue,
        };
        if neighbor.inert {
            continue;
        }
        let dist = genetic_distance(&neighbor.traits, prey_traits, config);
        if dist < prey_traits.kin_tolerance {
            defense_sum += neighbor.traits.kin_group_defense;
        }
    }
    defense_sum
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::actor::{Actor, ActorRegistry, HeritableTraits};
    use crate::grid::actor_config::ActorConfig;
    use crate::grid::brain::brain_empty;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// 3×3 grid, actor at center (1,1). Highest concentration at North (0,1).
    /// Expected: movement target = North cell index.
    #[test]
    fn sensing_selects_max_gradient_neighbor() {
        let mut occupancy = vec![None; 9];
        let mut registry = ActorRegistry::with_capacity(4);
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()), cooldown_remaining: 0 };
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
        let actor = Actor { cell_index: 0, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()), cooldown_remaining: 0 };
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
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()), cooldown_remaining: 0 };
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
        let actor = Actor { cell_index: 4, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()), cooldown_remaining: 0 };
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
            // Set reference_metabolic_rate == base_energy_decay so metabolic_ratio = 1.0,
            // preserving pre-scaling expected values in existing tests.
            reference_metabolic_rate: 0.5,
            // Disable readiness cost so these tests focus on consumption/decay/thermal.
            readiness_sensitivity: 0.0,
            // Disable thermal fitness so existing tests preserve pre-feature behavior.
            // When width == 0.0, thermal_fitness() returns 1.0 unconditionally.
            thermal_fitness_width: 0.0,
            // Disable cognitive cost so existing tests preserve pre-brain behavior.
            cognitive_cost_per_slot: 0.0,
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
        let actor = Actor { cell_index: 1, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config), cooldown_remaining: 0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let chemical_read = vec![0.0, 5.0, 0.0, 0.0]; // 5.0 at cell 1
        let mut chemical_write = chemical_read.clone();
        let heat_read = vec![config.optimal_temp; 4]; // match optimal_temp → zero thermal cost
        let mut removal_buffer = Vec::new();
        let mut brains = vec![brain_empty(); registry.slot_count()];

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &heat_read, &config, &mut removal_buffer,
            &mut brains, 0,
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
            base_energy_decay: 0.05,
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
            // Match reference to decay so metabolic_ratio = 1.0.
            reference_metabolic_rate: 0.05,
            // Disable readiness cost so this test focuses on partial consumption.
            readiness_sensitivity: 0.0,
            // Disable cognitive cost so this test focuses on partial consumption.
            cognitive_cost_per_slot: 0.0,
            ..ActorConfig::default()
        };
        let actor = Actor { cell_index: 0, energy: 10.0, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config), cooldown_remaining: 0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let chemical_read = vec![1.5, 0.0, 0.0, 0.0]; // only 1.5 available
        let mut chemical_write = chemical_read.clone();
        let heat_read = vec![config.optimal_temp; 4]; // match optimal_temp → zero thermal cost
        let mut removal_buffer = Vec::new();
        let mut brains = vec![brain_empty(); registry.slot_count()];

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &heat_read, &config, &mut removal_buffer,
            &mut brains, 0,
        ).unwrap();

        // metabolic_ratio = 0.05 / 0.05 = 1.0
        // consumed = min(5.0, 1.5) = 1.5
        // energy = 10.0 + 1.5 * 1.0 - 0.05 = 11.45
        let actor = registry.iter().next().unwrap().1;
        assert!((actor.energy - 11.45).abs() < 1e-5);
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
            // Disable cognitive cost so this test focuses on decay-driven inertness.
            cognitive_cost_per_slot: 0.0,
            ..ActorConfig::default()
        };
        // Low energy actor — decay will push it to zero.
        let actor = Actor { cell_index: 0, energy: 0.1, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&config), cooldown_remaining: 0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();
        let chemical_read = vec![0.0; 4]; // nothing to eat
        let mut chemical_write = chemical_read.clone();
        let heat_read = vec![config.optimal_temp; 4]; // match optimal_temp → zero thermal cost
        let mut removal_buffer = Vec::new();
        let mut brains = vec![brain_empty(); registry.slot_count()];

        run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &heat_read, &config, &mut removal_buffer,
            &mut brains, 0,
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
        let actor = Actor { cell_index: 0, energy: f32::NAN, inert: false, tumble_direction: 0, tumble_remaining: 0, traits: HeritableTraits::from_config(&default_config()), cooldown_remaining: 0 };
        let _id = registry.add(actor, 4, &mut occupancy).unwrap();

        let config = default_config();
        let chemical_read = vec![1.0; 4];
        let mut chemical_write = chemical_read.clone();
        let heat_read = vec![config.optimal_temp; 4]; // match optimal_temp → zero thermal cost
        let mut removal_buffer = Vec::new();
        let mut brains = vec![brain_empty(); registry.slot_count()];

        let result = run_actor_metabolism(
            &mut registry, &chemical_read, &mut chemical_write,
            &heat_read, &config, &mut removal_buffer,
            &mut brains, 0,
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
