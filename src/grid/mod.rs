pub mod actor;
pub mod actor_config;
pub mod config;
pub mod diffusion;
pub mod heat;
pub mod error;
pub mod field_buffer;
pub mod partition;
pub mod source;
pub mod tick;
pub mod world_init;

use actor::{Actor, ActorError, ActorId, ActorRegistry};
use actor_config::ActorConfig;
use config::{CellDefaults, GridConfig};
use error::GridError;
use field_buffer::FieldBuffer;
use partition::{compute_partitions, Partition};
use source::SourceRegistry;

/// Top-level environment grid.
///
/// Owns all field buffers (SoA layout) and spatial partition metadata.
/// Each physical field is a separate contiguous `FieldBuffer<f32>`:
/// one per chemical species, plus one for heat.
pub struct Grid {
    config: GridConfig,
    chemicals: Vec<FieldBuffer<f32>>,
    heat: FieldBuffer<f32>,
    partitions: Vec<Partition>,
    sources: SourceRegistry,
    actors: ActorRegistry,
    actor_config: Option<ActorConfig>,
    /// Cell index → slot index of the occupying Actor, or None.
    /// Pre-allocated to `cell_count` at construction time.
    occupancy: Vec<Option<usize>>,
    /// Pre-allocated buffer for deferred Actor removal during metabolism.
    removal_buffer: Vec<ActorId>,
    /// Pre-allocated buffer: slot index → target cell index for movement.
    movement_targets: Vec<Option<usize>>,
}

impl Grid {
    /// Construct a new grid, validating dimensions and allocating all
    /// double-buffered SoA field arrays.
    ///
    /// Returns `GridError::InvalidDimensions` if width or height is zero.
    pub fn new(
        config: GridConfig,
        defaults: CellDefaults,
        actor_config: Option<ActorConfig>,
    ) -> Result<Self, GridError> {
        if config.width == 0 || config.height == 0 {
            return Err(GridError::InvalidDimensions {
                width: config.width,
                height: config.height,
            });
        }

        let cell_count = (config.width as usize) * (config.height as usize);

        // One FieldBuffer per chemical species, initialized to caller defaults.
        let chemicals: Vec<FieldBuffer<f32>> = defaults
            .chemical_concentrations
            .iter()
            .map(|&default_conc| FieldBuffer::new(cell_count, default_conc))
            .collect();

        let heat = FieldBuffer::new(cell_count, defaults.heat);

        let partitions = compute_partitions(
            config.width,
            config.height,
            config.num_threads,
        );

        // Pre-allocate actor subsystem buffers based on config, or use
        // zero-capacity defaults when actors are not configured.
        let initial_cap = actor_config
            .as_ref()
            .map_or(0, |ac| ac.initial_actor_capacity);

        let actors = if initial_cap > 0 {
            ActorRegistry::with_capacity(initial_cap)
        } else {
            ActorRegistry::new()
        };

        let occupancy = vec![None; cell_count];
        let removal_buffer = Vec::with_capacity(initial_cap);
        let movement_targets = Vec::with_capacity(initial_cap);

        Ok(Self {
            config,
            chemicals,
            heat,
            partitions,
            sources: SourceRegistry::new(),
            actors,
            actor_config,
            occupancy,
            removal_buffer,
            movement_targets,
        })
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn cell_count(&self) -> usize {
        (self.config.width as usize) * (self.config.height as usize)
    }

    pub fn config(&self) -> &GridConfig {
        &self.config
    }

    // ── Coordinate access ──────────────────────────────────────────

    /// Convert (x, y) to a flat index. Returns `OutOfBounds` if invalid.
    #[inline]
    pub fn index(&self, x: u32, y: u32) -> Result<usize, GridError> {
        if x >= self.config.width || y >= self.config.height {
            return Err(GridError::OutOfBounds {
                x,
                y,
                width: self.config.width,
                height: self.config.height,
            });
        }
        Ok((y as usize) * (self.config.width as usize) + (x as usize))
    }

    // ── Read access (current read buffer) ──────────────────────────

    pub fn read_heat(&self) -> &[f32] {
        self.heat.read()
    }

    pub fn read_chemical(&self, species: usize) -> Result<&[f32], GridError> {
        self.chemicals
            .get(species)
            .map(FieldBuffer::read)
            .ok_or(GridError::InvalidChemicalSpecies {
                species,
                num_chemicals: self.chemicals.len(),
            })
    }

    // ── Write access (current write buffer) ────────────────────────

    pub fn write_heat(&mut self) -> &mut [f32] {
        self.heat.write()
    }

    pub fn write_chemical(&mut self, species: usize) -> Result<&mut [f32], GridError> {
        let num = self.chemicals.len();
        self.chemicals
            .get_mut(species)
            .map(FieldBuffer::write)
            .ok_or(GridError::InvalidChemicalSpecies {
                species,
                num_chemicals: num,
            })
    }

    // ── Buffer swaps ───────────────────────────────────────────────

    pub fn swap_heat(&mut self) {
        self.heat.swap();
    }

    pub fn swap_chemicals(&mut self) {
        for buf in &mut self.chemicals {
            buf.swap();
        }
    }

    /// Simultaneous read and write access to the heat field buffer.
    ///
    /// Returns `(read_slice, write_slice)` referencing distinct allocations.
    pub fn read_write_heat(&mut self) -> (&[f32], &mut [f32]) {
        self.heat.read_write()
    }

    /// Simultaneous read and write access to a chemical species buffer.
    ///
    /// Returns `(read_slice, write_slice)` for the given species.
    /// The two slices reference distinct allocations within the `FieldBuffer`.
    pub fn read_write_chemical(
        &mut self,
        species: usize,
    ) -> Result<(&[f32], &mut [f32]), GridError> {
        let num = self.chemicals.len();
        self.chemicals
            .get_mut(species)
            .map(FieldBuffer::read_write)
            .ok_or(GridError::InvalidChemicalSpecies {
                species,
                num_chemicals: num,
            })
    }

    // ── Partition access ───────────────────────────────────────────

    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }

    // ── Source registry access ─────────────────────────────────────

    pub fn sources(&self) -> &SourceRegistry {
        &self.sources
    }

    pub fn sources_mut(&mut self) -> &mut SourceRegistry {
        &mut self.sources
    }

    /// Temporarily take the source registry out of the grid.
    ///
    /// Used by the emission phase to split the borrow: the registry is
    /// extracted, emission runs against `&mut Grid` + `&SourceRegistry`,
    /// then the registry is returned via `put_sources`.
    pub(crate) fn take_sources(&mut self) -> SourceRegistry {
        std::mem::replace(&mut self.sources, SourceRegistry::new())
    }

    /// Return a previously taken source registry.
    pub(crate) fn put_sources(&mut self, sources: SourceRegistry) {
        self.sources = sources;
    }

    // ── Actor registry access ──────────────────────────────────────

    pub fn actors(&self) -> &ActorRegistry {
        &self.actors
    }

    pub fn actors_mut(&mut self) -> &mut ActorRegistry {
        &mut self.actors
    }

    pub fn occupancy(&self) -> &[Option<usize>] {
        &self.occupancy
    }

    pub fn actor_config(&self) -> Option<&ActorConfig> {
        self.actor_config.as_ref()
    }

    /// Add an Actor to the grid, validating cell_index against grid dimensions.
    pub fn add_actor(&mut self, actor: Actor) -> Result<ActorId, ActorError> {
        let cell_count = self.cell_count();
        self.actors.add(actor, cell_count, &mut self.occupancy)
    }

    /// Remove an Actor from the grid by its identifier.
    pub fn remove_actor(&mut self, id: ActorId) -> Result<(), ActorError> {
        self.actors.remove(id, &mut self.occupancy)
    }

    /// Temporarily extract the actor registry and occupancy map for
    /// split-borrow patterns in actor system phases.
    ///
    /// Returns `(ActorRegistry, Vec<Option<usize>>, Vec<ActorId>, Vec<Option<usize>>)`
    /// — the registry, occupancy map, removal buffer, and movement targets.
    pub(crate) fn take_actors(
        &mut self,
    ) -> (ActorRegistry, Vec<Option<usize>>, Vec<ActorId>, Vec<Option<usize>>) {
        let actors = std::mem::replace(&mut self.actors, ActorRegistry::new());
        let occupancy = std::mem::take(&mut self.occupancy);
        let removal_buffer = std::mem::take(&mut self.removal_buffer);
        let movement_targets = std::mem::take(&mut self.movement_targets);
        (actors, occupancy, removal_buffer, movement_targets)
    }

    /// Return previously taken actor subsystem state.
    pub(crate) fn put_actors(
        &mut self,
        actors: ActorRegistry,
        occupancy: Vec<Option<usize>>,
        removal_buffer: Vec<ActorId>,
        movement_targets: Vec<Option<usize>>,
    ) {
        self.actors = actors;
        self.occupancy = occupancy;
        self.removal_buffer = removal_buffer;
        self.movement_targets = movement_targets;
    }

    /// Add a source to the grid's registry, validating against grid dimensions.
    ///
    /// Delegates to `SourceRegistry::add()` with this grid's cell_count and
    /// num_chemicals for bounds checking.
    pub fn add_source(&mut self, source: source::Source) -> Result<source::SourceId, source::SourceError> {
        let cell_count = self.cell_count();
        let num_chemicals = self.num_chemicals();
        self.sources.add(source, cell_count, num_chemicals)
    }

    /// Remove a source from the grid's registry by its identifier.
    ///
    /// Delegates to `SourceRegistry::remove()`.
    pub fn remove_source(&mut self, id: source::SourceId) -> Result<(), source::SourceError> {
        self.sources.remove(id)
    }

    // ── Internal field buffer access for emission phase ────────────

    /// Direct mutable access to the heat `FieldBuffer`.
    pub(crate) fn heat_buffer_mut(&mut self) -> &mut FieldBuffer<f32> {
        &mut self.heat
    }

    /// Direct mutable access to a chemical species `FieldBuffer`.
    ///
    /// Returns `None` if species is out of range.
    pub(crate) fn chemical_buffer_mut(&mut self, species: usize) -> Option<&mut FieldBuffer<f32>> {
        self.chemicals.get_mut(species)
    }

    /// Number of chemical species in this grid.
    pub fn num_chemicals(&self) -> usize {
        self.chemicals.len()
    }
}
