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
        }
    }
}

/// Extended top-level config for the Bevy binary.
///
/// Uses `#[serde(flatten)]` so that `seed`, `grid`, `world_init`, and `actor`
/// live at the TOML root alongside the `[bevy]` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BevyWorldConfig {
    #[serde(flatten)]
    pub world: WorldConfig,
    #[serde(default)]
    pub bevy: BevyExtras,
}

impl Default for BevyWorldConfig {
    fn default() -> Self {
        Self {
            world: WorldConfig::default(),
            bevy: BevyExtras::default(),
        }
    }
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
    }

    Ok(())
}
