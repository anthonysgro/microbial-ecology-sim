use std::fmt;

/// Errors arising from grid construction or coordinate access.
#[derive(Debug, Clone, PartialEq)]
pub enum GridError {
    InvalidDimensions { width: u32, height: u32 },
    OutOfBounds { x: u32, y: u32, width: u32, height: u32 },
    InvalidChemicalSpecies { species: usize, num_chemicals: usize },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid grid dimensions: {width}x{height}")
            }
            Self::OutOfBounds { x, y, width, height } => {
                write!(
                    f,
                    "coordinate ({x}, {y}) out of bounds for {width}x{height} grid"
                )
            }
            Self::InvalidChemicalSpecies {
                species,
                num_chemicals,
            } => {
                write!(
                    f,
                    "chemical species {species} out of range (max {num_chemicals})"
                )
            }
        }
    }
}

impl std::error::Error for GridError {}

/// Errors arising during tick execution.
#[derive(Debug, Clone, PartialEq)]
pub enum TickError {
    NumericalError {
        system: &'static str,
        cell_index: usize,
        field: &'static str,
        value: f32,
    },
}

impl fmt::Display for TickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NumericalError {
                system,
                cell_index,
                field,
                value,
            } => {
                write!(
                    f,
                    "numerical error in {system} at cell {cell_index}, field '{field}': {value}"
                )
            }
        }
    }
}

impl std::error::Error for TickError {}
