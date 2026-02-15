/// Persistent energy source data types, registry, and emission system.
///
/// This module defines the data model for grid energy sources — persistent
/// emitters that inject heat or chemical values into field write buffers
/// each tick during the WARM emission phase.

use crate::grid::Grid;

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
///
/// Renewable sources use `f32::INFINITY` for `reservoir` and `initial_capacity`.
/// This eliminates branching in the emission loop — the deceleration math works
/// identically for both renewable and finite sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub cell_index: usize,
    pub field: SourceField,
    /// Base emission rate (units per tick). May be negative for sinks.
    pub emission_rate: f32,
    /// Remaining emittable quantity. `f32::INFINITY` for renewable sources.
    pub reservoir: f32,
    /// Total capacity at creation. `f32::INFINITY` for renewable sources.
    /// Used as denominator in deceleration computation.
    pub initial_capacity: f32,
    /// Fraction of initial_capacity below which emission decelerates.
    /// 0.0 = no deceleration (full rate until exhaustion).
    /// 1.0 = deceleration begins immediately.
    pub deceleration_threshold: f32,
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

    #[error("invalid reservoir: finite source requires reservoir > 0.0 and reservoir <= initial_capacity, got reservoir={reservoir}, initial_capacity={initial_capacity}")]
    InvalidReservoir {
        reservoir: f32,
        initial_capacity: f32,
    },

    #[error("deceleration threshold {threshold} out of range [0.0, 1.0]")]
    InvalidDecelerationThreshold {
        threshold: f32,
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

        // Validate reservoir fields.
        // Renewable sources: both reservoir and initial_capacity must be INFINITY.
        // Finite sources: both must be finite, positive, and reservoir <= initial_capacity.
        let r = source.reservoir;
        let ic = source.initial_capacity;
        if r.is_infinite() && ic.is_infinite() {
            // Renewable — valid.
        } else if r.is_infinite() || ic.is_infinite() {
            // Mixed infinite/finite — invalid.
            return Err(SourceError::InvalidReservoir {
                reservoir: r,
                initial_capacity: ic,
            });
        } else {
            // Both finite: must be positive and reservoir <= initial_capacity.
            if r <= 0.0 || r.is_nan() || ic <= 0.0 || ic.is_nan() || r > ic {
                return Err(SourceError::InvalidReservoir {
                    reservoir: r,
                    initial_capacity: ic,
                });
            }
        }

        // Validate deceleration threshold in [0.0, 1.0].
        let t = source.deceleration_threshold;
        if !(0.0..=1.0).contains(&t) {
            return Err(SourceError::InvalidDecelerationThreshold { threshold: t });
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

    /// Mutable iteration over active sources in deterministic slot order.
    /// Required by `run_emission` to mutate reservoir state during depletion.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Source> {
        self.slots
            .iter_mut()
            .filter_map(|slot| slot.source.as_mut())
    }

    /// Returns true if the source identified by `id` exists and is depleted
    /// (reservoir == 0.0). Renewable sources (reservoir = INFINITY) are never
    /// depleted.
    pub fn is_depleted(&self, id: SourceId) -> Result<bool, SourceError> {
        let slot = self.slots.get(id.index).ok_or(SourceError::InvalidSourceId {
            index: id.index,
            generation: id.generation,
        })?;

        if slot.generation != id.generation || slot.source.is_none() {
            return Err(SourceError::InvalidSourceId {
                index: id.index,
                generation: id.generation,
            });
        }

        // SAFETY of unwrap: guarded by the is_none() check above.
        let source = slot.source.as_ref().expect("checked above");
        Ok(source.reservoir == 0.0)
    }

    /// Count of active sources with reservoir > 0.0 (includes renewable
    /// sources where reservoir is INFINITY). Depleted sources (reservoir == 0.0)
    /// are excluded.
    pub fn active_emitting_count(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|slot| slot.source.as_ref())
            .filter(|source| source.reservoir > 0.0)
            .count()
    }
}

// WARM PATH: Executes once per tick over the source list.
// No heap allocation. No dynamic dispatch. Sequential iteration.

/// Inject emission rates from all active sources into the appropriate
/// field write buffers.
///
/// Iterates the registry in deterministic slot order. For each source,
/// adds `emission_rate` to the write buffer of the target field at the
/// source's `cell_index`. Caller is responsible for copy-read-to-write
/// before calling this, and for validation + swap after.
///
/// Species indices were validated at registration time, so chemical
/// buffer lookups use `get_mut` with a silent skip on index mismatch
/// (defensive — should never occur in practice).
pub fn run_emission(grid: &mut Grid, registry: &SourceRegistry) {
    for source in registry.iter() {
        match source.field {
            SourceField::Heat => {
                grid.write_heat()[source.cell_index] += source.emission_rate;
            }
            SourceField::Chemical(species) => {
                // Species validated at add() time. Defensive get_mut avoids
                // panic if registry and grid are somehow out of sync.
                if let Ok(buf) = grid.write_chemical(species) {
                    buf[source.cell_index] += source.emission_rate;
                }
            }
        }
    }
}

