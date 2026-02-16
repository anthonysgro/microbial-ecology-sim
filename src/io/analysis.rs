// COLD PATH: Static analysis of simulation configuration.
// Pure functions over config structs — no I/O, no side effects.

use crate::grid::actor_config::ActorConfig;
use crate::grid::config::GridConfig;
use crate::grid::world_init::WorldInitConfig;
use crate::io::config_file::WorldConfig;

// ── Report structs ─────────────────────────────────────────────────

/// Top-level aggregation of all analysis sections.
#[derive(Debug, Clone)]
pub struct FullReport {
    pub grid_width: u32,
    pub grid_height: u32,
    pub cell_count: usize,
    pub seed: u64,
    pub tick_duration: f32,
    pub actors_enabled: bool,
    pub stability: StabilityReport,
    pub chemical_budget: ChemicalBudgetReport,
    pub energy_budget: Option<EnergyBudgetReport>,
    pub carrying_capacity: Option<CarryingCapacityReport>,
    pub source_density: SourceDensityReport,
    pub diffusion: DiffusionReport,
}

#[derive(Debug, Clone)]
pub struct StabilityReport {
    pub diffusion_number: f32,
    pub thermal_stability_number: f32,
    pub diffusion_stable: bool,
    pub thermal_stable: bool,
}

#[derive(Debug, Clone)]
pub struct ChemicalBudgetReport {
    pub expected_source_count: f32,
    pub expected_emission_per_tick: f32,
    pub expected_decay_per_tick: f32,
    pub expected_actor_consumption: f32,
    pub net_chemical_per_tick: f32,
    pub in_deficit: bool,
    pub actors_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct EnergyBudgetReport {
    pub net_energy_per_tick: f32,
    pub break_even_concentration: f32,
    pub idle_survival_ticks: f32,
    pub ticks_to_reproduction: Option<f32>,
    pub energy_positive: bool,
}

#[derive(Debug, Clone)]
pub struct CarryingCapacityReport {
    pub carrying_capacity: f32,
    pub cell_count: usize,
    pub space_limited: bool,
}

#[derive(Debug, Clone)]
pub struct SourceDensityReport {
    pub chemical_source_density: f32,
    pub heat_source_density: f32,
    pub chemical_renewable_fraction: f32,
    pub heat_renewable_fraction: f32,
    pub chemical_respawn_enabled: bool,
    pub chemical_respawn_cooldown_range: Option<(u32, u32)>,
    pub heat_respawn_enabled: bool,
    pub heat_respawn_cooldown_range: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct DiffusionReport {
    pub chemical_length_scale: f32,
    pub thermal_length_scale: f32,
    pub ticks_to_reach_5_cells: f32,
    pub ticks_to_reach_10_cells: f32,
    pub chemical_half_lives: Vec<f32>,
}

// ── Stub analysis functions ────────────────────────────────────────

pub fn analyze_stability(grid: &GridConfig) -> StabilityReport {
    let diffusion_number = grid.diffusion_rate * grid.tick_duration * 8.0;
    let thermal_stability_number = grid.thermal_conductivity * grid.tick_duration * 8.0;

    StabilityReport {
        diffusion_number,
        thermal_stability_number,
        diffusion_stable: diffusion_number < 1.0,
        thermal_stable: thermal_stability_number < 1.0,
    }
}

pub fn analyze_chemical_budget(
    grid: &GridConfig,
    world_init: &WorldInitConfig,
    actor: Option<&ActorConfig>,
) -> ChemicalBudgetReport {
    let cell_count = (grid.width as f32) * (grid.height as f32);
    let chem = &world_init.chemical_source_config;

    // Midpoint source count and midpoint emission rate (Req 3.1)
    let expected_source_count =
        (chem.min_sources as f32 + chem.max_sources as f32) / 2.0;
    let midpoint_emission_rate =
        (chem.min_emission_rate + chem.max_emission_rate) / 2.0;
    let expected_emission_per_tick = expected_source_count * midpoint_emission_rate;

    // Expected decay: cell_count * avg_concentration * decay_rate (Req 3.2)
    // Use the first chemical species decay rate (species 0 is the metabolic currency).
    let avg_concentration =
        (world_init.min_initial_concentration + world_init.max_initial_concentration) / 2.0;
    let decay_rate = grid.chemical_decay_rates.first().copied().unwrap_or(0.0);
    let expected_decay_per_tick = cell_count * avg_concentration * decay_rate;

    // Actor consumption: midpoint actor count * consumption_rate (Req 3.3, 3.6)
    let expected_actor_consumption = actor
        .map(|a| {
            let mid_actors =
                (world_init.min_actors as f32 + world_init.max_actors as f32) / 2.0;
            mid_actors * a.consumption_rate
        })
        .unwrap_or(0.0);

    // Net balance and deficit flag (Req 3.4, 3.5)
    let net_chemical_per_tick =
        expected_emission_per_tick - expected_decay_per_tick - expected_actor_consumption;

    ChemicalBudgetReport {
        expected_source_count,
        expected_emission_per_tick,
        expected_decay_per_tick,
        expected_actor_consumption,
        net_chemical_per_tick,
        in_deficit: net_chemical_per_tick < 0.0,
        actors_enabled: actor.is_some(),
    }
}

pub fn analyze_energy_budget(
    _grid: &GridConfig,
    world_init: &WorldInitConfig,
    actor: &ActorConfig,
) -> EnergyBudgetReport {
    // Average chemical concentration across the grid (species 0).
    let avg_concentration =
        (world_init.min_initial_concentration + world_init.max_initial_concentration) / 2.0;

    // Actual consumed amount is min(available, consumption_rate) per Req 4.1.
    let consumed = avg_concentration.min(actor.consumption_rate);

    // Net energy per tick at average concentration (Req 4.1):
    // consumed * (energy_conversion_factor - extraction_cost) - base_energy_decay - base_movement_cost
    let net_gain_factor = actor.energy_conversion_factor - actor.extraction_cost;
    let net_energy_per_tick =
        consumed * net_gain_factor - actor.base_energy_decay - actor.base_movement_cost;

    // Break-even concentration: the concentration at which net energy = 0 (Req 4.2).
    // c * (ecf - ec) - bed - bmc = 0  =>  c = (bed + bmc) / (ecf - ec)
    // Guard: if net_gain_factor <= 0, break-even is infinite (actor can never break even).
    let break_even_concentration = if net_gain_factor > 0.0 {
        (actor.base_energy_decay + actor.base_movement_cost) / net_gain_factor
    } else {
        f32::INFINITY
    };

    // Idle survival: ticks until energy depletes from basal decay alone (Req 4.3).
    let idle_survival_ticks = if actor.base_energy_decay > 0.0 {
        actor.initial_energy / actor.base_energy_decay
    } else {
        f32::INFINITY
    };

    // Ticks to reach reproduction threshold from initial energy (Req 4.4, 4.5).
    let ticks_to_reproduction = if net_energy_per_tick > 0.0 {
        Some((actor.reproduction_threshold - actor.initial_energy) / net_energy_per_tick)
    } else {
        None
    };

    EnergyBudgetReport {
        net_energy_per_tick,
        break_even_concentration,
        idle_survival_ticks,
        ticks_to_reproduction,
        energy_positive: net_energy_per_tick > 0.0,
    }
}

pub fn analyze_carrying_capacity(
    grid: &GridConfig,
    world_init: &WorldInitConfig,
    actor: &ActorConfig,
) -> CarryingCapacityReport {
    let cell_count = (grid.width as usize) * (grid.height as usize);
    let chem = &world_init.chemical_source_config;

    // Total chemical input per tick: midpoint source count * midpoint emission rate (Req 5.1).
    let expected_source_count =
        (chem.min_sources as f32 + chem.max_sources as f32) / 2.0;
    let midpoint_emission_rate =
        (chem.min_emission_rate + chem.max_emission_rate) / 2.0;
    let expected_emission_per_tick = expected_source_count * midpoint_emission_rate;

    // Carrying capacity = total chemical input / per-actor consumption (Req 5.1).
    let carrying_capacity = if actor.consumption_rate > 0.0 {
        expected_emission_per_tick / actor.consumption_rate
    } else {
        f32::INFINITY
    };

    // Space-limited if carrying capacity exceeds available cells (Req 5.2).
    let space_limited = carrying_capacity > cell_count as f32;

    CarryingCapacityReport {
        carrying_capacity,
        cell_count,
        space_limited,
    }
}

pub fn analyze_source_density(
    grid: &GridConfig,
    world_init: &WorldInitConfig,
) -> SourceDensityReport {
    let cell_count = (grid.width as f32) * (grid.height as f32);
    let chem = &world_init.chemical_source_config;
    let heat = &world_init.heat_source_config;

    // Midpoint source counts (Req 6.1, 6.2)
    let mid_chem_sources = (chem.min_sources as f32 + chem.max_sources as f32) / 2.0;
    let mid_heat_sources = (heat.min_sources as f32 + heat.max_sources as f32) / 2.0;

    let chemical_source_density = mid_chem_sources / cell_count;
    let heat_source_density = mid_heat_sources / cell_count;

    // Renewable fractions directly from config (Req 6.3)
    let chemical_renewable_fraction = chem.renewable_fraction;
    let heat_renewable_fraction = heat.renewable_fraction;

    // Respawn cooldown ranges when enabled (Req 6.4, 6.5)
    let chemical_respawn_cooldown_range = if chem.respawn_enabled {
        Some((chem.min_respawn_cooldown_ticks, chem.max_respawn_cooldown_ticks))
    } else {
        None
    };
    let heat_respawn_cooldown_range = if heat.respawn_enabled {
        Some((heat.min_respawn_cooldown_ticks, heat.max_respawn_cooldown_ticks))
    } else {
        None
    };

    SourceDensityReport {
        chemical_source_density,
        heat_source_density,
        chemical_renewable_fraction,
        heat_renewable_fraction,
        chemical_respawn_enabled: chem.respawn_enabled,
        chemical_respawn_cooldown_range,
        heat_respawn_enabled: heat.respawn_enabled,
        heat_respawn_cooldown_range,
    }
}

pub fn analyze_diffusion(grid: &GridConfig) -> DiffusionReport {
    let chemical_length_scale = (grid.diffusion_rate * grid.tick_duration).sqrt();
    let thermal_length_scale = (grid.thermal_conductivity * grid.tick_duration).sqrt();

    let ticks_to_reach_5_cells = if chemical_length_scale > 0.0 {
        (5.0 / chemical_length_scale).powi(2)
    } else {
        f32::INFINITY
    };

    let ticks_to_reach_10_cells = if chemical_length_scale > 0.0 {
        (10.0 / chemical_length_scale).powi(2)
    } else {
        f32::INFINITY
    };

    let chemical_half_lives = grid
        .chemical_decay_rates
        .iter()
        .map(|&rate| {
            if rate > 0.0 {
                f32::ln(2.0) / rate
            } else {
                f32::INFINITY
            }
        })
        .collect();

    DiffusionReport {
        chemical_length_scale,
        thermal_length_scale,
        ticks_to_reach_5_cells,
        ticks_to_reach_10_cells,
        chemical_half_lives,
    }
}

/// Orchestrator: run all analysis functions and assemble the full report.
pub fn analyze(config: &WorldConfig) -> FullReport {
    let grid = &config.grid;
    let world_init = &config.world_init;
    let actor = config.actor.as_ref();
    let cell_count = (grid.width as usize) * (grid.height as usize);

    let stability = analyze_stability(grid);
    let chemical_budget = analyze_chemical_budget(grid, world_init, actor);
    let energy_budget = actor.map(|a| analyze_energy_budget(grid, world_init, a));
    let carrying_capacity = actor.map(|a| analyze_carrying_capacity(grid, world_init, a));
    let source_density = analyze_source_density(grid, world_init);
    let diffusion = analyze_diffusion(grid);

    FullReport {
        grid_width: grid.width,
        grid_height: grid.height,
        cell_count,
        seed: config.seed,
        tick_duration: grid.tick_duration,
        actors_enabled: actor.is_some(),
        stability,
        chemical_budget,
        energy_budget,
        carrying_capacity,
        source_density,
        diffusion,
    }
}

/// Format a full report as plain text for stdout.
/// Format a full report as plain text for stdout.
pub fn format_report(report: &FullReport) -> String {
    let mut out = String::new();

    // ── Summary header (Req 8.3) ───────────────────────────────────
    out.push_str(&format!(
        "=== Config Analysis Report ===\n\
         Grid: {}x{} ({} cells)  |  Seed: {}  |  Tick: {}s  |  Actors: {}\n\n",
        report.grid_width,
        report.grid_height,
        report.cell_count,
        report.seed,
        report.tick_duration,
        if report.actors_enabled { "enabled" } else { "disabled" },
    ));

    // ── Numerical Stability (Req 8.1, 8.4, 8.5) ───────────────────
    format_stability(&mut out, &report.stability);

    // ── Chemical Budget (Req 8.1, 8.4, 8.5) ───────────────────────
    format_chemical_budget(&mut out, &report.chemical_budget);

    // ── Energy Budget — skip when actors disabled (Req 8.1, 8.4) ──
    if let Some(ref eb) = report.energy_budget {
        format_energy_budget(&mut out, eb);
    }

    // ── Carrying Capacity — skip when actors disabled (Req 8.1) ───
    if let Some(ref cc) = report.carrying_capacity {
        format_carrying_capacity(&mut out, cc);
    }

    // ── Source Density (Req 8.1) ───────────────────────────────────
    format_source_density(&mut out, &report.source_density);

    // ── Diffusion Characterization (Req 8.1) ──────────────────────
    format_diffusion(&mut out, &report.diffusion);

    out
}

fn format_stability(out: &mut String, s: &StabilityReport) {
    out.push_str("--- Numerical Stability ---\n");
    out.push_str(&format!("  Diffusion number:          {:.4}\n", s.diffusion_number));
    if s.diffusion_stable {
        out.push_str("  [OK]   Chemical diffusion is stable\n");
    } else {
        out.push_str("  [WARN] Chemical diffusion is numerically UNSTABLE (>= 1.0)\n");
    }
    out.push_str(&format!("  Thermal stability number:  {:.4}\n", s.thermal_stability_number));
    if s.thermal_stable {
        out.push_str("  [OK]   Thermal diffusion is stable\n");
    } else {
        out.push_str("  [WARN] Thermal diffusion is numerically UNSTABLE (>= 1.0)\n");
    }
    out.push('\n');
}

fn format_chemical_budget(out: &mut String, cb: &ChemicalBudgetReport) {
    out.push_str("--- Chemical Budget ---\n");
    out.push_str(&format!("  Expected source count:     {:.1}\n", cb.expected_source_count));
    out.push_str(&format!("  Emission per tick:         {:.4}\n", cb.expected_emission_per_tick));
    out.push_str(&format!("  Decay per tick:            {:.4}\n", cb.expected_decay_per_tick));
    if cb.actors_enabled {
        out.push_str(&format!("  Actor consumption/tick:    {:.4}\n", cb.expected_actor_consumption));
    } else {
        out.push_str("  Actor consumption/tick:    0 (actors disabled)\n");
    }
    out.push_str(&format!("  Net chemical/tick:         {:.4}\n", cb.net_chemical_per_tick));
    if cb.in_deficit {
        out.push_str("  [WARN] System is in chemical deficit — sources are being out-consumed\n");
    } else {
        out.push_str("  [OK]   Chemical budget is positive\n");
    }
    out.push('\n');
}

fn format_energy_budget(out: &mut String, eb: &EnergyBudgetReport) {
    out.push_str("--- Energy Budget ---\n");
    out.push_str(&format!("  Net energy/tick:           {:.4}\n", eb.net_energy_per_tick));
    out.push_str(&format!("  Break-even concentration:  {:.4}\n", eb.break_even_concentration));
    out.push_str(&format!("  Idle survival:             {:.1} ticks\n", eb.idle_survival_ticks));
    match eb.ticks_to_reproduction {
        Some(t) => out.push_str(&format!("  Ticks to reproduction:     {:.1}\n", t)),
        None => out.push_str("  Ticks to reproduction:     N/A (net energy <= 0)\n"),
    }
    if eb.energy_positive {
        out.push_str("  [OK]   Actors gain energy under average conditions\n");
    } else {
        out.push_str("  [WARN] Actors lose energy under average conditions\n");
    }
    out.push('\n');
}

fn format_carrying_capacity(out: &mut String, cc: &CarryingCapacityReport) {
    out.push_str("--- Carrying Capacity ---\n");
    out.push_str(&format!("  Estimated capacity:        {:.1} actors\n", cc.carrying_capacity));
    out.push_str(&format!("  Grid cell count:           {}\n", cc.cell_count));
    if cc.space_limited {
        out.push_str("  [WARN] Grid is space-limited (capacity exceeds cell count)\n");
    } else {
        out.push_str("  [OK]   Grid is resource-limited\n");
    }
    out.push('\n');
}

fn format_source_density(out: &mut String, sd: &SourceDensityReport) {
    out.push_str("--- Source Density ---\n");
    out.push_str(&format!("  Chemical source density:   {:.6} sources/cell\n", sd.chemical_source_density));
    out.push_str(&format!("  Heat source density:       {:.6} sources/cell\n", sd.heat_source_density));
    out.push_str(&format!("  Chemical renewable:        {:.0}%\n", sd.chemical_renewable_fraction * 100.0));
    out.push_str(&format!("  Heat renewable:            {:.0}%\n", sd.heat_renewable_fraction * 100.0));
    if sd.chemical_respawn_enabled {
        if let Some((min, max)) = sd.chemical_respawn_cooldown_range {
            out.push_str(&format!("  Chemical respawn cooldown: {}–{} ticks\n", min, max));
        }
    } else {
        out.push_str("  Chemical respawn:          disabled (depleted sources are permanent)\n");
    }
    if sd.heat_respawn_enabled {
        if let Some((min, max)) = sd.heat_respawn_cooldown_range {
            out.push_str(&format!("  Heat respawn cooldown:     {}–{} ticks\n", min, max));
        }
    } else {
        out.push_str("  Heat respawn:              disabled (depleted sources are permanent)\n");
    }
    out.push('\n');
}

fn format_diffusion(out: &mut String, d: &DiffusionReport) {
    out.push_str("--- Diffusion Characterization ---\n");
    out.push_str(&format!("  Chemical length scale:     {:.4} cells/tick\n", d.chemical_length_scale));
    out.push_str(&format!("  Thermal length scale:      {:.4} cells/tick\n", d.thermal_length_scale));
    out.push_str(&format!("  Ticks to reach 5 cells:    {:.1}\n", d.ticks_to_reach_5_cells));
    out.push_str(&format!("  Ticks to reach 10 cells:   {:.1}\n", d.ticks_to_reach_10_cells));
    for (i, hl) in d.chemical_half_lives.iter().enumerate() {
        if hl.is_infinite() {
            out.push_str(&format!("  Species {} half-life:       ∞ (no decay)\n", i));
        } else {
            out.push_str(&format!("  Species {} half-life:       {:.1} ticks\n", i, hl));
        }
    }
}
