// COLD PATH: Runs once at startup for procedural world generation.
// Allocations and dynamic dispatch permitted.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

use crate::grid::Grid;
use crate::grid::actor::{Actor, ActorError, HeritableTraits};
use crate::grid::actor_config::ActorConfig;
use crate::grid::config::{CellDefaults, GridConfig};
use crate::grid::error::GridError;
use crate::grid::source::{ClusterCenter, RespawnQueue, Source, SourceError, SourceField};
use smallvec::SmallVec;

/// Per-field-type configuration for source generation.
/// Reusable for any fundamental (heat, chemical, future types).
/// All ranges are inclusive: [min, max].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceFieldConfig {
    /// Range for number of sources to place.
    pub min_sources: u32,
    pub max_sources: u32,
    /// Range for source emission rates (units per tick).
    pub min_emission_rate: f32,
    pub max_emission_rate: f32,
    /// Fraction of sources that are renewable. [0.0, 1.0].
    pub renewable_fraction: f32,
    /// Range for initial reservoir capacity of non-renewable sources.
    pub min_reservoir_capacity: f32,
    pub max_reservoir_capacity: f32,
    /// Range for deceleration threshold of non-renewable sources. [0.0, 1.0].
    pub min_deceleration_threshold: f32,
    pub max_deceleration_threshold: f32,
    /// Whether depleted sources of this field type trigger respawns.
    /// Default: false (backward compatible).
    pub respawn_enabled: bool,
    /// Minimum cooldown ticks before a depleted source respawns.
    pub min_respawn_cooldown_ticks: u32,
    /// Maximum cooldown ticks before a depleted source respawns.
    pub max_respawn_cooldown_ticks: u32,
    /// Spatial clustering of sources. 0.0 = uniform random, 1.0 = tight clusters.
    /// Range: [0.0, 1.0]. Default: 0.0.
    pub source_clustering: f32,
    /// Inter-cluster dispersion. Controls how many distinct cluster centers
    /// sources are distributed across. [0.0, 1.0]. Default: 0.0.
    /// 0.0 = one shared center (current behavior).
    /// 1.0 = one center per source.
    /// Formula: num_clusters = max(1, round(source_dispersion * num_sources)).
    pub source_dispersion: f32,
}

/// Per-species chemical configuration bundle.
/// Groups source generation parameters, decay rate, and diffusion rate
/// for a single chemical species.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChemicalSpeciesConfig {
    /// Source generation parameters for this species.
    pub source_config: SourceFieldConfig,
    /// Exponential decay rate per tick. [0.0, 1.0].
    /// Applied as concentration *= (1.0 - decay_rate).
    pub decay_rate: f32,
    /// Diffusion coefficient (discrete Laplacian scaling).
    /// Must be non-negative and finite.
    /// Stability: diffusion_rate * tick_duration * 8 < 1.0.
    pub diffusion_rate: f32,
}

impl Default for ChemicalSpeciesConfig {
    fn default() -> Self {
        Self {
            source_config: SourceFieldConfig {
                max_sources: 3,
                ..SourceFieldConfig::default()
            },
            decay_rate: 0.05,
            diffusion_rate: 0.05,
        }
    }
}

/// Ranges and constraints for procedural world generation.
/// All ranges are inclusive: [min, max].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldInitConfig {
    /// All heat source generation parameters.
    pub heat_source_config: SourceFieldConfig,
    /// Per-species chemical configuration bundles.
    pub chemical_species_configs: Vec<ChemicalSpeciesConfig>,

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

impl Default for SourceFieldConfig {
    fn default() -> Self {
        Self {
            min_sources: 1,
            max_sources: 5,
            min_emission_rate: 0.1,
            max_emission_rate: 5.0,
            renewable_fraction: 0.3,
            min_reservoir_capacity: 50.0,
            max_reservoir_capacity: 200.0,
            min_deceleration_threshold: 0.1,
            max_deceleration_threshold: 0.5,
            respawn_enabled: false,
            min_respawn_cooldown_ticks: 50,
            max_respawn_cooldown_ticks: 150,
            source_clustering: 0.0,
            source_dispersion: 0.0,
        }
    }
}

impl Default for WorldInitConfig {
    fn default() -> Self {
        Self {
            heat_source_config: SourceFieldConfig::default(),
            chemical_species_configs: vec![
                ChemicalSpeciesConfig::default(),
                ChemicalSpeciesConfig::default(),
            ],
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

    #[error("invalid config: {reason}")]
    InvalidConfig { reason: &'static str },

    #[error("chemical species {species}: {source}")]
    ChemicalSpeciesConfigError {
        species: usize,
        source: Box<WorldInitError>,
    },

    #[error("chemical species {species}: decay_rate ({value}) must be in [0.0, 1.0]")]
    InvalidDecayRate { species: usize, value: f32 },

    #[error("chemical species {species}: diffusion_rate ({value}) must be non-negative and finite")]
    InvalidDiffusionRate { species: usize, value: f32 },

    #[error("grid construction failed: {0}")]
    GridError(#[from] GridError),

    #[error("source registration failed: {0}")]
    SourceError(#[from] SourceError),

    #[error("actor registration failed: {0}")]
    ActorError(#[from] ActorError),
}

/// Static label set for a single field type, used by `validate_source_field_config`
/// to produce `&'static str` error fields without allocation.
struct SourceFieldLabels {
    sources: &'static str,
    emission_rate: &'static str,
    renewable_fraction: &'static str,
    min_reservoir_capacity: &'static str,
    reservoir_capacity: &'static str,
    min_deceleration_threshold: &'static str,
    max_deceleration_threshold: &'static str,
    deceleration_threshold: &'static str,
    respawn_cooldown: &'static str,
    respawn_zero_cooldown: &'static str,
    source_clustering_range: &'static str,
    source_clustering_finite: &'static str,
    source_dispersion_range: &'static str,
    source_dispersion_finite: &'static str,
}

const HEAT_LABELS: SourceFieldLabels = SourceFieldLabels {
    sources: "heat_sources",
    emission_rate: "heat_emission_rate",
    renewable_fraction: "heat renewable_fraction must be in [0.0, 1.0]",
    min_reservoir_capacity: "heat min_reservoir_capacity must be > 0.0",
    reservoir_capacity: "heat_reservoir_capacity",
    min_deceleration_threshold: "heat min_deceleration_threshold must be in [0.0, 1.0]",
    max_deceleration_threshold: "heat max_deceleration_threshold must be in [0.0, 1.0]",
    deceleration_threshold: "heat_deceleration_threshold",
    respawn_cooldown: "heat_respawn_cooldown_ticks",
    respawn_zero_cooldown: "heat max_respawn_cooldown_ticks must be > 0 when respawn is enabled",
    source_clustering_range: "heat source_clustering must be in [0.0, 1.0]",
    source_clustering_finite: "heat source_clustering must be finite",
    source_dispersion_range: "heat source_dispersion must be in [0.0, 1.0]",
    source_dispersion_finite: "heat source_dispersion must be finite",
};

const CHEMICAL_LABELS: SourceFieldLabels = SourceFieldLabels {
    sources: "chemical_sources",
    emission_rate: "chemical_emission_rate",
    renewable_fraction: "chemical renewable_fraction must be in [0.0, 1.0]",
    min_reservoir_capacity: "chemical min_reservoir_capacity must be > 0.0",
    reservoir_capacity: "chemical_reservoir_capacity",
    min_deceleration_threshold: "chemical min_deceleration_threshold must be in [0.0, 1.0]",
    max_deceleration_threshold: "chemical max_deceleration_threshold must be in [0.0, 1.0]",
    deceleration_threshold: "chemical_deceleration_threshold",
    respawn_cooldown: "chemical_respawn_cooldown_ticks",
    respawn_zero_cooldown: "chemical max_respawn_cooldown_ticks must be > 0 when respawn is enabled",
    source_clustering_range: "chemical source_clustering must be in [0.0, 1.0]",
    source_clustering_finite: "chemical source_clustering must be finite",
    source_dispersion_range: "chemical source_dispersion must be in [0.0, 1.0]",
    source_dispersion_finite: "chemical source_dispersion must be finite",
};

/// Validate a single `SourceFieldConfig`. All error messages are prefixed
/// with the field type via the pre-baked `labels`.
fn validate_source_field_config(
    config: &SourceFieldConfig,
    labels: &SourceFieldLabels,
) -> Result<(), WorldInitError> {
    if config.min_sources > config.max_sources {
        return Err(WorldInitError::InvalidRange {
            field: labels.sources,
            min: f64::from(config.min_sources),
            max: f64::from(config.max_sources),
        });
    }
    if config.min_emission_rate > config.max_emission_rate {
        return Err(WorldInitError::InvalidRange {
            field: labels.emission_rate,
            min: f64::from(config.min_emission_rate),
            max: f64::from(config.max_emission_rate),
        });
    }
    if !(0.0..=1.0).contains(&config.renewable_fraction) {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.renewable_fraction,
        });
    }
    if config.min_reservoir_capacity <= 0.0 {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.min_reservoir_capacity,
        });
    }
    if config.max_reservoir_capacity < config.min_reservoir_capacity {
        return Err(WorldInitError::InvalidRange {
            field: labels.reservoir_capacity,
            min: f64::from(config.min_reservoir_capacity),
            max: f64::from(config.max_reservoir_capacity),
        });
    }
    if !(0.0..=1.0).contains(&config.min_deceleration_threshold) {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.min_deceleration_threshold,
        });
    }
    if !(0.0..=1.0).contains(&config.max_deceleration_threshold) {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.max_deceleration_threshold,
        });
    }
    if config.max_deceleration_threshold < config.min_deceleration_threshold {
        return Err(WorldInitError::InvalidRange {
            field: labels.deceleration_threshold,
            min: f64::from(config.min_deceleration_threshold),
            max: f64::from(config.max_deceleration_threshold),
        });
    }
    // Respawn cooldown validation: only when respawn is enabled.
    if config.respawn_enabled {
        if config.max_respawn_cooldown_ticks == 0 {
            return Err(WorldInitError::InvalidConfig {
                reason: labels.respawn_zero_cooldown,
            });
        }
        if config.min_respawn_cooldown_ticks > config.max_respawn_cooldown_ticks {
            return Err(WorldInitError::InvalidRange {
                field: labels.respawn_cooldown,
                min: f64::from(config.min_respawn_cooldown_ticks),
                max: f64::from(config.max_respawn_cooldown_ticks),
            });
        }
    }
    // Source clustering validation.
    if !config.source_clustering.is_finite() {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.source_clustering_finite,
        });
    }
    if !(0.0..=1.0).contains(&config.source_clustering) {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.source_clustering_range,
        });
    }
    // Source dispersion validation.
    if !config.source_dispersion.is_finite() {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.source_dispersion_finite,
        });
    }
    if !(0.0..=1.0).contains(&config.source_dispersion) {
        return Err(WorldInitError::InvalidConfig {
            reason: labels.source_dispersion_range,
        });
    }
    Ok(())
}
/// Sample a cell index for a source, clustered around (`center_col`, `center_row`).
///
/// When `source_clustering == 0.0`, returns a uniform random cell index (preserving
/// legacy behavior). Otherwise, samples a 2D normal offset with
/// `sigma = max(width, height) * (1.0 - source_clustering)` and clamps to grid
/// bounds.
///
/// At `source_clustering == 1.0`, sigma drops below 0.5 and all sources land
/// directly on the cluster center.
pub(crate) fn sample_clustered_position(
    rng: &mut impl Rng,
    center_col: u32,
    center_row: u32,
    width: u32,
    height: u32,
    source_clustering: f32,
) -> usize {
    let cell_count = width as usize * height as usize;

    // Fast path: uniform random (legacy behavior).
    if source_clustering == 0.0 {
        return rng.random_range(0..cell_count);
    }

    let max_dim = width.max(height) as f32;
    let sigma = max_dim * (1.0 - source_clustering);

    // When sigma is negligibly small, place directly on center.
    if sigma < 0.5 {
        return center_row as usize * width as usize + center_col as usize;
    }

    // SAFETY of unwrap: sigma >= 0.5 guarantees a valid (positive, finite) std dev.
    let normal = Normal::new(0.0_f32, sigma).expect("sigma >= 0.5 is always valid for Normal");
    let dx = normal.sample(rng).round() as i32;
    let dy = normal.sample(rng).round() as i32;

    // Toroidal wrapping: sources that overshoot the grid wrap around
    // instead of clamping to edges (which causes artificial edge accumulation).
    let col = (center_col as i32 + dx).rem_euclid(width as i32) as usize;
    let row = (center_row as i32 + dy).rem_euclid(height as i32) as usize;

    row * width as usize + col
}

/// Validate all `WorldInitConfig` ranges. Returns first error found.
pub(crate) fn validate_config(config: &WorldInitConfig) -> Result<(), WorldInitError> {
    validate_source_field_config(&config.heat_source_config, &HEAT_LABELS)?;

    // Per-species chemical validation: source_config, decay_rate, diffusion_rate.
    for (i, species_config) in config.chemical_species_configs.iter().enumerate() {
        validate_source_field_config(&species_config.source_config, &CHEMICAL_LABELS)
            .map_err(|e| WorldInitError::ChemicalSpeciesConfigError {
                species: i,
                source: Box::new(e),
            })?;

        if !(0.0..=1.0).contains(&species_config.decay_rate) {
            return Err(WorldInitError::InvalidDecayRate {
                species: i,
                value: species_config.decay_rate,
            });
        }

        if species_config.diffusion_rate < 0.0 || !species_config.diffusion_rate.is_finite() {
            return Err(WorldInitError::InvalidDiffusionRate {
                species: i,
                value: species_config.diffusion_rate,
            });
        }
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
/// samples a cell position (uniform or clustered depending on `source_clustering`)
/// and an emission rate from `[min_emission_rate, max_emission_rate]`. Each source
/// is assigned as renewable or finite based on `renewable_fraction`. Finite sources
/// get a reservoir sampled from the configured capacity range and a deceleration
/// threshold from the configured threshold range. Registers each source
/// via `Grid::add_source`, propagating any `SourceError`.
///
/// When `source_clustering > 0.0`, a single cluster center is chosen per batch
/// (heat, each chemical species) and all sources in that batch are offset from
/// the center using a 2D normal distribution with toroidal wrapping.
pub(crate) fn generate_sources(
    grid: &mut Grid,
    rng: &mut impl Rng,
    config: &WorldInitConfig,
    num_chemicals: usize,
) -> Result<(), WorldInitError> {
    let width = grid.width();
    let height = grid.height();

    // Heat sources
    let heat_cfg = &config.heat_source_config;
    let heat_renewable_prob = f64::from(heat_cfg.renewable_fraction);
    let heat_count = rng.random_range(heat_cfg.min_sources..=heat_cfg.max_sources);

    generate_field_sources(
        grid,
        rng,
        SourceField::Heat,
        heat_cfg,
        heat_count,
        heat_renewable_prob,
        width,
        height,
    )?;

    // Chemical sources: one batch per species.
    for species in 0..num_chemicals {
        let chem_cfg = &config.chemical_species_configs[species].source_config;
        let chem_renewable_prob = f64::from(chem_cfg.renewable_fraction);
        let chem_count = rng.random_range(chem_cfg.min_sources..=chem_cfg.max_sources);

        generate_field_sources(
            grid,
            rng,
            SourceField::Chemical(species),
            chem_cfg,
            chem_count,
            chem_renewable_prob,
            width,
            height,
        )?;
    }

    Ok(())
}

/// Generate sources for a single field type with multi-center dispersion support.
///
/// Computes K = max(1, round(source_dispersion * num_sources)), clamped to 255.
/// Samples K independent cluster centers, assigns sources round-robin across them,
/// and positions each source via `sample_clustered_position` using its assigned center.
///
/// When `source_dispersion == 0.0`, K=1, collapsing to the legacy single-center behavior.
fn generate_field_sources(
    grid: &mut Grid,
    rng: &mut impl Rng,
    field: SourceField,
    cfg: &SourceFieldConfig,
    num_sources: u32,
    renewable_prob: f64,
    width: u32,
    height: u32,
) -> Result<(), WorldInitError> {
    // Compute cluster count K from dispersion formula.
    let k = compute_cluster_count(cfg.source_dispersion, num_sources);

    // Sample K independent cluster center positions.
    let centers: SmallVec<[ClusterCenter; 8]> = (0..k)
        .map(|_| ClusterCenter {
            col: rng.random_range(0..width),
            row: rng.random_range(0..height),
        })
        .collect();

    // Store centers in ClusterCenterMap when they are meaningful for respawn.
    if cfg.source_clustering > 0.0 || cfg.source_dispersion > 0.0 {
        for (idx, center) in centers.iter().enumerate() {
            grid.cluster_centers_mut().push((field, idx as u8, *center));
        }
    }

    // Generate sources with round-robin cluster assignment.
    for i in 0..num_sources {
        let cluster_idx = (i % u32::from(k)) as u8;
        let center = &centers[cluster_idx as usize];
        let cell_index = sample_clustered_position(
            rng,
            center.col,
            center.row,
            width,
            height,
            cfg.source_clustering,
        );
        let emission_rate = rng.random_range(cfg.min_emission_rate..=cfg.max_emission_rate);
        let (reservoir, initial_capacity, deceleration_threshold) =
            sample_reservoir_params(rng, cfg, renewable_prob);
        grid.add_source(Source {
            cell_index,
            field,
            emission_rate,
            reservoir,
            initial_capacity,
            deceleration_threshold,
            cluster_index: cluster_idx,
        })?;
    }

    Ok(())
}

/// Compute the number of cluster centers from the dispersion formula.
///
/// `K = max(1, round(source_dispersion * num_sources))`, clamped to 255 (u8 bound).
/// When `source_dispersion == 0.0`, returns 1 (single-center, backward compatible).
fn compute_cluster_count(source_dispersion: f32, num_sources: u32) -> u8 {
    let raw = (source_dispersion * num_sources as f32).round() as u32;
    raw.max(1).min(255) as u8
}

/// Sample reservoir parameters for a single source.
///
/// Returns `(reservoir, initial_capacity, deceleration_threshold)`.
/// Renewable sources get `(INFINITY, INFINITY, 0.0)`.
/// Finite sources sample capacity and threshold from the configured ranges.
fn sample_reservoir_params(
    rng: &mut impl Rng,
    config: &SourceFieldConfig,
    renewable_prob: f64,
) -> (f32, f32, f32) {
    if rng.random_bool(renewable_prob) {
        (f32::INFINITY, f32::INFINITY, 0.0)
    } else {
        let capacity =
            rng.random_range(config.min_reservoir_capacity..=config.max_reservoir_capacity);
        let threshold = rng.random_range(
            config.min_deceleration_threshold..=config.max_deceleration_threshold,
        );
        (capacity, capacity, threshold)
    }
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
                inert: false,
                tumble_direction: 0,
                tumble_remaining: 0,
                traits: HeritableTraits::from_config(&actor_config),
                cooldown_remaining: 0,
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
        for val in heat_write.iter_mut().take(cell_count) {
            *val = rng.random_range(config.min_initial_heat..=config.max_initial_heat);
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
        for val in chem_write.iter_mut().take(cell_count) {
            *val = rng.random_range(
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

    let mut grid = Grid::new(grid_config, defaults, actor_config, seed)?;

    // Deterministic RNG forking: each phase draws from an independent stream,
    // so changes to one phase cannot perturb the other.
    let mut master_rng = ChaCha8Rng::seed_from_u64(seed);
    let mut source_rng = ChaCha8Rng::from_rng(&mut master_rng);
    let mut field_rng = ChaCha8Rng::from_rng(&mut master_rng);
    let mut actor_rng = ChaCha8Rng::from_rng(&mut master_rng);

    generate_sources(&mut grid, &mut source_rng, init_config, num_chemicals)?;

    // Pre-allocate respawn queue capacity based on initial source count.
    // Bounded by total sources — the queue can never exceed this.
    let source_count = grid.sources().len();
    *grid.respawn_queue_mut() = RespawnQueue::with_capacity(source_count);

    populate_fields(&mut grid, &mut field_rng, init_config, num_chemicals);
    generate_actors(&mut grid, &mut actor_rng, init_config)?;

    Ok(grid)
}

