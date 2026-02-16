use serde::{Deserialize, Serialize};

/// Immutable configuration for the environment grid.
///
/// Provided at initialization time. All rates must be chosen such that
/// the discrete update remains stable (e.g., `diffusion_rate * tick_duration * 8 < 1.0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridConfig {
    pub width: u32,
    pub height: u32,
    /// Fixed number of chemical species tracked per cell.
    pub num_chemicals: usize,
    /// Heat radiation coefficient (thermal conductivity).
    pub thermal_conductivity: f32,
    /// Boundary condition for heat: missing neighbors use this value.
    pub ambient_heat: f32,
    /// Simulated time per tick (seconds).
    pub tick_duration: f32,
    /// Number of spatial partitions (maps to thread count for rayon).
    pub num_threads: usize,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            width: 30,
            height: 30,
            num_chemicals: 2,
            thermal_conductivity: 0.05,
            ambient_heat: 0.0,
            tick_duration: 1.0,
            num_threads: 4,
        }
    }
}

/// Default values for initializing every cell in the grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CellDefaults {
    /// One concentration value per chemical species.
    pub chemical_concentrations: Vec<f32>,
    pub heat: f32,
}

impl Default for CellDefaults {
    fn default() -> Self {
        Self {
            chemical_concentrations: Vec::new(),
            heat: 0.0,
        }
    }
}
