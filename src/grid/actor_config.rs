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
fn default_base_movement_cost() -> f32 { 0.5 }
fn default_reference_energy() -> f32 { 25.0 }
fn default_absorption_efficiency() -> f32 { 0.5 }
fn default_kin_tolerance() -> f32 { 0.5 }
fn default_trait_kin_tolerance_min() -> f32 { 0.0 }
fn default_trait_kin_tolerance_max() -> f32 { 1.0 }
fn default_reference_metabolic_rate() -> f32 { 0.05 }
fn default_thermal_sensitivity() -> f32 { 0.01 }
fn default_optimal_temp() -> f32 { 0.5 }
fn default_trait_optimal_temp_min() -> f32 { 0.0 }
fn default_trait_optimal_temp_max() -> f32 { 2.0 }

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
    /// Base energy cost for movement at the reference energy level.
    /// Actual cost scales proportionally with actor energy:
    /// `max(base_movement_cost * (energy / reference_energy), base_movement_cost * 0.1)`.
    /// Must be >= 0.0. Default: 0.5.
    #[serde(default = "default_base_movement_cost")]
    pub base_movement_cost: f32,
    /// Energy level at which movement cost equals `base_movement_cost`.
    /// Actors above this pay more; actors below pay less.
    /// Must be > 0.0. Default: 25.0.
    #[serde(default = "default_reference_energy")]
    pub reference_energy: f32,
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

    // ── Contact predation config ───────────────────────────────────
    /// Fraction of prey energy absorbed by predator on successful predation.
    /// Must be in (0.0, 1.0]. Default: 0.5.
    #[serde(default = "default_absorption_efficiency")]
    pub absorption_efficiency: f32,

    /// Seed genome default for kin_tolerance. Default: 0.5.
    #[serde(default = "default_kin_tolerance")]
    pub kin_tolerance: f32,

    /// Minimum clamp bound for heritable `kin_tolerance`. Default: 0.0.
    #[serde(default = "default_trait_kin_tolerance_min")]
    pub trait_kin_tolerance_min: f32,
    /// Maximum clamp bound for heritable `kin_tolerance`. Default: 1.0.
    #[serde(default = "default_trait_kin_tolerance_max")]
    pub trait_kin_tolerance_max: f32,

    // ── Thermal metabolism config ──────────────────────────────────
    /// Quadratic penalty coefficient for thermal mismatch.
    /// Extra energy cost per tick = thermal_sensitivity * (cell_heat - optimal_temp)^2.
    /// Must be >= 0.0 and finite. Default: 0.01.
    #[serde(default = "default_thermal_sensitivity")]
    pub thermal_sensitivity: f32,

    /// Seed genome default for heritable `optimal_temp` trait.
    /// Must be within [trait_optimal_temp_min, trait_optimal_temp_max]. Default: 0.5.
    #[serde(default = "default_optimal_temp")]
    pub optimal_temp: f32,

    /// Minimum clamp bound for heritable `optimal_temp`. Default: 0.0.
    #[serde(default = "default_trait_optimal_temp_min")]
    pub trait_optimal_temp_min: f32,
    /// Maximum clamp bound for heritable `optimal_temp`. Default: 2.0.
    #[serde(default = "default_trait_optimal_temp_max")]
    pub trait_optimal_temp_max: f32,

    // ── Metabolic scaling config ───────────────────────────────────
    /// Metabolic rate at which all scaling multipliers equal 1.0.
    /// Actors with base_energy_decay above this value gain enhanced
    /// consumption, cheaper movement, and stronger predation.
    /// Actors below this value get the inverse.
    /// Must be > 0.0 and finite. Default: 0.05.
    #[serde(default = "default_reference_metabolic_rate")]
    pub reference_metabolic_rate: f32,
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
            base_movement_cost: default_base_movement_cost(),
            reference_energy: default_reference_energy(),
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
            absorption_efficiency: default_absorption_efficiency(),
            kin_tolerance: default_kin_tolerance(),
            trait_kin_tolerance_min: default_trait_kin_tolerance_min(),
            trait_kin_tolerance_max: default_trait_kin_tolerance_max(),
            thermal_sensitivity: default_thermal_sensitivity(),
            optimal_temp: default_optimal_temp(),
            trait_optimal_temp_min: default_trait_optimal_temp_min(),
            trait_optimal_temp_max: default_trait_optimal_temp_max(),
            reference_metabolic_rate: default_reference_metabolic_rate(),
        }
    }
}
