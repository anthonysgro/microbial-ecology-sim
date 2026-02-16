// Bevy resources and marker components for the grid visualization.
//
// COLD: Defined once at startup, mutated per-tick (SimulationState)
// or per-frame (RenderState, ActiveOverlay). No hot-path allocation
// constraints on the struct definitions themselves — allocation
// discipline is enforced at the usage sites.

use bevy::prelude::{Component, Resource};

use crate::grid::Grid;
use crate::grid::actor_config::ActorConfig;
use crate::grid::config::GridConfig;
use crate::grid::world_init::WorldInitConfig;

// ── Resources ──────────────────────────────────────────────────────

/// Wraps the simulation state. Inserted as a Bevy resource.
///
/// WARM: accessed every fixed tick (mutation) and every render frame (read).
#[derive(Resource)]
pub struct SimulationState {
    pub grid: Grid,
    pub config: GridConfig,
    pub tick: u64,
    pub running: bool,
}

/// Pre-allocated buffers for the render path. Inserted as a Bevy resource.
///
/// WARM: accessed every render frame. Zero allocations after init.
/// `pixel_buffer` length = width * height * 4 (RGBA).
/// `norm_buffer` length = width * height.
#[derive(Resource)]
pub struct RenderState {
    pub pixel_buffer: Vec<u8>,
    pub norm_buffer: Vec<f32>,
}

/// Controls simulation pacing: tick rate, pause state, and reset baseline.
///
/// COLD: mutated only on user key press (rate_control_input system).
/// Read per fixed-tick (tick_simulation pause guard) and per-frame
/// (update_rate_label). No hot-path allocation.
///
/// Invariant: `MIN_HZ <= tick_hz <= MAX_HZ` after any public method call.
/// `initial_tick_hz` is immutable after construction.
///
/// Requirements: 1.1, 1.2, 1.3, 1.4
#[derive(Resource)]
pub struct SimRateController {
    /// Current simulation ticks per second.
    pub tick_hz: f64,
    /// Whether the user has paused the simulation.
    pub paused: bool,
    /// Initial tick_hz from startup config, used for reset.
    pub initial_tick_hz: f64,
}

impl SimRateController {
    pub const MIN_HZ: f64 = 0.5;
    pub const MAX_HZ: f64 = 2048.0;

    pub fn new(tick_hz: f64) -> Self {
        Self {
            tick_hz,
            paused: false,
            initial_tick_hz: tick_hz,
        }
    }

    /// Double the tick rate, clamping to MAX_HZ.
    pub fn speed_up(&mut self) {
        self.tick_hz = (self.tick_hz * 2.0).min(Self::MAX_HZ);
    }

    /// Halve the tick rate, clamping to MIN_HZ.
    pub fn slow_down(&mut self) {
        self.tick_hz = (self.tick_hz / 2.0).max(Self::MIN_HZ);
    }

    /// Reset to the initial tick rate.
    pub fn reset(&mut self) {
        self.tick_hz = self.initial_tick_hz;
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }
}

/// Current overlay selection. Inserted as a Bevy resource.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveOverlay {
    Heat,
    Chemical(usize),
}

/// Configuration for the Bevy visualization app.
///
/// Inserted as a resource before the startup system runs.
/// Consumed during setup to initialize the simulation and configure
/// camera/tick parameters.
#[derive(Resource)]
pub struct BevyVizConfig {
    pub seed: u64,
    pub grid_config: GridConfig,
    pub init_config: WorldInitConfig,
    pub actor_config: Option<ActorConfig>,
    pub initial_overlay: ActiveOverlay,
    /// Simulation ticks per second (drives FixedUpdate timestep).
    pub tick_hz: f64,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub zoom_speed: f32,
    pub pan_speed: f32,
    /// Fixed upper bound for color mapping. Raw values are divided by this
    /// instead of the dynamic max, so the color scale stays stable.
    /// Values above this render as full intensity.
    pub color_scale_max: f32,
    /// Number of simulation ticks between stats recomputations.
    /// 0 or 1 means every tick (no throttling). Default: 10.
    pub stats_update_interval: u64,
}

// ── Marker Components ──────────────────────────────────────────────

/// Marker for the grid texture sprite entity.
#[derive(Component)]
pub struct GridSprite;

/// Marker for the overlay label UI text entity.
#[derive(Component)]
pub struct OverlayLabel;

/// Marker for the main camera entity.
#[derive(Component)]
pub struct MainCamera;

/// Marker for the hover tooltip text entity.
#[derive(Component)]
pub struct HoverTooltip;

/// Marker for the color scale bar image entity.
#[derive(Component)]
pub struct ScaleBar;

/// Marker for the scale bar "0" label.
#[derive(Component)]
pub struct ScaleMinLabel;

/// Marker for the scale bar max label.
#[derive(Component)]
pub struct ScaleMaxLabel;

/// Marker for the simulation rate display text entity.
///
/// Requirements: 6.1, 6.3
#[derive(Component)]
pub struct RateLabel;

/// Marker for the config info panel text entity.
///
/// COLD: Only queried when `InfoPanelVisible` changes.
/// Requirements: 5.1
#[derive(Component)]
pub struct InfoPanel;

/// Tracks whether the info panel is shown or hidden.
///
/// COLD: Mutated only on `I` key press.
/// Default: hidden (`false`).
/// Requirements: 5.2, 1.2
#[derive(Resource)]
pub struct InfoPanelVisible(pub bool);

// ── Trait Visualization Resources ──────────────────────────────────

/// Per-trait aggregate statistics (min, max, mean, percentiles).
///
/// Plain data struct — no methods, no business logic.
/// Used as an element of `TraitStats::traits`.
#[derive(Debug, Clone, Copy)]
pub struct SingleTraitStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
    pub std_dev: f32,
}

/// Pre-computed population statistics for heritable traits.
///
/// Recomputed every tick in `FixedUpdate` by `compute_trait_stats`.
/// COLD path — heap allocation during computation is acceptable.
///
/// Array order: [consumption_rate, base_energy_decay, levy_exponent,
/// reproduction_threshold, max_tumble_steps, reproduction_cost,
/// offspring_energy, mutation_rate, kin_tolerance, optimal_temp,
/// reproduction_cooldown, kin_group_defense].
///
/// Requirements: 1.2, 1.3, 7.1, 7.5, 8.1
#[derive(Resource, Debug, Clone)]
pub struct TraitStats {
    pub actor_count: usize,
    pub tick: u64,
    /// `None` when `actor_count == 0`.
    pub traits: Option<[SingleTraitStats; 12]>,
    /// Population energy statistics. `None` when `actor_count == 0`.
    pub energy_stats: Option<SingleTraitStats>,
}

/// Tracks the currently selected actor for inspection (by slot index).
///
/// `None` = no selection. Default: `None`.
///
/// Requirements: 3.1
#[derive(Resource, Default)]
pub struct SelectedActor(pub Option<usize>);

/// Tracks whether the population stats panel is visible.
///
/// Default: hidden (`false`).
#[derive(Resource)]
pub struct StatsPanelVisible(pub bool);

// ── Trait Visualization Marker Components ──────────────────────────

/// Marker for the population stats panel text entity.
#[derive(Component)]
pub struct StatsPanel;

/// Marker for the actor inspector panel text entity.
#[derive(Component)]
pub struct ActorInspector;

// ── Predation Counter Resource ──────────────────────────────────────

/// Tracks per-tick and cumulative predation events for HUD display.
///
/// COLD: Updated once per tick in `tick_simulation`. Read by `update_stats_panel`.
///
/// Requirements: 2.1, 2.2
#[derive(Resource, Default)]
pub struct PredationCounter {
    /// Predation events in the most recent tick.
    pub last_tick: usize,
    /// Cumulative predation events since simulation start.
    pub total: u64,
}

// ── Stats Throttle Resource ────────────────────────────────────────

/// Tick counter for throttling `compute_trait_stats` recomputation.
///
/// Inserted at startup. Mutated only by `compute_trait_stats`.
/// When `interval <= 1`, stats recompute every tick (no throttling).
#[derive(Resource)]
pub struct StatsTickCounter {
    pub ticks_since_update: u64,
    pub interval: u64,
}
