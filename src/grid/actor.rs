/// Actor data types, generational registry slot, and error enum.
///
/// Mirrors the `source` module's generational slot pattern. Actors are
/// mobile biological agents occupying exactly one grid cell, with an
/// internal energy reserve. This module defines only the data model;
/// the registry and system functions live in separate modules.

use crate::grid::actor_config::ActorConfig;
use rand::Rng;
use rand_distr::{Distribution, Normal};

/// Per-actor heritable trait values. Inherited from parent during fission
/// with proportional gaussian mutation. 32 bytes (includes 2 bytes padding after u16).
///
/// Plain data struct — no methods beyond construction and mutation.
/// Stored inline in `Actor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeritableTraits {
    pub consumption_rate: f32,
    pub base_energy_decay: f32,
    pub levy_exponent: f32,
    pub reproduction_threshold: f32,
    pub max_tumble_steps: u16,
    pub reproduction_cost: f32,
    pub offspring_energy: f32,
    pub mutation_rate: f32,
}

const _: () = assert!(std::mem::size_of::<HeritableTraits>() == 32);

impl HeritableTraits {
    /// Create traits from global config defaults (seed genome).
    pub fn from_config(config: &ActorConfig) -> Self {
        Self {
            consumption_rate: config.consumption_rate,
            base_energy_decay: config.base_energy_decay,
            levy_exponent: config.levy_exponent,
            reproduction_threshold: config.reproduction_threshold,
            max_tumble_steps: config.max_tumble_steps,
            reproduction_cost: config.reproduction_cost,
            offspring_energy: config.offspring_energy,
            mutation_rate: config.mutation_stddev,
        }
    }

    /// Apply independent proportional gaussian mutation to all eight trait fields,
    /// then clamp each to its configured range. No-op when `mutation_rate == 0.0`.
    ///
    /// Proportional model: `trait * (1.0 + Normal(0, mutation_rate))`.
    /// `max_tumble_steps` is mutated in f32 space (convert → scale → round → clamp → cast u16).
    /// `mutation_rate` mutates itself last, using the pre-mutation rate as σ.
    ///
    /// The caller is responsible for providing a deterministically-seeded RNG
    /// derived from the simulation master seed, tick, and spawn index.
    pub fn mutate(&mut self, config: &ActorConfig, rng: &mut impl Rng) {
        if self.mutation_rate == 0.0 {
            return;
        }

        // SAFETY of expect: mutation_rate is validated > 0.0 via clamp bounds at config load.
        let normal = Normal::new(0.0_f64, self.mutation_rate as f64)
            .expect("mutation_rate validated non-negative at config load");

        self.consumption_rate = (self.consumption_rate * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_consumption_rate_min, config.trait_consumption_rate_max);

        self.base_energy_decay = (self.base_energy_decay * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_base_energy_decay_min, config.trait_base_energy_decay_max);

        self.levy_exponent = (self.levy_exponent * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_levy_exponent_min, config.trait_levy_exponent_max);

        self.reproduction_threshold = (self.reproduction_threshold * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_reproduction_threshold_min, config.trait_reproduction_threshold_max);

        // max_tumble_steps: proportional in f32 space, round, clamp to u16 range.
        let tumble_f32 = self.max_tumble_steps as f32 * (1.0 + normal.sample(rng) as f32);
        self.max_tumble_steps = tumble_f32
            .round()
            .clamp(config.trait_max_tumble_steps_min as f32, config.trait_max_tumble_steps_max as f32)
            as u16;

        self.reproduction_cost = (self.reproduction_cost * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_reproduction_cost_min, config.trait_reproduction_cost_max);

        self.offspring_energy = (self.offspring_energy * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_offspring_energy_min, config.trait_offspring_energy_max);

        // mutation_rate mutates itself — self-referential proportional mutation.
        // Uses the pre-mutation rate (captured in `normal` above) as σ.
        self.mutation_rate = (self.mutation_rate * (1.0 + normal.sample(rng) as f32))
            .clamp(config.trait_mutation_rate_min, config.trait_mutation_rate_max);
    }
}

/// A mobile biological agent occupying one grid cell.
///
/// Plain data struct — no methods beyond construction. Carries the
/// physical state needed for simulation: position, energy, tumble state,
/// and heritable traits for per-actor behavioral variation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Actor {
    pub cell_index: usize,
    pub energy: f32,
    pub inert: bool,
    /// Encoded tumble direction: 0=North, 1=South, 2=West, 3=East.
    /// Only meaningful when tumble_remaining > 0.
    pub tumble_direction: u8,
    /// Steps remaining in current Lévy flight tumble run. 0 = not tumbling.
    pub tumble_remaining: u16,
    /// Per-actor heritable traits for behavioral variation.
    pub traits: HeritableTraits,
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

    /// Total number of slots (active + free). Used to size external
    /// buffers indexed by slot index (e.g., movement_targets).
    pub fn slot_count(&self) -> usize {
        self.slots.len()
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

    /// Iterate active Actors mutably, yielding `(ActorId, &mut Actor)` in
    /// deterministic slot-index order.
    ///
    /// Used by metabolism (and future systems) that need the full
    /// generational id to record actors for deferred removal.
    pub fn iter_mut_with_ids(&mut self) -> impl Iterator<Item = (ActorId, &mut Actor)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, slot)| {
            let generation = slot.generation;
            slot.actor.as_mut().map(|actor| {
                (ActorId { index: i, generation }, actor)
            })
        })
    }
}

