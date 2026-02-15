// HOT PATH: Executes per tick, orchestrating all environmental systems.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

use crate::grid::actor_systems::{
    run_actor_metabolism, run_actor_movement, run_actor_sensing, run_deferred_removal,
};
use crate::grid::config::GridConfig;
use crate::grid::decay::run_decay;
use crate::grid::diffusion::run_diffusion;
use crate::grid::error::TickError;
use crate::grid::heat::run_heat;
use crate::grid::source::{run_emission, run_respawn_phase, RespawnEntry, SourceField};
use crate::grid::world_init::SourceFieldConfig;
use crate::grid::Grid;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

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
/// 0. Emission  → copy read→write, inject, clamp chemicals, validate, swap (WARM)
/// 1. Diffusion  → validate chemical write buffers → swap chemicals
/// 2. Heat       → validate heat write buffer      → swap heat
///
/// If validation fails, the tick halts immediately. The read buffer
/// retains the last valid state for diagnostics.
///
/// # Requirements
/// 8.1 — emission phase before all other systems
/// 9.1 — sequential system execution order
/// 9.2 — buffer swap between systems
/// 9.4 — NaN/infinity detection before swap
pub struct TickOrchestrator;

// WARM PATH: Emission phase — runs once per tick over the source list.
// Allocation: one temporary take/put of the SourceRegistry (no heap alloc,
// just a pointer swap via mem::replace). No dynamic dispatch.

/// Execute the emission phase: inject source values into field write buffers.
///
/// For each field type that has active sources:
/// 1. Copy read buffer → write buffer (so emission adds to current state)
/// 2. Run emission (additive injection from all sources)
/// 3. Clamp chemical write buffers to ≥ 0.0
/// 4. Validate write buffers for NaN/infinity
/// 5. Swap affected field buffers
///
/// No-op if the source registry is empty.
///
/// # Requirements
/// 8.1 — emission before downstream systems
/// 8.2 — copy read→write before emission
/// 8.3 — swap after emission so downstream reads post-emission state
/// 9.1 — NaN/infinity validation
/// 9.2 — clamp chemical concentrations to ≥ 0.0
// WARM PATH: Emission phase — runs once per tick over the source list.
// Allocation: one temporary take/put of the SourceRegistry (no heap alloc,
// just a pointer swap via mem::replace). No dynamic dispatch.

/// Execute the emission phase: inject source values into field write buffers,
/// process depletion events, and run the respawn phase.
///
/// For each field type that has active sources:
/// 1. Copy read buffer → write buffer (so emission adds to current state)
/// 2. Run emission (additive injection from all sources)
/// 3. Process depletion events: remove depleted slots, enqueue respawns if enabled
/// 4. Clamp chemical write buffers to ≥ 0.0
/// 5. Validate write buffers for NaN/infinity
/// 6. Swap affected field buffers
/// 7. Run respawn phase: spawn replacements for mature queue entries
///
/// No-op if the source registry is empty and the respawn queue is empty.
///
/// # Requirements
/// 2.1, 2.2, 2.4 — cooldown sampling and queue management
/// 7.1, 7.2, 7.3 — respawn phase after emission, new sources emit next tick
/// 8.1 — depleted slot removal after respawn entry queued
fn run_emission_phase(
    grid: &mut Grid,
    _config: &GridConfig,
    current_tick: u64,
    heat_config: &SourceFieldConfig,
    chemical_config: &SourceFieldConfig,
    rng: &mut ChaCha8Rng,
) -> Result<(), TickError> {
    if grid.sources().is_empty() && grid.respawn_queue().is_empty() {
        return Ok(());
    }

    // Scan sources to determine which field types are affected.
    let mut heat_affected = false;
    // Track affected chemical species. For small num_chemicals (1–4),
    // a fixed-size array avoids allocation.
    let num_chemicals = grid.num_chemicals();
    let mut chem_affected = [false; 16]; // supports up to 16 species without alloc

    for source in grid.sources().iter() {
        match source.field {
            SourceField::Heat => heat_affected = true,
            SourceField::Chemical(species) => {
                if species < chem_affected.len() {
                    chem_affected[species] = true;
                }
            }
        }
    }

    // Temporarily extract the registry to split the borrow:
    // run_emission needs &mut Grid (for write buffers) + &SourceRegistry.
    let mut registry = grid.take_sources();

    // Copy read→write for affected fields, then run emission.
    if heat_affected {
        grid.heat_buffer_mut().copy_read_to_write();
    }
    for species in 0..num_chemicals {
        if species < chem_affected.len() && chem_affected[species] {
            if let Some(buf) = grid.chemical_buffer_mut(species) {
                buf.copy_read_to_write();
            }
        }
    }

    // Inject emission values into write buffers, draining reservoirs.
    let depletions = run_emission(grid, &mut registry, current_tick);

    // Process depletion events: remove depleted slots, enqueue respawns.
    for event in &depletions {
        let field_config = match event.field {
            SourceField::Heat => heat_config,
            SourceField::Chemical(_) => chemical_config,
        };
        if field_config.respawn_enabled {
            let cooldown = rng.random_range(
                field_config.min_respawn_cooldown_ticks..=field_config.max_respawn_cooldown_ticks,
            );
            grid.respawn_queue_mut().push(RespawnEntry {
                field: event.field,
                respawn_tick: current_tick + u64::from(cooldown),
            });
        }
        // Remove depleted source from registry — slot freed for reuse.
        // Defensive .ok(): should not fail if depletion detection is correct.
        registry.remove(event.source_id).ok();
    }

    // Clamp chemical write buffers to ≥ 0.0 (concentrations cannot be negative).
    for species in 0..num_chemicals {
        if species < chem_affected.len() && chem_affected[species] {
            if let Ok(write_buf) = grid.write_chemical(species) {
                for val in write_buf.iter_mut() {
                    if *val < 0.0 {
                        *val = 0.0;
                    }
                }
            }
        }
    }

    // Validate affected write buffers for NaN/infinity.
    if heat_affected {
        validate_buffer(grid.write_heat(), "emission", "heat")?;
    }
    for species in 0..num_chemicals {
        if species < chem_affected.len() && chem_affected[species] {
            if let Ok(write_buf) = grid.write_chemical(species) {
                let field_name = match species {
                    0 => "chemical_0",
                    1 => "chemical_1",
                    2 => "chemical_2",
                    3 => "chemical_3",
                    _ => "chemical_N",
                };
                validate_buffer(write_buf, "emission", field_name)?;
            }
        }
    }

    // Swap affected field buffers so downstream systems read post-emission state.
    if heat_affected {
        grid.swap_heat();
    }
    for species in 0..num_chemicals {
        if species < chem_affected.len() && chem_affected[species] {
            if let Some(buf) = grid.chemical_buffer_mut(species) {
                buf.swap();
            }
        }
    }

    // Return the registry to the grid.
    grid.put_sources(registry);

    // Respawn phase: spawn replacements for mature entries.
    // Executes after emission so depletion events from this tick are captured.
    // Newly spawned sources begin emitting on the next tick (Req 7.2).
    run_respawn_phase(
        grid,
        rng,
        current_tick,
        heat_config,
        chemical_config,
        num_chemicals,
    );

    Ok(())
}

// WARM PATH: Actor phases — runs once per tick over the actor list.
// Allocation: one temporary take/put of actor data (pointer swaps via
// mem::replace/mem::take). No dynamic dispatch.

/// Execute all actor phases: sensing, metabolism, deferred removal, movement.
///
/// Borrow-splitting strategy: `take_actors` extracts the ActorRegistry,
/// occupancy map, removal buffer, and movement targets so that actor
/// system functions can operate on them while the Grid retains ownership
/// of field buffers.
///
/// Buffer discipline:
/// - Sensing reads from chemical read buffer (species 0).
/// - Metabolism copies chemical read→write, then subtracts consumption
///   from the write buffer.
/// - After actor phases complete, chemical write buffers are validated
///   and swapped so diffusion reads post-consumption state.
///
/// # Requirements
/// 4.1 — phase ordering: sensing → metabolism → removal → movement
/// 4.2 — read from read buffers, write to write buffers
/// 4.3 — swap chemical buffers after actor consumption, before diffusion
/// 4.4 — deterministic slot-index order
fn run_actor_phases(grid: &mut Grid, _config: &GridConfig, tick: u64) -> Result<(), TickError> {
    let actor_config = grid
        .actor_config()
        .expect("actor_config must be set when actors are registered")
        .clone();

    // Extract actor data to split borrows with field buffers.
    let (mut actors, mut occupancy, mut removal_buffer, mut movement_targets) =
        grid.take_actors();

    // Ensure movement_targets covers all registry slots.
    let slot_count = actors.slot_count();
    if movement_targets.len() < slot_count {
        movement_targets.resize(slot_count, None);
    }

    // Phase 1: Sensing (WARM) — read chemical gradients, compute movement targets.
    // Reads from chemical species 0 read buffer only.
    // Per-tick RNG seeded deterministically from grid seed + tick number.
    // Requirements 6.1, 6.2: deterministic replay from seed + tick.
    let mut tick_rng = ChaCha8Rng::seed_from_u64(grid.seed().wrapping_add(tick));
    {
        let chemical_read = grid
            .read_chemical(0)
            .expect("at least one chemical species required for actor sensing");
        run_actor_sensing(
            &mut actors,
            chemical_read,
            grid.width(),
            grid.height(),
            &mut movement_targets,
            &actor_config,
            &mut tick_rng,
        );
    }

    // Phase 2: Metabolism (WARM) — consume chemicals, update energy, mark dead.
    // Copy read→write before metabolism so consumption subtracts from current state.
    {
        if let Some(buf) = grid.chemical_buffer_mut(0) {
            buf.copy_read_to_write();
        }
        let (chemical_read, chemical_write) = grid
            .read_write_chemical(0)
            .expect("at least one chemical species required for actor metabolism");
        run_actor_metabolism(
            &mut actors,
            chemical_read,
            chemical_write,
            &actor_config,
            &mut removal_buffer,
        )?;
    }

    // Phase 3: Deferred removal — remove dead actors after metabolism iteration.
    if !removal_buffer.is_empty() {
        // ActorError from removal is a logic bug (stale id in the buffer we just
        // built), so convert to a TickError for uniform error propagation.
        run_deferred_removal(&mut actors, &mut occupancy, &mut removal_buffer)
            .map_err(|_| TickError::NumericalError {
                system: "actor_deferred_removal",
                cell_index: 0,
                field: "actor_id",
                value: f32::NAN,
            })?;
    }

    // Phase 4: Movement (WARM) — relocate actors toward sensed gradients.
    run_actor_movement(
        &mut actors,
        &mut occupancy,
        &movement_targets,
        actor_config.movement_cost,
    )?;

    // Return actor data to the grid.
    grid.put_actors(actors, occupancy, removal_buffer, movement_targets);

    // Validate chemical write buffer after actor consumption (NaN/Inf check).
    {
        let write_buf = grid
            .write_chemical(0)
            .expect("at least one chemical species required");
        validate_buffer(write_buf, "actor_metabolism", "chemical_0")?;
    }

    // Swap chemical buffers so diffusion reads post-consumption state.
    grid.swap_chemicals();

    Ok(())
}

impl TickOrchestrator {
    /// Advance the simulation by one tick.
    ///
    /// Requires `SourceFieldConfig` references for the emission/respawn phases.
    /// These control depletion event processing (cooldown sampling) and
    /// replacement source parameter sampling.
    pub fn step(
        grid: &mut Grid,
        config: &GridConfig,
        tick: u64,
        heat_source_config: &SourceFieldConfig,
        chemical_source_config: &SourceFieldConfig,
    ) -> Result<(), TickError> {
        // Per-tick RNG for emission-phase cooldown sampling and respawn-phase
        // source parameter sampling. Seeded deterministically from grid seed +
        // tick, offset by a domain constant to avoid correlation with the
        // actor-phase RNG (which uses seed + tick directly).
        let mut emission_rng =
            ChaCha8Rng::seed_from_u64(grid.seed().wrapping_add(tick).wrapping_add(0xDEAD_BEEF));

        // Phase 0: Emission (WARM) — inject source values, detect depletions
        // Phase 0.5: Respawn (WARM) — process mature queue entries, spawn replacements
        run_emission_phase(
            grid,
            config,
            tick,
            heat_source_config,
            chemical_source_config,
            &mut emission_rng,
        )?;

        // Phases 1–4: Actor phases (WARM) — sensing, metabolism, removal, movement.
        // Skip entirely when no actors are registered (zero overhead).
        //
        // Requirements: 4.1 (phase ordering), 4.2 (read/write discipline),
        //               4.3 (swap before diffusion), 4.4 (deterministic), 4.5 (zero-actor skip)
        if !grid.actors().is_empty() {
            run_actor_phases(grid, config, tick)?;
        }

        // Phase 5: Chemical diffusion
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

        // Phase 6: Chemical decay (HOT) — apply per-species decay after diffusion
        run_decay(grid, config)?;
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
            validate_buffer(write_buf, "decay", field_name)?;
        }
        grid.swap_chemicals();

        // Phase 7: Heat radiation
        run_heat(grid, config)?;
        validate_buffer(grid.write_heat(), "heat", "heat")?;
        grid.swap_heat();

        Ok(())
    }
}
