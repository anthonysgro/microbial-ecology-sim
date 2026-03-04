// COLD PATH: Static analysis of simulation configuration.
// Pure functions over config structs — no I/O, no side effects.

use crate::grid::actor_config::ActorConfig;
use crate::grid::config::GridConfig;
use crate::grid::world_init::{SourceFieldConfig, WorldInitConfig};
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
    /// Per-species diffusion stability numbers (rate * tick_duration * 8).
    pub diffusion_numbers: Vec<f32>,
    /// Per-species stability flags.
    pub diffusion_stable: Vec<bool>,
    pub thermal_stability_number: f32,
    pub thermal_stable: bool,
}

#[derive(Debug, Clone)]
pub struct ChemicalBudgetReport {
    pub expected_source_count: f32,
    /// Raw instantaneous emission if all sources were active at full rate.
    pub raw_emission_per_tick: f32,
    /// Effective long-term emission accounting for renewable fraction,
    /// non-renewable depletion lifetime, and respawn duty cycle.
    pub effective_emission_per_tick: f32,
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
    /// Estimated steady-state concentration per cell from source emission.
    pub steady_state_concentration: f32,
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
    /// Per-species diffusion length scales (sqrt(rate * tick_duration)).
    pub chemical_length_scales: Vec<f32>,
    pub thermal_length_scale: f32,
    /// Per-species chemical half-lives (ln(2) / decay_rate).
    pub chemical_half_lives: Vec<f32>,
}

// ── Stub analysis functions ────────────────────────────────────────

pub fn analyze_stability(grid: &GridConfig, world_init: &WorldInitConfig) -> StabilityReport {
    let diffusion_numbers: Vec<f32> = world_init
        .chemical_species_configs
        .iter()
        .map(|c| c.diffusion_rate * grid.tick_duration * 8.0)
        .collect();
    let diffusion_stable: Vec<bool> = diffusion_numbers.iter().map(|&n| n < 1.0).collect();
    let thermal_stability_number = grid.thermal_conductivity * grid.tick_duration * 8.0;

    StabilityReport {
        diffusion_numbers,
        diffusion_stable,
        thermal_stability_number,
        thermal_stable: thermal_stability_number < 1.0,
    }
}

/// Compute the long-term effective emission rate for a source field config.
///
/// Accounts for:
/// - Renewable sources: emit at full rate indefinitely.
/// - Non-renewable sources with respawn: emit for `lifetime` ticks, then wait
///   `cooldown` ticks. Effective rate = raw_rate * lifetime / (lifetime + cooldown).
/// - Non-renewable sources without respawn: contribute 0 to long-term budget
///   (they eventually deplete permanently).
///
/// Returns (raw_emission_per_tick, effective_emission_per_tick).
fn effective_source_emission(cfg: &SourceFieldConfig) -> (f32, f32) {
    let expected_count = (cfg.min_sources as f32 + cfg.max_sources as f32) / 2.0;
    let mid_rate = (cfg.min_emission_rate + cfg.max_emission_rate) / 2.0;
    let raw = expected_count * mid_rate;

    let renewable_count = expected_count * cfg.renewable_fraction;
    let nonrenewable_count = expected_count * (1.0 - cfg.renewable_fraction);

    // Renewable sources emit indefinitely at full rate.
    let renewable_emission = renewable_count * mid_rate;

    // Non-renewable sources: estimate average lifetime from reservoir / emission_rate.
    let nonrenewable_emission = if nonrenewable_count > 0.0 && cfg.respawn_enabled {
        let mid_reservoir =
            (cfg.min_reservoir_capacity + cfg.max_reservoir_capacity) / 2.0;
        let lifetime = if mid_rate > 0.0 {
            mid_reservoir / mid_rate
        } else {
            f32::INFINITY
        };
        let mid_cooldown = (cfg.min_respawn_cooldown_ticks as f32
            + cfg.max_respawn_cooldown_ticks as f32)
            / 2.0;
        let cycle = lifetime + mid_cooldown;
        let duty = if cycle > 0.0 { lifetime / cycle } else { 1.0 };
        nonrenewable_count * mid_rate * duty
    } else {
        // No respawn: non-renewable sources deplete permanently.
        // Long-term contribution is zero.
        0.0
    };

    (raw, renewable_emission + nonrenewable_emission)
}

/// Estimate steady-state concentration per cell.
///
/// At equilibrium, emission balances decay: `effective_emission = cell_count * C_ss * decay_rate`.
/// Solving: `C_ss = effective_emission / (cell_count * decay_rate)`.
/// If decay_rate is 0, concentration grows without bound (return INFINITY).
fn steady_state_concentration(
    effective_emission: f32,
    cell_count: f32,
    decay_rate: f32,
) -> f32 {
    if decay_rate > 0.0 && cell_count > 0.0 {
        effective_emission / (cell_count * decay_rate)
    } else {
        f32::INFINITY
    }
}

pub fn analyze_chemical_budget(
    grid: &GridConfig,
    world_init: &WorldInitConfig,
    actor: Option<&ActorConfig>,
) -> ChemicalBudgetReport {
    let cell_count = (grid.width as f32) * (grid.height as f32);
    let chem_species = world_init.chemical_species_configs.first()
        .expect("at least one chemical species config required");
    let chem = &chem_species.source_config;

    let expected_source_count =
        (chem.min_sources as f32 + chem.max_sources as f32) / 2.0;

    let (raw_emission, effective_emission) = effective_source_emission(chem);

    // Steady-state decay uses the equilibrium concentration, not initial.
    let decay_rate = chem_species.decay_rate;
    let ss_concentration = steady_state_concentration(effective_emission, cell_count, decay_rate);
    let expected_decay_per_tick = cell_count * ss_concentration * decay_rate;

    // Actor consumption at steady state.
    let expected_actor_consumption = actor
        .map(|a| {
            let mid_actors =
                (world_init.min_actors as f32 + world_init.max_actors as f32) / 2.0;
            mid_actors * a.consumption_rate
        })
        .unwrap_or(0.0);

    let net_chemical_per_tick =
        effective_emission - expected_decay_per_tick - expected_actor_consumption;

    ChemicalBudgetReport {
        expected_source_count,
        raw_emission_per_tick: raw_emission,
        effective_emission_per_tick: effective_emission,
        expected_decay_per_tick,
        expected_actor_consumption,
        net_chemical_per_tick,
        in_deficit: net_chemical_per_tick < 0.0,
        actors_enabled: actor.is_some(),
    }
}

pub fn analyze_energy_budget(
    grid: &GridConfig,
    world_init: &WorldInitConfig,
    actor: &ActorConfig,
) -> EnergyBudgetReport {
    let cell_count = (grid.width as f32) * (grid.height as f32);
    let chem_species = world_init.chemical_species_configs.first()
        .expect("at least one chemical species config required");

    // Compute effective emission to derive steady-state concentration.
    let (_, effective_emission) = effective_source_emission(&chem_species.source_config);
    let ss_concentration =
        steady_state_concentration(effective_emission, cell_count, chem_species.decay_rate);

    // Actual consumed amount is min(available, consumption_rate).
    let consumed = ss_concentration.min(actor.consumption_rate);

    // Net energy per tick at steady-state concentration:
    // consumed * (energy_conversion_factor - extraction_cost) - base_energy_decay - base_movement_cost
    let net_gain_factor = actor.energy_conversion_factor - actor.extraction_cost;
    let net_energy_per_tick =
        consumed * net_gain_factor - actor.base_energy_decay - actor.base_movement_cost;

    // Break-even concentration: the concentration at which net energy = 0.
    // c * (ecf - ec) - bed - bmc = 0  =>  c = (bed + bmc) / (ecf - ec)
    let break_even_concentration = if net_gain_factor > 0.0 {
        (actor.base_energy_decay + actor.base_movement_cost) / net_gain_factor
    } else {
        f32::INFINITY
    };

    // Idle survival: ticks until energy depletes from basal decay alone.
    let idle_survival_ticks = if actor.base_energy_decay > 0.0 {
        actor.initial_energy / actor.base_energy_decay
    } else {
        f32::INFINITY
    };

    // Ticks to reach reproduction threshold from initial energy.
    let ticks_to_reproduction = if net_energy_per_tick > 0.0 {
        Some((actor.reproduction_threshold - actor.initial_energy) / net_energy_per_tick)
    } else {
        None
    };

    EnergyBudgetReport {
        net_energy_per_tick,
        break_even_concentration,
        steady_state_concentration: ss_concentration,
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
    let chem_species = world_init.chemical_species_configs.first()
        .expect("at least one chemical species config required");

    // Use effective long-term emission, not raw instantaneous.
    let (_, effective_emission) = effective_source_emission(&chem_species.source_config);

    // Energy-balance carrying capacity: how many actors can the system's
    // energy throughput sustain?
    //
    // total_energy_input = effective_emission * (ecf - extraction_cost)
    // per_actor_drain    = base_energy_decay + base_movement_cost
    // capacity           = total_energy_input / per_actor_drain
    //
    // This is more accurate than emission / consumption_rate because actors
    // don't all consume at full rate — they consume what they need to offset
    // metabolic costs. The old model asked "how many actors could eat at max
    // rate?" which dramatically underestimates sustainable population.
    let net_gain_factor = actor.energy_conversion_factor - actor.extraction_cost;
    let total_energy_input = effective_emission * net_gain_factor;
    let per_actor_drain = actor.base_energy_decay + actor.base_movement_cost;

    let carrying_capacity = if per_actor_drain > 0.0 {
        total_energy_input / per_actor_drain
    } else {
        f32::INFINITY
    };

    // Space-limited if carrying capacity exceeds available cells.
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
    let chem = &world_init.chemical_species_configs.first()
        .map(|c| &c.source_config)
        .expect("at least one chemical species config required");
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

pub fn analyze_diffusion(grid: &GridConfig, world_init: &WorldInitConfig) -> DiffusionReport {
    let chemical_length_scales: Vec<f32> = world_init
        .chemical_species_configs
        .iter()
        .map(|c| (c.diffusion_rate * grid.tick_duration).sqrt())
        .collect();
    let thermal_length_scale = (grid.thermal_conductivity * grid.tick_duration).sqrt();

    let chemical_half_lives: Vec<f32> = world_init
        .chemical_species_configs
        .iter()
        .map(|c| {
            if c.decay_rate > 0.0 {
                f32::ln(2.0) / c.decay_rate
            } else {
                f32::INFINITY
            }
        })
        .collect();

    DiffusionReport {
        chemical_length_scales,
        thermal_length_scale,
        chemical_half_lives,
    }
}

/// Orchestrator: run all analysis functions and assemble the full report.
pub fn analyze(config: &WorldConfig) -> FullReport {
    let grid = &config.grid;
    let world_init = &config.world_init;
    let actor = config.actor.as_ref();
    let cell_count = (grid.width as usize) * (grid.height as usize);

    let stability = analyze_stability(grid, world_init);
    let chemical_budget = analyze_chemical_budget(grid, world_init, actor);
    let energy_budget = actor.map(|a| analyze_energy_budget(grid, world_init, a));
    let carrying_capacity = actor.map(|a| analyze_carrying_capacity(grid, world_init, a));
    let source_density = analyze_source_density(grid, world_init);
    let diffusion = analyze_diffusion(grid, world_init);

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
    for (i, (&number, &stable)) in s
        .diffusion_numbers
        .iter()
        .zip(s.diffusion_stable.iter())
        .enumerate()
    {
        out.push_str(&format!("  Species {} diffusion number: {:.4}\n", i, number));
        if stable {
            out.push_str(&format!("  [OK]   Species {} chemical diffusion is stable\n", i));
        } else {
            out.push_str(&format!(
                "  [WARN] Species {} chemical diffusion is numerically UNSTABLE (>= 1.0)\n",
                i
            ));
        }
    }
    out.push_str(&format!(
        "  Thermal stability number:  {:.4}\n",
        s.thermal_stability_number
    ));
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
    out.push_str(&format!("  Raw emission per tick:     {:.4}\n", cb.raw_emission_per_tick));
    out.push_str(&format!("  Effective emission/tick:   {:.4}  (accounts for depletion + respawn duty cycle)\n", cb.effective_emission_per_tick));
    out.push_str(&format!("  Decay per tick (ss):       {:.4}\n", cb.expected_decay_per_tick));
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
    if eb.steady_state_concentration.is_infinite() {
        out.push_str("  Steady-state conc.:        ∞ (no decay)\n");
    } else {
        out.push_str(&format!("  Steady-state conc.:        {:.4}\n", eb.steady_state_concentration));
    }
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
    out.push_str("  [NOTE] Capacity assumes uniform distribution. Actors cluster near sources,\n");
    out.push_str("         so actual population may exceed this estimate.\n");
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
    for (i, &ls) in d.chemical_length_scales.iter().enumerate() {
        out.push_str(&format!(
            "  Species {} length scale:    {:.4} cells/tick\n",
            i, ls
        ));
    }
    out.push_str(&format!(
        "  Thermal length scale:      {:.4} cells/tick\n",
        d.thermal_length_scale
    ));
    for (i, hl) in d.chemical_half_lives.iter().enumerate() {
        if hl.is_infinite() {
            out.push_str(&format!("  Species {} half-life:       ∞ (no decay)\n", i));
        } else {
            out.push_str(&format!("  Species {} half-life:       {:.1} ticks\n", i, hl));
        }
    }
}
