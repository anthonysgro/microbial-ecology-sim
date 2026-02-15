pub mod config;
pub mod diffusion;
pub mod heat;
pub mod error;
pub mod field_buffer;
pub mod partition;

use config::{CellDefaults, GridConfig};
use error::GridError;
use field_buffer::FieldBuffer;
use partition::{compute_partitions, Partition};

/// Top-level environment grid.
///
/// Owns all field buffers (SoA layout) and spatial partition metadata.
/// Each physical field is a separate contiguous `FieldBuffer<f32>`:
/// one per chemical species, plus one each for heat and moisture.
pub struct Grid {
    config: GridConfig,
    chemicals: Vec<FieldBuffer<f32>>,
    heat: FieldBuffer<f32>,
    moisture: FieldBuffer<f32>,
    partitions: Vec<Partition>,
}

impl Grid {
    /// Construct a new grid, validating dimensions and allocating all
    /// double-buffered SoA field arrays.
    ///
    /// Returns `GridError::InvalidDimensions` if width or height is zero.
    pub fn new(config: GridConfig, defaults: CellDefaults) -> Result<Self, GridError> {
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
        let moisture = FieldBuffer::new(cell_count, defaults.moisture);

        let partitions = compute_partitions(
            config.width,
            config.height,
            config.num_threads,
        );

        Ok(Self {
            config,
            chemicals,
            heat,
            moisture,
            partitions,
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

    pub fn read_moisture(&self) -> &[f32] {
        self.moisture.read()
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

    pub fn write_moisture(&mut self) -> &mut [f32] {
        self.moisture.write()
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

    pub fn swap_moisture(&mut self) {
        self.moisture.swap();
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
}
