// COLD PATH: Runs once at startup for procedural world generation.
// Allocations and dynamic dispatch permitted.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::grid::Grid;
use crate::grid::actor::{Actor, ActorError};
use crate::grid::actor_config::ActorConfig;
use crate::grid::config::{CellDefaults, GridConfig};
use crate::grid::error::GridError;
use crate::grid::source::{Source, SourceError, SourceField};

/// Ranges and constraints for procedural world generation.
/// All ranges are inclusive: [min, max].
#[derive(Debug, Clone, PartialEq)]
pub struct WorldInitConfig {
    /// Range for number of heat sources to place.
    pub min_heat_sources: u32,
    pub max_heat_sources: u32,

    /// Range for number of chemical sources per species.
    pub min_chemical_sources: u32,
    pub max_chemical_sources: u32,

    /// Range for source emission rates (applies to both heat and chemical).
    pub min_emission_rate: f32,
    pub max_emission_rate: f32,

    /// Range for initial per-cell heat values.
    pub min_initial_heat: f32,
    pub max_initial_heat: f32,

    /// Range for initial per-cell chemical concentrations (per species).
    pub min_initial_concentration: f32,
    pub max_initial_concentration: f32,

    /// Range for number of actors to seed at initialization.
    /// Set both to 0 to skip actor seeding.
    pub min_actors: u32,
    pub max_actors: u32,
}

impl Default for WorldInitConfig {
    fn default() -> Self {
        Self {
            min_heat_sources: 1,
            max_heat_sources: 5,
            min_chemical_sources: 1,
            max_chemical_sources: 3,
            min_emission_rate: 0.1,
            max_emission_rate: 5.0,
            min_initial_heat: 0.0,
            max_initial_heat: 1.0,
            min_initial_concentration: 0.0,
            max_initial_concentration: 0.5,
            min_actors: 0,
            max_actors: 0,
        }
    }
}

/// Domain error type for world initialization failures.
#[derive(Debug, thiserror::Error)]
pub enum WorldInitError {
    #[error("invalid range: {field} min ({min}) > max ({max})")]
    InvalidRange {
        field: &'static str,
        min: f64,
        max: f64,
    },

    #[error("grid construction failed: {0}")]
    GridError(#[from] GridError),

    #[error("source registration failed: {0}")]
    SourceError(#[from] SourceError),

    #[error("actor registration failed: {0}")]
    ActorError(#[from] ActorError),
}

/// Validate all `WorldInitConfig` ranges. Returns first error found.
pub(crate) fn validate_config(config: &WorldInitConfig) -> Result<(), WorldInitError> {
    if config.min_heat_sources > config.max_heat_sources {
        return Err(WorldInitError::InvalidRange {
            field: "heat_sources",
            min: f64::from(config.min_heat_sources),
            max: f64::from(config.max_heat_sources),
        });
    }
    if config.min_chemical_sources > config.max_chemical_sources {
        return Err(WorldInitError::InvalidRange {
            field: "chemical_sources",
            min: f64::from(config.min_chemical_sources),
            max: f64::from(config.max_chemical_sources),
        });
    }
    if config.min_emission_rate > config.max_emission_rate {
        return Err(WorldInitError::InvalidRange {
            field: "emission_rate",
            min: f64::from(config.min_emission_rate),
            max: f64::from(config.max_emission_rate),
        });
    }
    if config.min_initial_heat > config.max_initial_heat {
        return Err(WorldInitError::InvalidRange {
            field: "initial_heat",
            min: f64::from(config.min_initial_heat),
            max: f64::from(config.max_initial_heat),
        });
    }
    if config.min_initial_concentration > config.max_initial_concentration {
        return Err(WorldInitError::InvalidRange {
            field: "initial_concentration",
            min: f64::from(config.min_initial_concentration),
            max: f64::from(config.max_initial_concentration),
        });
    }
    if config.min_actors > config.max_actors {
        return Err(WorldInitError::InvalidRange {
            field: "actors",
            min: f64::from(config.min_actors),
            max: f64::from(config.max_actors),
        });
    }
    Ok(())
}
/// Generate and register heat and chemical sources into the grid.
///
/// Samples source counts from the configured ranges, then for each source
/// samples a cell position uniformly from `[0, cell_count)` and an emission
/// rate from `[min_emission_rate, max_emission_rate]`. Registers each source
/// via `Grid::add_source`, propagating any `SourceError`.
pub(crate) fn generate_sources(
    grid: &mut Grid,
    rng: &mut impl Rng,
    config: &WorldInitConfig,
    num_chemicals: usize,
) -> Result<(), WorldInitError> {
    let cell_count = grid.cell_count();

    // Heat sources
    let heat_count = rng.random_range(config.min_heat_sources..=config.max_heat_sources);
    for _ in 0..heat_count {
        let cell_index = rng.random_range(0..cell_count);
        let emission_rate = rng.random_range(config.min_emission_rate..=config.max_emission_rate);
        grid.add_source(Source {
            cell_index,
            field: SourceField::Heat,
            emission_rate,
        })?;
    }

    // Chemical sources: one batch per species
    for species in 0..num_chemicals {
        let chem_count = rng.random_range(config.min_chemical_sources..=config.max_chemical_sources);
        for _ in 0..chem_count {
            let cell_index = rng.random_range(0..cell_count);
            let emission_rate = rng.random_range(config.min_emission_rate..=config.max_emission_rate);
            grid.add_source(Source {
                cell_index,
                field: SourceField::Chemical(species),
                emission_rate,
            })?;
        }
    }

    Ok(())
}

/// Generate and register initial actors into the grid.
///
/// Samples actor count from `[min_actors, max_actors]`, then for each actor
/// picks a random unoccupied cell. If a chosen cell is already occupied,
/// it retries up to `cell_count` times before giving up on that actor.
/// Each actor is spawned with `initial_energy` from the `ActorConfig`.
///
/// Skips entirely if `max_actors == 0` or no `ActorConfig` is present.
pub(crate) fn generate_actors(
    grid: &mut Grid,
    rng: &mut impl Rng,
    config: &WorldInitConfig,
) -> Result<(), WorldInitError> {
    let actor_config = match grid.actor_config() {
        Some(ac) => ac.clone(),
        None => return Ok(()),
    };

    if config.max_actors == 0 {
        return Ok(());
    }

    let cell_count = grid.cell_count();
    let actor_count = rng.random_range(config.min_actors..=config.max_actors) as usize;

    for _ in 0..actor_count {
        // Try to find an unoccupied cell. Bounded retries to avoid
        // infinite loops on nearly-full grids.
        let mut placed = false;
        for _ in 0..cell_count {
            let cell_index = rng.random_range(0..cell_count);
            let actor = Actor {
                cell_index,
                energy: actor_config.initial_energy,
            };
            match grid.add_actor(actor) {
                Ok(_) => {
                    placed = true;
                    break;
                }
                Err(ActorError::CellOccupied { .. }) => continue,
                Err(e) => return Err(e.into()),
            }
        }
        // If the grid is too full to place this actor, stop trying.
        if !placed {
            break;
        }
    }

    Ok(())
}

/// Write seeded initial values into grid field buffers.
///
/// Writes to the write buffers, then swaps so seeded values land in the
/// read buffers. This avoids needing mutable access to read buffers.
///
/// Infallible: all indices are derived from grid dimensions, so no
/// bounds errors are possible.
pub(crate) fn populate_fields(
    grid: &mut Grid,
    rng: &mut impl Rng,
    config: &WorldInitConfig,
    num_chemicals: usize,
) {
    let cell_count = grid.cell_count();

    // Heat: sample per-cell values into the write buffer, then swap.
    {
        let heat_write = grid.write_heat();
        for i in 0..cell_count {
            heat_write[i] = rng.random_range(config.min_initial_heat..=config.max_initial_heat);
        }
    }
    grid.swap_heat();

    // Chemicals: for each species, sample per-cell concentrations into
    // the write buffer, then swap all chemical buffers at the end.
    for species in 0..num_chemicals {
        // Species index is always valid since num_chemicals comes from the grid.
        let chem_write = grid
            .write_chemical(species)
            .expect("species index derived from grid; always valid");
        for i in 0..cell_count {
            chem_write[i] = rng.random_range(
                config.min_initial_concentration..=config.max_initial_concentration,
            );
        }
    }
    grid.swap_chemicals();
}
/// Generate a fully initialized Grid from a seed and configuration.
///
/// COLD PATH: Runs once at startup. Allocations permitted.
///
/// Creates a master `ChaCha8Rng` from the seed, then forks into independent
/// child RNGs for source generation and field population. This isolation
/// ensures that adding new generation phases won't retroactively change
/// earlier outputs for the same seed.
///
/// Returns a Grid ready for immediate tick execution.
pub fn initialize(
    seed: u64,
    grid_config: GridConfig,
    init_config: &WorldInitConfig,
    actor_config: Option<ActorConfig>,
) -> Result<Grid, WorldInitError> {
    validate_config(init_config)?;

    let num_chemicals = grid_config.num_chemicals;

    // Zero defaults — populate_fields overwrites all cells after construction.
    let defaults = CellDefaults {
        chemical_concentrations: vec![0.0; num_chemicals],
        heat: 0.0,
    };

    let mut grid = Grid::new(grid_config, defaults, actor_config)?;

    // Deterministic RNG forking: each phase draws from an independent stream,
    // so changes to one phase cannot perturb the other.
    let mut master_rng = ChaCha8Rng::seed_from_u64(seed);
    let mut source_rng = ChaCha8Rng::from_rng(&mut master_rng);
    let mut field_rng = ChaCha8Rng::from_rng(&mut master_rng);
    let mut actor_rng = ChaCha8Rng::from_rng(&mut master_rng);

    generate_sources(&mut grid, &mut source_rng, init_config, num_chemicals)?;
    populate_fields(&mut grid, &mut field_rng, init_config, num_chemicals);
    generate_actors(&mut grid, &mut actor_rng, init_config)?;

    Ok(grid)
}

