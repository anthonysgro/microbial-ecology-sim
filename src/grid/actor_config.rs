use serde::{Deserialize, Serialize};

// ── Serde default functions for heritable trait mutation fields ─────
fn default_mutation_stddev() -> f32 { 0.05 }
fn default_trait_consumption_rate_min() -> f32 { 0.1 }
fn default_trait_consumption_rate_max() -> f32 { 10.0 }
fn default_trait_base_energy_decay_min() -> f32 { 0.001 }
fn default_trait_base_energy_decay_max() -> f32 { 1.0 }
fn default_trait_levy_exponent_min() -> f32 { 1.01 }
fn default_trait_levy_exponent_max() -> f32 { 3.0 }
fn default_trait_reproduction_threshold_min() -> f32 { 1.0 }
fn default_trait_reproduction_threshold_max() -> f32 { 100.0 }
fn default_trait_max_tumble_steps_min() -> u16 { 1 }
fn default_trait_max_tumble_steps_max() -> u16 { 50 }
fn default_trait_reproduction_cost_min() -> f32 { 0.1 }
fn default_trait_reproduction_cost_max() -> f32 { 100.0 }
fn default_trait_offspring_energy_min() -> f32 { 0.1 }
fn default_trait_offspring_energy_max() -> f32 { 100.0 }
fn default_trait_mutation_rate_min() -> f32 { 0.001 }
fn default_trait_mutation_rate_max() -> f32 { 0.5 }

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

    // ── Heritable trait mutation config ─────────────────────────────
    /// Standard deviation of gaussian noise applied to each heritable trait
    /// during binary fission. Set to 0.0 to disable mutation. Must be >= 0.0.
    /// Default: 0.05.
    #[serde(default = "default_mutation_stddev")]
    pub mutation_stddev: f32,

    /// Minimum clamp bound for heritable `consumption_rate`. Must be > 0.0.
    #[serde(default = "default_trait_consumption_rate_min")]
    pub trait_consumption_rate_min: f32,
    /// Maximum clamp bound for heritable `consumption_rate`.
    #[serde(default = "default_trait_consumption_rate_max")]
    pub trait_consumption_rate_max: f32,

    /// Minimum clamp bound for heritable `base_energy_decay`. Must be > 0.0.
    #[serde(default = "default_trait_base_energy_decay_min")]
    pub trait_base_energy_decay_min: f32,
    /// Maximum clamp bound for heritable `base_energy_decay`.
    #[serde(default = "default_trait_base_energy_decay_max")]
    pub trait_base_energy_decay_max: f32,

    /// Minimum clamp bound for heritable `levy_exponent`. Must be > 1.0.
    #[serde(default = "default_trait_levy_exponent_min")]
    pub trait_levy_exponent_min: f32,
    /// Maximum clamp bound for heritable `levy_exponent`.
    #[serde(default = "default_trait_levy_exponent_max")]
    pub trait_levy_exponent_max: f32,

    /// Minimum clamp bound for heritable `reproduction_threshold`. Must be > 0.0.
    #[serde(default = "default_trait_reproduction_threshold_min")]
    pub trait_reproduction_threshold_min: f32,
    /// Maximum clamp bound for heritable `reproduction_threshold`.
    #[serde(default = "default_trait_reproduction_threshold_max")]
    pub trait_reproduction_threshold_max: f32,

    /// Minimum clamp bound for heritable `max_tumble_steps`. Must be >= 1.
    #[serde(default = "default_trait_max_tumble_steps_min")]
    pub trait_max_tumble_steps_min: u16,
    /// Maximum clamp bound for heritable `max_tumble_steps`.
    #[serde(default = "default_trait_max_tumble_steps_max")]
    pub trait_max_tumble_steps_max: u16,

    /// Minimum clamp bound for heritable `reproduction_cost`. Must be > 0.0.
    #[serde(default = "default_trait_reproduction_cost_min")]
    pub trait_reproduction_cost_min: f32,
    /// Maximum clamp bound for heritable `reproduction_cost`.
    #[serde(default = "default_trait_reproduction_cost_max")]
    pub trait_reproduction_cost_max: f32,

    /// Minimum clamp bound for heritable `offspring_energy`. Must be > 0.0.
    #[serde(default = "default_trait_offspring_energy_min")]
    pub trait_offspring_energy_min: f32,
    /// Maximum clamp bound for heritable `offspring_energy`.
    #[serde(default = "default_trait_offspring_energy_max")]
    pub trait_offspring_energy_max: f32,

    /// Minimum clamp bound for heritable `mutation_rate`. Must be > 0.0.
    #[serde(default = "default_trait_mutation_rate_min")]
    pub trait_mutation_rate_min: f32,
    /// Maximum clamp bound for heritable `mutation_rate`.
    #[serde(default = "default_trait_mutation_rate_max")]
    pub trait_mutation_rate_max: f32,
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
            mutation_stddev: default_mutation_stddev(),
            trait_consumption_rate_min: default_trait_consumption_rate_min(),
            trait_consumption_rate_max: default_trait_consumption_rate_max(),
            trait_base_energy_decay_min: default_trait_base_energy_decay_min(),
            trait_base_energy_decay_max: default_trait_base_energy_decay_max(),
            trait_levy_exponent_min: default_trait_levy_exponent_min(),
            trait_levy_exponent_max: default_trait_levy_exponent_max(),
            trait_reproduction_threshold_min: default_trait_reproduction_threshold_min(),
            trait_reproduction_threshold_max: default_trait_reproduction_threshold_max(),
            trait_max_tumble_steps_min: default_trait_max_tumble_steps_min(),
            trait_max_tumble_steps_max: default_trait_max_tumble_steps_max(),
            trait_reproduction_cost_min: default_trait_reproduction_cost_min(),
            trait_reproduction_cost_max: default_trait_reproduction_cost_max(),
            trait_offspring_energy_min: default_trait_offspring_energy_min(),
            trait_offspring_energy_max: default_trait_offspring_energy_max(),
            trait_mutation_rate_min: default_trait_mutation_rate_min(),
            trait_mutation_rate_max: default_trait_mutation_rate_max(),
        }
    }
}
