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
use crate::grid::source::{run_emission, SourceField};
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
fn run_emission_phase(grid: &mut Grid, _config: &GridConfig) -> Result<(), TickError> {
    if grid.sources().is_empty() {
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
    run_emission(grid, &mut registry);

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
fn run_actor_phases(grid: &mut Grid, _config: &GridConfig) -> Result<(), TickError> {
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
    {
        let chemical_read = grid
            .read_chemical(0)
            .expect("at least one chemical species required for actor sensing");
        run_actor_sensing(
            &actors,
            chemical_read,
            grid.width(),
            grid.height(),
            &mut movement_targets,
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
    pub fn step(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError> {
        // Phase 0: Emission (WARM) — inject source values before downstream systems
        run_emission_phase(grid, config)?;

        // Phases 1–4: Actor phases (WARM) — sensing, metabolism, removal, movement.
        // Skip entirely when no actors are registered (zero overhead).
        //
        // Requirements: 4.1 (phase ordering), 4.2 (read/write discipline),
        //               4.3 (swap before diffusion), 4.4 (deterministic), 4.5 (zero-actor skip)
        if !grid.actors().is_empty() {
            run_actor_phases(grid, config)?;
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
