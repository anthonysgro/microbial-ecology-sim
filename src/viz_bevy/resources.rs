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
