/// Persistent energy source data types and registry error handling.
///
/// This module defines the data model for grid energy sources — persistent
/// emitters that inject heat or chemical values into field write buffers
/// each tick during the WARM emission phase.

/// Identifies which grid field a source emits into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceField {
    Heat,
    Chemical(usize),
}

/// Opaque identifier for a registered source.
///
/// Internally a generational index: the `generation` field detects stale
/// removals after a slot has been reused, preventing the ABA problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

/// A persistent emitter that injects a value into a grid field each tick.
///
/// Plain data struct — no methods beyond construction. Negative `emission_rate`
/// values are valid and represent drains (sinks).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub cell_index: usize,
    pub field: SourceField,
    pub emission_rate: f32,
}

/// Internal slot in the `SourceRegistry`. Holds an optional source and a
/// generation counter for generational index validation.
pub(crate) struct SourceSlot {
    pub(crate) source: Option<Source>,
    pub(crate) generation: u64,
}

/// Errors from source registration and management.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SourceError {
    #[error("cell index {cell_index} out of bounds (grid has {cell_count} cells)")]
    CellOutOfBounds {
        cell_index: usize,
        cell_count: usize,
    },

    #[error("chemical species {species} out of range (grid has {num_chemicals} species)")]
    InvalidChemicalSpecies {
        species: usize,
        num_chemicals: usize,
    },

    #[error("invalid source id (index={index}, generation={generation})")]
    InvalidSourceId {
        index: usize,
        generation: u64,
    },
}

/// Stores all active sources in a contiguous Vec with generational slots.
///
/// Slot-based storage: each slot holds an `Option<Source>` and a generation
/// counter. Removed slots become `None` and are reused on the next insert.
/// A free list tracks available slots to avoid linear scans on insertion.
pub struct SourceRegistry {
    slots: Vec<SourceSlot>,
    free_list: Vec<usize>,
    active_count: usize,
}

impl SourceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Add a source. Returns a `SourceId` for later removal.
    ///
    /// Validates `cell_index` against `cell_count` and chemical species
    /// against `num_chemicals`. Reuses free slots via the free list.
    pub fn add(
        &mut self,
        source: Source,
        cell_count: usize,
        num_chemicals: usize,
    ) -> Result<SourceId, SourceError> {
        // Validate cell bounds.
        if source.cell_index >= cell_count {
            return Err(SourceError::CellOutOfBounds {
                cell_index: source.cell_index,
                cell_count,
            });
        }

        // Validate chemical species if applicable.
        if let SourceField::Chemical(species) = source.field {
            if species >= num_chemicals {
                return Err(SourceError::InvalidChemicalSpecies {
                    species,
                    num_chemicals,
                });
            }
        }

        let (index, generation) = if let Some(free_index) = self.free_list.pop() {
            // Reuse a freed slot. Generation was already bumped on removal.
            let slot = &mut self.slots[free_index];
            slot.source = Some(source);
            (free_index, slot.generation)
        } else {
            // Append a new slot.
            let index = self.slots.len();
            self.slots.push(SourceSlot {
                source: Some(source),
                generation: 0,
            });
            (index, 0)
        };

        self.active_count += 1;

        Ok(SourceId { index, generation })
    }

    /// Remove a source by its identifier.
    ///
    /// Returns `SourceError::InvalidSourceId` if the id is stale or out of range.
    pub fn remove(&mut self, id: SourceId) -> Result<(), SourceError> {
        let slot = self.slots.get_mut(id.index).ok_or(SourceError::InvalidSourceId {
            index: id.index,
            generation: id.generation,
        })?;

        if slot.generation != id.generation || slot.source.is_none() {
            return Err(SourceError::InvalidSourceId {
                index: id.index,
                generation: id.generation,
            });
        }

        slot.source = None;
        slot.generation += 1;
        self.free_list.push(id.index);
        self.active_count -= 1;

        Ok(())
    }

    /// Number of active (non-removed) sources.
    pub fn len(&self) -> usize {
        self.active_count
    }

    /// Whether the registry has no active sources.
    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    /// Iterate active sources in deterministic slot order.
    pub fn iter(&self) -> impl Iterator<Item = &Source> {
        self.slots.iter().filter_map(|slot| slot.source.as_ref())
    }
}

