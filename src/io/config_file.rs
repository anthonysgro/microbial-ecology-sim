// COLD PATH: TOML configuration file loading, validation, and serialization.
// Runs once at startup. Allocations and dynamic dispatch permitted.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::grid::actor_config::ActorConfig;
use crate::grid::config::GridConfig;
use crate::grid::world_init::{self, WorldInitConfig};
use crate::io::config_error::ConfigError;

// ── Default value functions for serde ──────────────────────────────

fn default_seed() -> u64 {
    42
}

fn default_tick_hz() -> f64 {
    10.0
}

fn default_zoom_min() -> f32 {
    0.1
}

fn default_zoom_max() -> f32 {
    10.0
}

fn default_zoom_speed() -> f32 {
    0.1
}

fn default_pan_speed() -> f32 {
    1.0
}

fn default_color_scale_max() -> f32 {
    10.0
}

fn default_stats_update_interval() -> u64 {
    10
}

// ── Top-level config structs ───────────────────────────────────────

/// Top-level configuration aggregating seed and all simulation sub-configs.
///
/// Maps directly to a TOML document. Omitted fields fall back to compiled
/// defaults via `#[serde(default)]`. Unknown keys are rejected at parse time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldConfig {
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default)]
    pub grid: GridConfig,
    #[serde(default)]
    pub world_init: WorldInitConfig,
    #[serde(default)]
    pub actor: Option<ActorConfig>,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            seed: default_seed(),
            grid: GridConfig::default(),
            world_init: WorldInitConfig::default(),
            actor: None,
        }
    }
}

/// Bevy-specific configuration fields. Only consumed by the Bevy binary.
///
/// Deserialized from an optional `[bevy]` TOML section. All fields have
/// compiled defaults matching the current hardcoded values in `bevy_viz.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyExtras {
    #[serde(default = "default_tick_hz")]
    pub tick_hz: f64,
    #[serde(default = "default_zoom_min")]
    pub zoom_min: f32,
    #[serde(default = "default_zoom_max")]
    pub zoom_max: f32,
    #[serde(default = "default_zoom_speed")]
    pub zoom_speed: f32,
    #[serde(default = "default_pan_speed")]
    pub pan_speed: f32,
    #[serde(default = "default_color_scale_max")]
    pub color_scale_max: f32,
    #[serde(default = "default_stats_update_interval")]
    pub stats_update_interval: u64,
}

impl Default for BevyExtras {
    fn default() -> Self {
        Self {
            tick_hz: default_tick_hz(),
            zoom_min: default_zoom_min(),
            zoom_max: default_zoom_max(),
            zoom_speed: default_zoom_speed(),
            pan_speed: default_pan_speed(),
            color_scale_max: default_color_scale_max(),
            stats_update_interval: default_stats_update_interval(),
        }
    }
}

/// Extended top-level config for the Bevy binary.
///
/// Uses `#[serde(flatten)]` so that `seed`, `grid`, `world_init`, and `actor`
/// live at the TOML root alongside the `[bevy]` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BevyWorldConfig {
    #[serde(flatten)]
    pub world: WorldConfig,
    #[serde(default)]
    pub bevy: BevyExtras,
}

// ── Public API ─────────────────────────────────────────────────────

/// Load a `WorldConfig` from a TOML file at `path`.
///
/// Returns `ConfigError::Io` on filesystem errors, `ConfigError::Parse`
/// on malformed TOML or unknown keys (via `deny_unknown_fields`).
pub fn load_world_config(path: &Path) -> Result<WorldConfig, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let config: WorldConfig = toml::from_str(&contents)?;
    Ok(config)
}

/// Load a `BevyWorldConfig` from a TOML file at `path`.
///
/// Same error semantics as `load_world_config`. The `[bevy]` section is
/// optional — omitted fields fall back to `BevyExtras::default()`.
pub fn load_bevy_config(path: &Path) -> Result<BevyWorldConfig, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let config: BevyWorldConfig = toml::from_str(&contents)?;
    Ok(config)
}

/// Serialize a `WorldConfig` to a pretty-printed TOML string.
pub fn to_toml_string(config: &WorldConfig) -> Result<String, ConfigError> {
    let s = toml::to_string_pretty(config)?;
    Ok(s)
}

/// Validate cross-field invariants on a `WorldConfig`.
///
/// Checks performed (in order):
/// 1. `world_init` range validation via `world_init::validate_config`.
/// 2. `chemical_decay_rates.len() == num_chemicals`.
/// 3. `actor.removal_threshold <= 0.0` (if actor config present).
pub fn validate_world_config(config: &WorldConfig) -> Result<(), ConfigError> {
    // 1. Existing range checks on WorldInitConfig.
    world_init::validate_config(&config.world_init).map_err(|e| ConfigError::Validation {
        reason: e.to_string(),
    })?;

    // 2. Decay rates length must match num_chemicals.
    if config.grid.chemical_decay_rates.len() != config.grid.num_chemicals {
        return Err(ConfigError::Validation {
            reason: format!(
                "chemical_decay_rates length ({}) does not match num_chemicals ({})",
                config.grid.chemical_decay_rates.len(),
                config.grid.num_chemicals,
            ),
        });
    }

    // 3. Actor config validation.
    if let Some(ref actor) = config.actor {
        if actor.removal_threshold > 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "removal_threshold ({}) must be <= 0.0",
                    actor.removal_threshold,
                ),
            });
        }

        // 3a. max_energy must be positive and finite.
        if actor.max_energy <= 0.0 || !actor.max_energy.is_finite() {
            return Err(ConfigError::Validation {
                reason: format!(
                    "max_energy ({}) must be > 0.0 and finite",
                    actor.max_energy,
                ),
            });
        }

        // 3b. initial_energy must not exceed max_energy.
        if actor.initial_energy > actor.max_energy {
            return Err(ConfigError::Validation {
                reason: format!(
                    "initial_energy ({}) must be <= max_energy ({})",
                    actor.initial_energy, actor.max_energy,
                ),
            });
        }

        // 3c. extraction_cost must be non-negative.
        if actor.extraction_cost < 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "extraction_cost ({}) must be >= 0.0",
                    actor.extraction_cost,
                ),
            });
        }

        // 3d. extraction_cost must be strictly less than energy_conversion_factor.
        if actor.extraction_cost >= actor.energy_conversion_factor {
            return Err(ConfigError::Validation {
                reason: format!(
                    "extraction_cost ({}) must be < energy_conversion_factor ({})",
                    actor.extraction_cost, actor.energy_conversion_factor,
                ),
            });
        }

        // 3e. levy_exponent must be > 1.0.
        if actor.levy_exponent <= 1.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "levy_exponent ({}) must be > 1.0",
                    actor.levy_exponent,
                ),
            });
        }

        // 3f. max_tumble_steps must be >= 1.
        if actor.max_tumble_steps < 1 {
            return Err(ConfigError::Validation {
                reason: "max_tumble_steps must be >= 1".to_string(),
            });
        }

        // 3g. mutation_stddev must be non-negative.
        if actor.mutation_stddev < 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "mutation_stddev ({}) must be >= 0.0",
                    actor.mutation_stddev,
                ),
            });
        }

        // 3h. Trait clamp ranges (f32): min must be strictly less than max.
        let clamp_ranges: [(&str, f32, f32); 8] = [
            ("trait_consumption_rate", actor.trait_consumption_rate_min, actor.trait_consumption_rate_max),
            ("trait_base_energy_decay", actor.trait_base_energy_decay_min, actor.trait_base_energy_decay_max),
            ("trait_levy_exponent", actor.trait_levy_exponent_min, actor.trait_levy_exponent_max),
            ("trait_reproduction_threshold", actor.trait_reproduction_threshold_min, actor.trait_reproduction_threshold_max),
            ("trait_reproduction_cost", actor.trait_reproduction_cost_min, actor.trait_reproduction_cost_max),
            ("trait_offspring_energy", actor.trait_offspring_energy_min, actor.trait_offspring_energy_max),
            ("trait_mutation_rate", actor.trait_mutation_rate_min, actor.trait_mutation_rate_max),
            ("trait_kin_tolerance", actor.trait_kin_tolerance_min, actor.trait_kin_tolerance_max),
        ];
        for (name, min, max) in &clamp_ranges {
            if min >= max {
                return Err(ConfigError::Validation {
                    reason: format!(
                        "{name}_min ({min}) must be < {name}_max ({max})",
                    ),
                });
            }
        }

        // 3h-2. Trait clamp range for max_tumble_steps (u16).
        if actor.trait_max_tumble_steps_min < 1 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_max_tumble_steps_min ({}) must be >= 1",
                    actor.trait_max_tumble_steps_min,
                ),
            });
        }
        if actor.trait_max_tumble_steps_min >= actor.trait_max_tumble_steps_max {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_max_tumble_steps_min ({}) must be < trait_max_tumble_steps_max ({})",
                    actor.trait_max_tumble_steps_min, actor.trait_max_tumble_steps_max,
                ),
            });
        }

        // 3i. Trait-specific lower-bound constraints.
        if actor.trait_consumption_rate_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_consumption_rate_min ({}) must be > 0.0",
                    actor.trait_consumption_rate_min,
                ),
            });
        }
        if actor.trait_base_energy_decay_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_base_energy_decay_min ({}) must be > 0.0",
                    actor.trait_base_energy_decay_min,
                ),
            });
        }
        if actor.trait_levy_exponent_min <= 1.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_levy_exponent_min ({}) must be > 1.0",
                    actor.trait_levy_exponent_min,
                ),
            });
        }
        if actor.trait_reproduction_threshold_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_reproduction_threshold_min ({}) must be > 0.0",
                    actor.trait_reproduction_threshold_min,
                ),
            });
        }
        if actor.trait_reproduction_cost_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_reproduction_cost_min ({}) must be > 0.0",
                    actor.trait_reproduction_cost_min,
                ),
            });
        }
        if actor.trait_offspring_energy_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_offspring_energy_min ({}) must be > 0.0",
                    actor.trait_offspring_energy_min,
                ),
            });
        }
        if actor.trait_mutation_rate_min <= 0.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_mutation_rate_min ({}) must be > 0.0",
                    actor.trait_mutation_rate_min,
                ),
            });
        }

        // 3i-2. trait_offspring_energy_max must not exceed max_energy.
        if actor.trait_offspring_energy_max > actor.max_energy {
            return Err(ConfigError::Validation {
                reason: format!(
                    "trait_offspring_energy_max ({}) must be <= max_energy ({})",
                    actor.trait_offspring_energy_max, actor.max_energy,
                ),
            });
        }

        // 3j. Default heritable field values must fall within their clamp ranges.
        if actor.consumption_rate < actor.trait_consumption_rate_min
            || actor.consumption_rate > actor.trait_consumption_rate_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "consumption_rate ({}) must be within trait clamp range [{}, {}]",
                    actor.consumption_rate,
                    actor.trait_consumption_rate_min,
                    actor.trait_consumption_rate_max,
                ),
            });
        }
        if actor.base_energy_decay < actor.trait_base_energy_decay_min
            || actor.base_energy_decay > actor.trait_base_energy_decay_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "base_energy_decay ({}) must be within trait clamp range [{}, {}]",
                    actor.base_energy_decay,
                    actor.trait_base_energy_decay_min,
                    actor.trait_base_energy_decay_max,
                ),
            });
        }
        if actor.levy_exponent < actor.trait_levy_exponent_min
            || actor.levy_exponent > actor.trait_levy_exponent_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "levy_exponent ({}) must be within trait clamp range [{}, {}]",
                    actor.levy_exponent,
                    actor.trait_levy_exponent_min,
                    actor.trait_levy_exponent_max,
                ),
            });
        }
        if actor.reproduction_threshold < actor.trait_reproduction_threshold_min
            || actor.reproduction_threshold > actor.trait_reproduction_threshold_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "reproduction_threshold ({}) must be within trait clamp range [{}, {}]",
                    actor.reproduction_threshold,
                    actor.trait_reproduction_threshold_min,
                    actor.trait_reproduction_threshold_max,
                ),
            });
        }
        if actor.max_tumble_steps < actor.trait_max_tumble_steps_min
            || actor.max_tumble_steps > actor.trait_max_tumble_steps_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "max_tumble_steps ({}) must be within trait clamp range [{}, {}]",
                    actor.max_tumble_steps,
                    actor.trait_max_tumble_steps_min,
                    actor.trait_max_tumble_steps_max,
                ),
            });
        }
        if actor.reproduction_cost < actor.trait_reproduction_cost_min
            || actor.reproduction_cost > actor.trait_reproduction_cost_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "reproduction_cost ({}) must be within trait clamp range [{}, {}]",
                    actor.reproduction_cost,
                    actor.trait_reproduction_cost_min,
                    actor.trait_reproduction_cost_max,
                ),
            });
        }
        if actor.offspring_energy < actor.trait_offspring_energy_min
            || actor.offspring_energy > actor.trait_offspring_energy_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "offspring_energy ({}) must be within trait clamp range [{}, {}]",
                    actor.offspring_energy,
                    actor.trait_offspring_energy_min,
                    actor.trait_offspring_energy_max,
                ),
            });
        }
        if actor.mutation_stddev < actor.trait_mutation_rate_min
            || actor.mutation_stddev > actor.trait_mutation_rate_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "mutation_stddev ({}) must be within trait clamp range [{}, {}]",
                    actor.mutation_stddev,
                    actor.trait_mutation_rate_min,
                    actor.trait_mutation_rate_max,
                ),
            });
        }

        // 3k. absorption_efficiency must be in (0.0, 1.0].
        if actor.absorption_efficiency <= 0.0 || actor.absorption_efficiency > 1.0 {
            return Err(ConfigError::Validation {
                reason: format!(
                    "absorption_efficiency ({}) must be in (0.0, 1.0]",
                    actor.absorption_efficiency,
                ),
            });
        }

        // 3l. kin_tolerance seed value must be within clamp range.
        if actor.kin_tolerance < actor.trait_kin_tolerance_min
            || actor.kin_tolerance > actor.trait_kin_tolerance_max
        {
            return Err(ConfigError::Validation {
                reason: format!(
                    "kin_tolerance ({}) must be within trait clamp range [{}, {}]",
                    actor.kin_tolerance,
                    actor.trait_kin_tolerance_min,
                    actor.trait_kin_tolerance_max,
                ),
            });
        }
    }

    Ok(())
}
