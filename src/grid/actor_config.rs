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
    /// Maximum energy an Actor can hold. Energy is clamped to this
    /// ceiling after each metabolic tick. Must be > 0.0 and >= initial_energy.
    pub max_energy: f32,
    /// Pre-allocated slot capacity for the ActorRegistry.
    pub initial_actor_capacity: usize,
    /// Energy subtracted from an Actor when it successfully moves to an adjacent cell.
    pub movement_cost: f32,
    /// Energy level below which an inert Actor is permanently removed (must be <= 0.0).
    pub removal_threshold: f32,
    /// Energy cost per unit of chemical consumed. Reduces net energy gain
    /// from consumption. Must be in `[0.0, energy_conversion_factor)`.
    /// Default: 0.2
    pub extraction_cost: f32,
    /// Power-law exponent α for Lévy flight step distribution.
    /// Higher values → shorter average runs. Must be > 1.0. Default: 1.5.
    pub levy_exponent: f32,
    /// Maximum steps in a single tumble run. Clamps the power-law sample.
    /// Must be >= 1. Default: 20.
    pub max_tumble_steps: u16,
    /// Energy threshold for binary fission. Actor must have energy >= this value.
    /// Must be > 0.0. Default: 20.0.
    pub reproduction_threshold: f32,
    /// Total energy deducted from the parent upon fission.
    /// Must be > 0.0 and >= offspring_energy. Default: 12.0.
    pub reproduction_cost: f32,
    /// Energy assigned to the offspring Actor at creation.
    /// Must be > 0.0 and <= max_energy. Default: 10.0.
    pub offspring_energy: f32,
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            consumption_rate: 1.5,
            energy_conversion_factor: 2.0,
            base_energy_decay: 0.05,
            initial_energy: 10.0,
            max_energy: 50.0,
            initial_actor_capacity: 64,
            movement_cost: 0.5,
            removal_threshold: -5.0,
            extraction_cost: 0.2,
            levy_exponent: 1.5,
            max_tumble_steps: 20,
            reproduction_threshold: 20.0,
            reproduction_cost: 12.0,
            offspring_energy: 10.0,
        }
    }
}
