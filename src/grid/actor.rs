/// Actor data types, generational registry slot, and error enum.
///
/// Mirrors the `source` module's generational slot pattern. Actors are
/// mobile biological agents occupying exactly one grid cell, with an
/// internal energy reserve. This module defines only the data model;
/// the registry and system functions live in separate modules.

/// A mobile biological agent occupying one grid cell.
///
/// Plain data struct — no methods beyond construction. Carries only the
/// physical state needed for v1: position and energy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Actor {
    pub cell_index: usize,
    pub energy: f32,
}

/// Opaque handle for a registered Actor.
///
/// Generational index: the `generation` field detects stale removals
/// after a slot has been reused, preventing the ABA problem. Matches
/// the pattern established by `SourceId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

/// Internal slot in the `ActorRegistry`.
///
/// Holds an optional Actor and a generation counter for generational
/// index validation. Generation is bumped on removal so that stale
/// `ActorId` handles are detected on subsequent access.
pub(crate) struct ActorSlot {
    pub(crate) actor: Option<Actor>,
    pub(crate) generation: u64,
}

/// Errors from Actor registration and management.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ActorError {
    #[error("cell index {cell_index} out of bounds (grid has {cell_count} cells)")]
    CellOutOfBounds {
        cell_index: usize,
        cell_count: usize,
    },

    #[error("cell {cell_index} is already occupied")]
    CellOccupied { cell_index: usize },

    #[error("invalid actor id (index={index}, generation={generation})")]
    InvalidActorId { index: usize, generation: u64 },
}

/// Stores all active Actors in a contiguous Vec with generational slots.
///
/// Slot-based storage mirrors `SourceRegistry`: each slot holds an
/// `Option<Actor>` and a generation counter. Removed slots become `None`
/// and are reused via a free list, avoiding linear scans on insertion.
///
/// Key difference from `SourceRegistry`: `add` and `remove` take a mutable
/// occupancy map slice (`&mut [Option<usize>]`) to maintain cell→slot
/// consistency atomically with registry mutations.
pub struct ActorRegistry {
    slots: Vec<ActorSlot>,
    free_list: Vec<usize>,
    active_count: usize,
}

impl ActorRegistry {
    /// Create an empty registry with no pre-allocated capacity.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Create an empty registry with pre-allocated slot capacity.
    ///
    /// Use this at grid construction time so that no heap allocation
    /// occurs during tick execution (WARM path requirement).
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            slots: Vec::with_capacity(cap),
            free_list: Vec::with_capacity(cap),
            active_count: 0,
        }
    }

    /// Add an Actor to the registry, updating the occupancy map.
    ///
    /// Validates:
    /// - `actor.cell_index < cell_count` → `CellOutOfBounds`
    /// - `occupancy[actor.cell_index]` is `None` → `CellOccupied`
    ///
    /// On success, inserts the Actor into a slot (reusing from the free
    /// list if available), sets `occupancy[cell_index] = Some(slot_index)`,
    /// and returns the `ActorId`.
    pub fn add(
        &mut self,
        actor: Actor,
        cell_count: usize,
        occupancy: &mut [Option<usize>],
    ) -> Result<ActorId, ActorError> {
        if actor.cell_index >= cell_count {
            return Err(ActorError::CellOutOfBounds {
                cell_index: actor.cell_index,
                cell_count,
            });
        }

        if occupancy[actor.cell_index].is_some() {
            return Err(ActorError::CellOccupied {
                cell_index: actor.cell_index,
            });
        }

        let (index, generation) = if let Some(free_index) = self.free_list.pop() {
            let slot = &mut self.slots[free_index];
            slot.actor = Some(actor);
            (free_index, slot.generation)
        } else {
            let index = self.slots.len();
            self.slots.push(ActorSlot {
                actor: Some(actor),
                generation: 0,
            });
            (index, 0)
        };

        occupancy[actor.cell_index] = Some(index);
        self.active_count += 1;

        Ok(ActorId { index, generation })
    }

    /// Remove an Actor by its identifier, clearing the occupancy map.
    ///
    /// Validates the `ActorId` generation against the slot's current
    /// generation. On success, clears the slot, bumps generation,
    /// pushes the slot index to the free list, and clears the
    /// occupancy entry for the Actor's former cell.
    pub fn remove(
        &mut self,
        id: ActorId,
        occupancy: &mut [Option<usize>],
    ) -> Result<(), ActorError> {
        let slot = self.slots.get_mut(id.index).ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })?;

        if slot.generation != id.generation {
            return Err(ActorError::InvalidActorId {
                index: id.index,
                generation: id.generation,
            });
        }

        // Take the actor out of the slot. If already None, the id is stale.
        let actor = slot.actor.take().ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })?;

        occupancy[actor.cell_index] = None;
        slot.generation += 1;
        self.free_list.push(id.index);
        self.active_count -= 1;

        Ok(())
    }

    /// Get a shared reference to an Actor by its identifier.
    pub fn get(&self, id: ActorId) -> Result<&Actor, ActorError> {
        let slot = self.slots.get(id.index).ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })?;

        if slot.generation != id.generation {
            return Err(ActorError::InvalidActorId {
                index: id.index,
                generation: id.generation,
            });
        }

        slot.actor.as_ref().ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })
    }

    /// Get a mutable reference to an Actor by its identifier.
    pub fn get_mut(&mut self, id: ActorId) -> Result<&mut Actor, ActorError> {
        let slot = self.slots.get_mut(id.index).ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })?;

        if slot.generation != id.generation {
            return Err(ActorError::InvalidActorId {
                index: id.index,
                generation: id.generation,
            });
        }

        slot.actor.as_mut().ok_or(ActorError::InvalidActorId {
            index: id.index,
            generation: id.generation,
        })
    }

    /// Number of active (non-removed) Actors.
    pub fn len(&self) -> usize {
        self.active_count
    }

    /// Whether the registry has no active Actors.
    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    /// Iterate active Actors in deterministic slot-index order.
    ///
    /// Yields `(slot_index, &Actor)` for each occupied slot, ascending
    /// by slot index. This ordering is critical for deterministic
    /// sensing, metabolism, and movement phases.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Actor)> {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            slot.actor.as_ref().map(|actor| (i, actor))
        })
    }

    /// Iterate active Actors mutably in deterministic slot-index order.
    ///
    /// Yields `(slot_index, &mut Actor)` for each occupied slot.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut Actor)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, slot)| {
            slot.actor.as_mut().map(|actor| (i, actor))
        })
    }
}

