/// Immutable configuration for the environment grid.
///
/// Provided at initialization time. All rates must be chosen such that
/// the discrete update remains stable (e.g., `diffusion_rate * tick_duration * 8 < 1.0`).
#[derive(Debug, Clone, PartialEq)]
pub struct GridConfig {
    pub width: u32,
    pub height: u32,
    /// Fixed number of chemical species tracked per cell.
    pub num_chemicals: usize,
    /// Chemical diffusion coefficient (discrete Laplacian scaling factor).
    pub diffusion_rate: f32,
    /// Heat radiation coefficient (thermal conductivity).
    pub thermal_conductivity: f32,
    /// Boundary condition for heat: missing neighbors use this value.
    pub ambient_heat: f32,
    /// Simulated time per tick (seconds).
    pub tick_duration: f32,
    /// Number of spatial partitions (maps to thread count for rayon).
    pub num_threads: usize,
    /// Per-species chemical decay rate. Length must equal `num_chemicals`.
    /// Each value in [0.0, 1.0]. Applied as `concentration *= (1.0 - rate)` per tick.
    pub chemical_decay_rates: Vec<f32>,
}

/// Default values for initializing every cell in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct CellDefaults {
    /// One concentration value per chemical species.
    pub chemical_concentrations: Vec<f32>,
    pub heat: f32,
}
