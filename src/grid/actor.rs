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
