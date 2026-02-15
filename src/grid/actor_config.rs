use serde::{Deserialize, Serialize};

/// Configuration parameters for Actor metabolism, sensing, and spawning.
///
/// Plain data struct — immutable after construction. All rates are per-tick.
/// Provided at grid construction time via `Option<ActorConfig>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActorConfig {
    /// Chemical units consumed per tick from the Actor's current cell (species 0).
    pub consumption_rate: f32,
    /// Energy gained per unit of chemical consumed.
    pub energy_conversion_factor: f32,
    /// Energy subtracted from every Actor each tick (basal metabolic cost).
    pub base_energy_decay: f32,
    /// Energy assigned to newly spawned Actors.
    pub initial_energy: f32,
    /// Pre-allocated slot capacity for the ActorRegistry.
    pub initial_actor_capacity: usize,
    /// Energy subtracted from an Actor when it successfully moves to an adjacent cell.
    pub movement_cost: f32,
    /// Energy level below which an inert Actor is permanently removed (must be <= 0.0).
    pub removal_threshold: f32,
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            consumption_rate: 1.5,
            energy_conversion_factor: 2.0,
            base_energy_decay: 0.05,
            initial_energy: 10.0,
            initial_actor_capacity: 64,
            movement_cost: 0.5,
            removal_threshold: -5.0,
        }
    }
}
