// COLD PATH: Startup system — runs once to initialize simulation,
// spawn camera, sprite, and overlay label.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::grid::actor_config::ActorConfig;
use crate::grid::config::GridConfig;
use crate::grid::world_init;
use crate::grid::world_init::WorldInitConfig;

use super::resources::{
    ActiveOverlay, ActorInspector, BevyVizConfig, GridSprite, HoverTooltip, InfoPanel,
    InfoPanelVisible, MainCamera, OverlayLabel, PredationCounter, RateLabel, RenderState,
    ScaleBar, ScaleMaxLabel, ScaleMinLabel, SelectedActor, SimRateController, SimulationState,
    StatsPanel, StatsPanelVisible, StatsTickCounter, TraitStats,
};

/// Format the overlay name for the UI label.
pub(super) fn overlay_label_text(overlay: &ActiveOverlay) -> String {
    match overlay {
        ActiveOverlay::Heat => "Heat".to_string(),
        ActiveOverlay::Chemical(n) => format!("Chemical {n}"),
    }
}

/// Trait names in display order, matching `TraitStats::traits` array indices.
const TRAIT_NAMES: [&str; 15] = [
    "consumption_rate",
    "base_energy_decay",
    "levy_exponent",
    "reproduction_thresh",
    "max_tumble_steps",
    "reproduction_cost",
    "offspring_energy",
    "mutation_rate",
    "kin_tolerance",
    "optimal_temp",
    "repro_cooldown",
    "kin_group_defense",
    "memory_capacity",
    "site_fidelity",
    "avoid_sensitivity",
];

/// Format `TraitStats` into a display string for the stats panel.
///
/// Pure function — no Bevy dependencies, testable in isolation.
/// Uses a header row with column labels and a `spread` column showing
/// `std_dev / (trait_max - trait_min)` as a percentage of the configured range.
///
/// When `traits` is `None` (zero living actors), returns a short
/// "No living actors." message with tick number.
///
/// Requirements: 2.2, 2.3
pub fn format_trait_stats(
    stats: &super::resources::TraitStats,
    predation: &super::resources::PredationCounter,
    actor_config: Option<&ActorConfig>,
) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    writeln!(
        out,
        "Tick: {}  |  Actors: {}  |  Predations: {} (total: {})",
        stats.tick, stats.actor_count, predation.last_tick, predation.total,
    )
    .ok();

    let Some(ref traits) = stats.traits else {
        writeln!(out, "\nNo living actors.").ok();
        return out;
    };

    // Trait ranges from config, matching TRAIT_NAMES order.
    // Each entry is (min, max) for computing spread = std_dev / (max - min).
    // Energy row uses (0.0, max_energy) as its range.
    let trait_ranges: Option<[(f32, f32); 15]> = actor_config.map(|c| [
        (c.trait_consumption_rate_min, c.trait_consumption_rate_max),
        (c.trait_base_energy_decay_min, c.trait_base_energy_decay_max),
        (c.trait_levy_exponent_min, c.trait_levy_exponent_max),
        (c.trait_reproduction_threshold_min, c.trait_reproduction_threshold_max),
        (c.trait_max_tumble_steps_min as f32, c.trait_max_tumble_steps_max as f32),
        (c.trait_reproduction_cost_min, c.trait_reproduction_cost_max),
        (c.trait_offspring_energy_min, c.trait_offspring_energy_max),
        (c.trait_mutation_rate_min, c.trait_mutation_rate_max),
        (c.trait_kin_tolerance_min, c.trait_kin_tolerance_max),
        (c.trait_optimal_temp_min, c.trait_optimal_temp_max),
        (c.trait_reproduction_cooldown_min as f32, c.trait_reproduction_cooldown_max as f32),
        (c.trait_kin_group_defense_min, c.trait_kin_group_defense_max),
        (c.trait_memory_capacity_min as f32, c.trait_memory_capacity_max as f32),
        (c.trait_site_fidelity_strength_min, c.trait_site_fidelity_strength_max),
        (c.trait_avoidance_sensitivity_min, c.trait_avoidance_sensitivity_max),
    ]);

    // Header row
    writeln!(out).ok();
    writeln!(
        out,
        "{:<20} {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
        "", "min", "p25", "p50", "p75", "max", "mean", "spread"
    ).ok();

    for (i, name) in TRAIT_NAMES.iter().enumerate() {
        let s = &traits[i];
        let spread = trait_ranges
            .as_ref()
            .map(|r| {
                let range = r[i].1 - r[i].0;
                if range > 0.0 { format!("{:>5.1}%", (s.std_dev / range) * 100.0) } else { "    —".to_string() }
            })
            .unwrap_or_else(|| "    —".to_string());
        writeln!(
            out,
            "{:<20} {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {}",
            name, s.min, s.p25, s.p50, s.p75, s.max, s.mean, spread,
        ).ok();
    }

    if let Some(ref energy) = stats.energy_stats {
        let spread = actor_config
            .map(|c| {
                if c.max_energy > 0.0 { format!("{:>5.1}%", (energy.std_dev / c.max_energy) * 100.0) } else { "    —".to_string() }
            })
            .unwrap_or_else(|| "    —".to_string());
        writeln!(
            out,
            "{:<20} {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {:>6.2}  {}",
            "energy", energy.min, energy.p25, energy.p50, energy.p75, energy.max, energy.mean, spread,
        ).ok();
    }

    out
}

/// Format a single selected actor's state into a display string.
///
/// Pure function — no Bevy dependencies, testable in isolation.
/// Energy formatted to 2dp, trait values to 4dp.
/// Grid position derived from `cell_index` and `grid_width`.
///
/// Requirements: 4.1, 4.2
pub fn format_actor_info(
    actor: &crate::grid::actor::Actor,
    slot_index: usize,
    grid_width: u32,
) -> String {
    use std::fmt::Write;

    let col = actor.cell_index % grid_width as usize;
    let row = actor.cell_index / grid_width as usize;
    let state = if actor.inert { "inert" } else { "active" };

    let mut out = String::new();
    writeln!(out, "Actor [slot {slot_index}] — {state}").ok();
    writeln!(out, "Position: ({col}, {row})").ok();
    writeln!(out, "Energy: {:.2}", actor.energy).ok();
    writeln!(out).ok();
    writeln!(out, "consumption_rate:        {:.4}", actor.traits.consumption_rate).ok();
    writeln!(out, "base_energy_decay:       {:.4}", actor.traits.base_energy_decay).ok();
    writeln!(out, "levy_exponent:           {:.4}", actor.traits.levy_exponent).ok();
    writeln!(out, "reproduction_threshold: {:.4}", actor.traits.reproduction_threshold).ok();
    writeln!(out, "max_tumble_steps:        {}", actor.traits.max_tumble_steps).ok();
    writeln!(out, "reproduction_cost:       {:.4}", actor.traits.reproduction_cost).ok();
    writeln!(out, "offspring_energy:        {:.4}", actor.traits.offspring_energy).ok();
    writeln!(out, "mutation_rate:           {:.4}", actor.traits.mutation_rate).ok();
    writeln!(out, "kin_tolerance:           {:.4}", actor.traits.kin_tolerance).ok();
    writeln!(out, "kin_group_defense:       {:.4}", actor.traits.kin_group_defense).ok();
    writeln!(out, "optimal_temp:            {:.4}", actor.traits.optimal_temp).ok();
    writeln!(out, "reproduction_cooldown:   {}", actor.traits.reproduction_cooldown).ok();
    writeln!(out, "memory_capacity:         {}", actor.traits.memory_capacity).ok();
    writeln!(out, "site_fidelity_strength:  {:.4}", actor.traits.site_fidelity_strength).ok();
    writeln!(out, "avoidance_sensitivity:   {:.4}", actor.traits.avoidance_sensitivity).ok();
    writeln!(out).ok();
    writeln!(out, "cooldown_remaining:      {}", actor.cooldown_remaining).ok();

    out
}



/// Format the full config info panel text from configuration data.
///
/// Pure function — no Bevy dependencies. Testable in isolation.
/// All floats formatted to 4 decimal places for consistency.
///
/// Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3
pub(super) fn format_config_info(
    seed: u64,
    grid_config: &GridConfig,
    init_config: &WorldInitConfig,
    actor_config: Option<&ActorConfig>,
    stats_update_interval: u64,
) -> String {
    use std::fmt::Write;

    let mut out = String::new();

    // ── Seed ───────────────────────────────────────────────────────
    writeln!(out, "--- Seed ---").ok();
    writeln!(out, "seed: {seed}").ok();

    // ── Grid ───────────────────────────────────────────────────────
    writeln!(out, "\n--- Grid ---").ok();
    writeln!(out, "width: {}", grid_config.width).ok();
    writeln!(out, "height: {}", grid_config.height).ok();
    writeln!(out, "num_chemicals: {}", grid_config.num_chemicals).ok();
    writeln!(out, "thermal_conductivity: {:.4}", grid_config.thermal_conductivity).ok();
    writeln!(out, "ambient_heat: {:.4}", grid_config.ambient_heat).ok();
    writeln!(out, "tick_duration: {:.4}", grid_config.tick_duration).ok();
    writeln!(out, "num_threads: {}", grid_config.num_threads).ok();

    // ── World Init ─────────────────────────────────────────────────
    writeln!(out, "\n--- World Init ---").ok();

    // Heat sources
    let hs = &init_config.heat_source_config;
    writeln!(out, "heat sources: {}..{}", hs.min_sources, hs.max_sources).ok();
    writeln!(out, "heat emission_rate: {:.4}..{:.4}", hs.min_emission_rate, hs.max_emission_rate).ok();
    writeln!(out, "heat renewable_fraction: {:.4}", hs.renewable_fraction).ok();
    writeln!(out, "heat reservoir_capacity: {:.4}..{:.4}", hs.min_reservoir_capacity, hs.max_reservoir_capacity).ok();
    writeln!(out, "heat deceleration_threshold: {:.4}..{:.4}", hs.min_deceleration_threshold, hs.max_deceleration_threshold).ok();
    writeln!(out, "heat respawn_enabled: {}", hs.respawn_enabled).ok();
    writeln!(out, "heat respawn_cooldown_ticks: {}..{}", hs.min_respawn_cooldown_ticks, hs.max_respawn_cooldown_ticks).ok();
    writeln!(out, "heat source_clustering: {:.4}", hs.source_clustering).ok();
    writeln!(out, "heat source_dispersion: {:.4}", hs.source_dispersion).ok();

    // Chemical species configs (per-species)
    for (i, cs) in init_config.chemical_species_configs.iter().enumerate() {
        writeln!(out, "\n  chemical species {i}:").ok();
        writeln!(out, "    decay_rate: {:.4}", cs.decay_rate).ok();
        writeln!(out, "    diffusion_rate: {:.4}", cs.diffusion_rate).ok();
        let sc = &cs.source_config;
        writeln!(out, "    sources: {}..{}", sc.min_sources, sc.max_sources).ok();
        writeln!(out, "    emission_rate: {:.4}..{:.4}", sc.min_emission_rate, sc.max_emission_rate).ok();
        writeln!(out, "    renewable_fraction: {:.4}", sc.renewable_fraction).ok();
        writeln!(out, "    reservoir_capacity: {:.4}..{:.4}", sc.min_reservoir_capacity, sc.max_reservoir_capacity).ok();
        writeln!(out, "    deceleration_threshold: {:.4}..{:.4}", sc.min_deceleration_threshold, sc.max_deceleration_threshold).ok();
        writeln!(out, "    respawn_enabled: {}", sc.respawn_enabled).ok();
        writeln!(out, "    respawn_cooldown_ticks: {}..{}", sc.min_respawn_cooldown_ticks, sc.max_respawn_cooldown_ticks).ok();
        writeln!(out, "    source_clustering: {:.4}", sc.source_clustering).ok();
        writeln!(out, "    source_dispersion: {:.4}", sc.source_dispersion).ok();
    }

    // Initial ranges
    writeln!(out, "initial_heat: {:.4}..{:.4}", init_config.min_initial_heat, init_config.max_initial_heat).ok();
    writeln!(out, "initial_concentration: {:.4}..{:.4}", init_config.min_initial_concentration, init_config.max_initial_concentration).ok();

    // Actor range
    writeln!(out, "actors: {}..{}", init_config.min_actors, init_config.max_actors).ok();

    // ── Actors ─────────────────────────────────────────────────────
    writeln!(out, "\n--- Actors ---").ok();
    match actor_config {
        Some(ac) => {
            writeln!(out, "consumption_rate: {:.4}", ac.consumption_rate).ok();
            writeln!(out, "energy_conversion_factor: {:.4}", ac.energy_conversion_factor).ok();
            writeln!(out, "extraction_cost: {:.4}", ac.extraction_cost).ok();
            writeln!(out, "base_energy_decay: {:.4}", ac.base_energy_decay).ok();
            writeln!(out, "reference_metabolic_rate: {:.4}", ac.reference_metabolic_rate).ok();
            writeln!(out, "initial_energy: {:.4}", ac.initial_energy).ok();
            writeln!(out, "initial_actor_capacity: {}", ac.initial_actor_capacity).ok();
            writeln!(out, "base_movement_cost: {:.4}", ac.base_movement_cost).ok();
            writeln!(out, "reference_energy: {:.4}", ac.reference_energy).ok();
            writeln!(out, "removal_threshold: {:.4}", ac.removal_threshold).ok();
            writeln!(out, "max_energy: {:.4}", ac.max_energy).ok();
            writeln!(out, "levy_exponent: {:.4}", ac.levy_exponent).ok();
            writeln!(out, "max_tumble_steps: {}", ac.max_tumble_steps).ok();
            writeln!(out, "reproduction_threshold: {:.4}", ac.reproduction_threshold).ok();
            writeln!(out, "reproduction_cost: {:.4}", ac.reproduction_cost).ok();
            writeln!(out, "offspring_energy: {:.4}", ac.offspring_energy).ok();
            writeln!(out, "mutation_stddev: {:.4}", ac.mutation_stddev).ok();
            writeln!(out, "trait_consumption_rate: {:.4}..{:.4}", ac.trait_consumption_rate_min, ac.trait_consumption_rate_max).ok();
            writeln!(out, "trait_base_energy_decay: {:.4}..{:.4}", ac.trait_base_energy_decay_min, ac.trait_base_energy_decay_max).ok();
            writeln!(out, "trait_levy_exponent: {:.4}..{:.4}", ac.trait_levy_exponent_min, ac.trait_levy_exponent_max).ok();
            writeln!(out, "trait_reproduction_threshold: {:.4}..{:.4}", ac.trait_reproduction_threshold_min, ac.trait_reproduction_threshold_max).ok();
            writeln!(out, "trait_max_tumble_steps: {}..{}", ac.trait_max_tumble_steps_min, ac.trait_max_tumble_steps_max).ok();
            writeln!(out, "trait_reproduction_cost: {:.4}..{:.4}", ac.trait_reproduction_cost_min, ac.trait_reproduction_cost_max).ok();
            writeln!(out, "trait_offspring_energy: {:.4}..{:.4}", ac.trait_offspring_energy_min, ac.trait_offspring_energy_max).ok();
            writeln!(out, "trait_mutation_rate: {:.4}..{:.4}", ac.trait_mutation_rate_min, ac.trait_mutation_rate_max).ok();
            writeln!(out, "absorption_efficiency: {:.4}", ac.absorption_efficiency).ok();
            writeln!(out, "kin_tolerance: {:.4}", ac.kin_tolerance).ok();
            writeln!(out, "trait_kin_tolerance: {:.4}..{:.4}", ac.trait_kin_tolerance_min, ac.trait_kin_tolerance_max).ok();
            writeln!(out, "kin_group_defense: {:.4}", ac.kin_group_defense).ok();
            writeln!(out, "trait_kin_group_defense: {:.4}..{:.4}", ac.trait_kin_group_defense_min, ac.trait_kin_group_defense_max).ok();
            writeln!(out, "thermal_sensitivity: {:.4}", ac.thermal_sensitivity).ok();
            writeln!(out, "optimal_temp: {:.4}", ac.optimal_temp).ok();
            writeln!(out, "trait_optimal_temp: {:.4}..{:.4}", ac.trait_optimal_temp_min, ac.trait_optimal_temp_max).ok();
            writeln!(out, "thermal_fitness_width: {:.4}", ac.thermal_fitness_width).ok();
            writeln!(out, "thermal_movement_cap: {:.4}", ac.thermal_movement_cap).ok();
            writeln!(out, "reproduction_cooldown: {}", ac.reproduction_cooldown).ok();
            writeln!(out, "trait_reproduction_cooldown: {}..{}", ac.trait_reproduction_cooldown_min, ac.trait_reproduction_cooldown_max).ok();
            writeln!(out, "readiness_sensitivity: {:.4}", ac.readiness_sensitivity).ok();
            writeln!(out, "reference_cooldown: {:.4}", ac.reference_cooldown).ok();
            writeln!(out, "memory_capacity: {}", ac.memory_capacity).ok();
            writeln!(out, "trait_memory_capacity: {}..{}", ac.trait_memory_capacity_min, ac.trait_memory_capacity_max).ok();
            writeln!(out, "cognitive_cost_per_slot: {:.4}", ac.cognitive_cost_per_slot).ok();
            writeln!(out, "site_fidelity_strength: {:.4}", ac.site_fidelity_strength).ok();
            writeln!(out, "trait_site_fidelity_strength: {:.4}..{:.4}", ac.trait_site_fidelity_strength_min, ac.trait_site_fidelity_strength_max).ok();
            writeln!(out, "avoidance_sensitivity: {:.4}", ac.avoidance_sensitivity).ok();
            writeln!(out, "trait_avoidance_sensitivity: {:.4}..{:.4}", ac.trait_avoidance_sensitivity_min, ac.trait_avoidance_sensitivity_max).ok();
        }
        None => {
            writeln!(out, "Actors: disabled").ok();
        }
    }

    // ── Bevy ───────────────────────────────────────────────────────
    writeln!(out, "\n--- Bevy ---").ok();
    writeln!(out, "stats_update_interval: {stats_update_interval}").ok();

    out
}

/// Build a vertical gradient image for the color scale bar.
///
/// Top of the image = normalized 1.0 (max), bottom = 0.0 (min).
/// Uses the appropriate color function for the given overlay.
pub(super) fn build_scale_image(
    width: u32,
    height: u32,
    overlay: &ActiveOverlay,
) -> Image {
    use super::color::{chemical_color_rgba, heat_color_rgba};

    let color_fn: fn(f32) -> [u8; 4] = match overlay {
        ActiveOverlay::Heat => heat_color_rgba,
        ActiveOverlay::Chemical(_) => chemical_color_rgba,
    };

    let mut data = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        // Top row = 1.0, bottom row = 0.0.
        let normalized = 1.0 - (y as f32 / (height - 1).max(1) as f32);
        let rgba = color_fn(normalized);
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            data[offset] = rgba[0];
            data[offset + 1] = rgba[1];
            data[offset + 2] = rgba[2];
            data[offset + 3] = rgba[3];
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Startup system: initializes the simulation, pre-allocates render buffers,
/// creates the GPU texture, and spawns camera + sprite + label entities.
///
/// Consumes `BevyVizConfig` (read-only) to derive all initialization parameters.
/// Panics on world-init failure — this is a COLD one-shot path where `expect`
/// is acceptable per project conventions.
pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Res<BevyVizConfig>,
) {
    // ── Initialize simulation ──────────────────────────────────────
    let grid = world_init::initialize(
        config.seed,
        config.grid_config.clone(),
        &config.init_config,
        config.actor_config.clone(),
    )
    .expect("world initialization failed during Bevy setup");

    let width = grid.width();
    let height = grid.height();
    let cell_count = grid.cell_count();

    // ── Insert simulation resource ─────────────────────────────────
    commands.insert_resource(SimulationState {
        config: config.grid_config.clone(),
        grid,
        tick: 0,
        running: true,
    });

    // ── Pre-allocate render buffers (Req 5.4, 9.2) ─────────────────
    commands.insert_resource(RenderState {
        pixel_buffer: vec![0u8; cell_count * 4],
        norm_buffer: vec![0.0f32; cell_count],
    });

    // ── Insert active overlay from config ──────────────────────────
    commands.insert_resource(config.initial_overlay);

    // ── Insert rate controller from config (Req 1.4) ───────────────
    commands.insert_resource(SimRateController::new(config.tick_hz));

    // ── Create GPU texture (Req 5.1) ───────────────────────────────
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();

    let image_handle = images.add(image);

    // ── Spawn camera (Req 8.1) ─────────────────────────────────────
    commands.spawn((Camera2d, MainCamera));

    // ── Spawn grid sprite (Req 5.3) ────────────────────────────────
    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(width as f32, height as f32)),
            ..default()
        },
        GridSprite,
    ));

    // ── Spawn overlay label (Req 7.1, 7.2) ─────────────────────────
    let label_text = overlay_label_text(&config.initial_overlay);
    commands.spawn((
        Text::new(label_text),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        OverlayLabel,
    ));

    // ── Spawn rate label (top-right, Req 6.3) ──────────────────────
    commands.spawn((
        Text::new(format!("{:.1} Hz", config.tick_hz)),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(50.0),
            ..default()
        },
        RateLabel,
    ));

    // ── Spawn hover tooltip (bottom-left) ──────────────────────────
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        HoverTooltip,
    ));

    // ── Spawn color scale bar (right edge) ─────────────────────────
    let scale_height: u32 = 256;
    let scale_width: u32 = 20;
    let scale_image = build_scale_image(
        scale_width,
        scale_height,
        &config.initial_overlay,
    );
    let scale_handle = images.add(scale_image);

    // Container node for the scale bar + labels, anchored to the right.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(40.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            // Max label at top of scale
            parent.spawn((
                Text::new(format!("{:.1}", config.color_scale_max)),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ScaleMaxLabel,
            ));

            // Scale bar image
            parent.spawn((
                ImageNode {
                    image: scale_handle,
                    ..default()
                },
                Node {
                    width: Val::Px(scale_width as f32),
                    height: Val::Px(scale_height as f32),
                    ..default()
                },
                ScaleBar,
            ));

            // Min label at bottom of scale
            parent.spawn((
                Text::new("0"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ScaleMinLabel,
            ));
        });

    // ── Spawn info panel (hidden by default, Req 1.2, 4.1, 4.2) ───
    let info_text = format_config_info(
        config.seed,
        &config.grid_config,
        &config.init_config,
        config.actor_config.as_ref(),
        config.stats_update_interval,
    );

    commands.spawn((
        Text::new(info_text),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Visibility::Hidden,
        InfoPanel,
    ));

    commands.insert_resource(InfoPanelVisible(false));

    // ── Insert trait visualization resources ────────────────────────
    commands.insert_resource(StatsPanelVisible(false));
    commands.insert_resource(TraitStats {
        actor_count: 0,
        tick: 0,
        traits: None,
        energy_stats: None,
    });

    // ── Insert predation counter resource ──────────────────────────
    commands.insert_resource(PredationCounter::default());

    // ── Insert stats throttle counter ──────────────────────────────
    commands.insert_resource(StatsTickCounter {
        ticks_since_update: 0,
        interval: config.stats_update_interval,
    });

    // ── Spawn stats panel (hidden by default, Req 2.1, 2.5, 2.6) ──
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            right: Val::Px(80.0),
            max_height: Val::Percent(80.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Visibility::Hidden,
        StatsPanel,
    ));

    // ── Insert SelectedActor resource ──────────────────────────────
    commands.insert_resource(SelectedActor::default());

    // ── Spawn actor inspector (hidden by default, Req 4.3–4.6) ────
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        Visibility::Hidden,
        ActorInspector,
    ));
}
