# Configuration Documentation — Steering Rule

When any spec adds, removes, or modifies a configuration field (in `ActorConfig`, `GridConfig`, `WorldInitConfig`, or any other config struct):

1. **`example_config.toml`** — Update the example config file to include the new/changed field with a comment explaining its purpose and valid range.
2. **World config info panel** — Update `format_config_info()` in `src/viz_bevy/setup.rs` to display the new/changed field. This panel is toggled by pressing `I` in the Bevy visualization and must reflect all active configuration values.
3. **This steering file** — Update the config reference below to reflect the new/changed field so this document stays the single source of truth.
4. **Spec requirements** — Include a documentation update requirement in the spec so it appears in the task list and is not forgotten during implementation.

This ensures configuration documentation stays in sync with the code at all times.

---

## Heritable Trait Update Rule

When any spec adds, removes, or renames a heritable trait on `Actor` (currently: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`, `max_tumble_steps`, `reproduction_cost`, `offspring_energy`, `mutation_rate`, `kin_tolerance`):

1. **`HeritableTraits` struct** — Update the struct in `src/grid/actor.rs` with the new/changed field.
2. **Trait visualization stats** — Update `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs` to collect and compute statistics for the new trait. The `TraitStats.traits` array size (currently `[SingleTraitStats; 9]`) must match the trait count.
3. **Stats panel formatting** — Update `format_trait_stats` in `src/viz_bevy/setup.rs` to display the new trait row.
4. **Actor inspector formatting** — Update `format_actor_info` in `src/viz_bevy/setup.rs` to display the new trait value.
5. **Trait clamp config** — Add `trait_{name}_min` / `trait_{name}_max` fields to `ActorConfig` and follow the configuration update rules above.
6. **Spec requirements** — Include a trait visualization update requirement in the spec so it appears in the task list.

---

## Configuration Reference

All configuration is loaded from a TOML file. Omitted sections/fields fall back to compiled defaults. Unknown keys are rejected at parse time (`deny_unknown_fields`).

### Top-level

| TOML key | Type | Default | Description |
|---|---|---|---|
| `seed` | `u64` | `42` | Master RNG seed. Deterministic replay: same seed + same config = same simulation. |

### `[grid]` — `GridConfig`

| TOML key | Type | Default | Description |
|---|---|---|---|
| `width` | `u32` | `30` | Grid width in cells. |
| `height` | `u32` | `30` | Grid height in cells. |
| `num_chemicals` | `usize` | `2` | Number of chemical species tracked per cell. |
| `thermal_conductivity` | `f32` | `0.05` | Heat radiation coefficient. |
| `ambient_heat` | `f32` | `0.0` | Boundary condition for heat: missing neighbors use this value. |
| `tick_duration` | `f32` | `1.0` | Simulated time per tick (seconds). |
| `num_threads` | `usize` | `4` | Number of spatial partitions (maps to rayon thread count). |

### `[world_init]` — `WorldInitConfig`

| TOML key | Type | Default | Description |
|---|---|---|---|
| `min_initial_heat` | `f32` | `0.0` | Minimum initial per-cell heat value. |
| `max_initial_heat` | `f32` | `1.0` | Maximum initial per-cell heat value. |
| `min_initial_concentration` | `f32` | `0.0` | Minimum initial per-cell chemical concentration (per species). |
| `max_initial_concentration` | `f32` | `0.5` | Maximum initial per-cell chemical concentration (per species). |
| `min_actors` | `u32` | `0` | Minimum number of actors seeded at init. |
| `max_actors` | `u32` | `0` | Maximum number of actors seeded at init. Set both to 0 to skip. |

### `[world_init.heat_source_config]` — `SourceFieldConfig` (heat)

| TOML key | Type | Default | Description |
|---|---|---|---|
| `min_sources` | `u32` | `1` | Minimum number of heat sources to place. |
| `max_sources` | `u32` | `5` | Maximum number of heat sources to place. |
| `min_emission_rate` | `f32` | `0.1` | Minimum emission rate (units per tick). |
| `max_emission_rate` | `f32` | `5.0` | Maximum emission rate (units per tick). |
| `renewable_fraction` | `f32` | `0.3` | Fraction of sources that are renewable. `[0.0, 1.0]`. |
| `min_reservoir_capacity` | `f32` | `50.0` | Minimum initial reservoir for non-renewable sources. Must be `> 0.0`. |
| `max_reservoir_capacity` | `f32` | `200.0` | Maximum initial reservoir for non-renewable sources. |
| `min_deceleration_threshold` | `f32` | `0.1` | Minimum deceleration threshold for non-renewable sources. `[0.0, 1.0]`. |
| `max_deceleration_threshold` | `f32` | `0.5` | Maximum deceleration threshold for non-renewable sources. `[0.0, 1.0]`. |
| `respawn_enabled` | `bool` | `false` | Whether depleted non-renewable sources trigger respawns. |
| `min_respawn_cooldown_ticks` | `u32` | `50` | Minimum ticks before a depleted source respawns. |
| `max_respawn_cooldown_ticks` | `u32` | `150` | Maximum ticks before a depleted source respawns. When `respawn_enabled` is true, must be `> 0` and `>= min_respawn_cooldown_ticks`. |
| `source_clustering` | `f32` | `0.0` | Spatial clustering of sources. `0.0` = uniform random, `1.0` = tight clusters around a single center. Range: `[0.0, 1.0]`. |

### `[[world_init.chemical_species_configs]]` — `ChemicalSpeciesConfig`

One entry per chemical species. The i-th entry configures Chemical_Species i. Length must equal `num_chemicals`.

| TOML key | Type | Default | Description |
|---|---|---|---|
| `decay_rate` | `f32` | `0.05` | Exponential decay rate per tick. `[0.0, 1.0]`. Applied as `concentration *= (1.0 - decay_rate)`. |
| `diffusion_rate` | `f32` | `0.05` | Diffusion coefficient (discrete Laplacian scaling). Must be non-negative and finite. Stability: `diffusion_rate * tick_duration * 8 < 1.0`. |

Each entry contains a nested `source_config` table with the same fields as `heat_source_config`:

### `[world_init.chemical_species_configs.source_config]` — `SourceFieldConfig` (per species)

Same fields as `heat_source_config`. Defaults differ only in:

| TOML key | Default (chemical) |
|---|---|
| `max_sources` | `3` |

All other fields share the same defaults as the heat source config.

### `[actor]` — `ActorConfig`

Present as `Option<ActorConfig>`. Omitting the entire `[actor]` section disables actors.

| TOML key | Type | Default | Description |
|---|---|---|---|
| `consumption_rate` | `f32` | `1.5` | Chemical units consumed per tick from the actor's cell (species 0). |
| `energy_conversion_factor` | `f32` | `2.0` | Energy gained per unit of chemical consumed. |
| `extraction_cost` | `f32` | `0.2` | Energy cost per unit of chemical consumed. Net gain = `consumed * (energy_conversion_factor - extraction_cost)`. Must be `>= 0.0` and `< energy_conversion_factor`. |
| `base_energy_decay` | `f32` | `0.05` | Energy subtracted every tick (basal metabolic cost). |
| `reference_metabolic_rate` | `f32` | `0.05` | Metabolic rate at which all scaling multipliers equal 1.0. Higher `base_energy_decay` → better consumption, cheaper movement, stronger predation. Must be `> 0.0` and finite. |
| `initial_energy` | `f32` | `10.0` | Energy assigned to newly spawned actors. Must be `<= max_energy`. |
| `max_energy` | `f32` | `50.0` | Maximum energy an actor can hold. Clamped after each metabolic tick. Must be `> 0.0`, finite, `>= initial_energy`. |
| `initial_actor_capacity` | `usize` | `64` | Pre-allocated slot capacity for the ActorRegistry. |
| `base_movement_cost` | `f32` | `0.5` | Base energy cost for movement at the reference energy level. Actual cost = `base_movement_cost * (actor.energy / reference_energy)`, floored at 10% of base. Must be `>= 0.0`. |
| `reference_energy` | `f32` | `25.0` | Energy level at which movement cost equals `base_movement_cost`. Actors above this pay more; actors below pay less. Must be `> 0.0`. |
| `removal_threshold` | `f32` | `-5.0` | Energy level below which an inert actor is permanently removed. Must be `<= 0.0`. |
| `levy_exponent` | `f32` | `1.5` | Power-law exponent α for Lévy flight step distribution. Controls the mix of short vs long tumble runs during random foraging. Must be `> 1.0`. |
| `max_tumble_steps` | `u16` | `20` | Maximum steps in a single tumble run. Clamps the power-law sample. Must be `>= 1`. |
| `reproduction_threshold` | `f32` | `20.0` | Minimum energy for binary fission. Must be `> 0.0` and `>= reproduction_cost`. |
| `reproduction_cost` | `f32` | `12.0` | Energy deducted from parent upon fission. Must be `> 0.0` and `>= offspring_energy`. |
| `offspring_energy` | `f32` | `10.0` | Energy assigned to offspring at creation. Must be `> 0.0` and `<= max_energy`. |
| `mutation_stddev` | `f32` | `0.05` | Seed genome default for the per-actor heritable `mutation_rate` trait. Each actor carries its own `mutation_rate` which evolves via proportional self-mutation. Must be within `[trait_mutation_rate_min, trait_mutation_rate_max]`. |
| `trait_consumption_rate_min` | `f32` | `0.1` | Minimum clamp bound for heritable `consumption_rate`. Must be `> 0.0` and `< trait_consumption_rate_max`. |
| `trait_consumption_rate_max` | `f32` | `10.0` | Maximum clamp bound for heritable `consumption_rate`. Must be `> trait_consumption_rate_min`. |
| `trait_base_energy_decay_min` | `f32` | `0.001` | Minimum clamp bound for heritable `base_energy_decay`. Must be `> 0.0` and `< trait_base_energy_decay_max`. |
| `trait_base_energy_decay_max` | `f32` | `1.0` | Maximum clamp bound for heritable `base_energy_decay`. Must be `> trait_base_energy_decay_min`. |
| `trait_levy_exponent_min` | `f32` | `1.01` | Minimum clamp bound for heritable `levy_exponent`. Must be `> 1.0` and `< trait_levy_exponent_max`. |
| `trait_levy_exponent_max` | `f32` | `3.0` | Maximum clamp bound for heritable `levy_exponent`. Must be `> trait_levy_exponent_min`. |
| `trait_reproduction_threshold_min` | `f32` | `1.0` | Minimum clamp bound for heritable `reproduction_threshold`. Must be `> 0.0` and `< trait_reproduction_threshold_max`. |
| `trait_reproduction_threshold_max` | `f32` | `100.0` | Maximum clamp bound for heritable `reproduction_threshold`. Must be `> trait_reproduction_threshold_min`. |
| `trait_max_tumble_steps_min` | `u16` | `1` | Minimum clamp bound for heritable `max_tumble_steps`. Must be `>= 1` and `< trait_max_tumble_steps_max`. |
| `trait_max_tumble_steps_max` | `u16` | `50` | Maximum clamp bound for heritable `max_tumble_steps`. Must be `> trait_max_tumble_steps_min`. |
| `trait_reproduction_cost_min` | `f32` | `0.1` | Minimum clamp bound for heritable `reproduction_cost`. Must be `> 0.0` and `< trait_reproduction_cost_max`. |
| `trait_reproduction_cost_max` | `f32` | `100.0` | Maximum clamp bound for heritable `reproduction_cost`. Must be `> trait_reproduction_cost_min`. |
| `trait_offspring_energy_min` | `f32` | `0.1` | Minimum clamp bound for heritable `offspring_energy`. Must be `> 0.0` and `< trait_offspring_energy_max`. |
| `trait_offspring_energy_max` | `f32` | `100.0` | Maximum clamp bound for heritable `offspring_energy`. Must be `> trait_offspring_energy_min` and `<= max_energy`. |
| `trait_mutation_rate_min` | `f32` | `0.001` | Minimum clamp bound for heritable `mutation_rate`. Must be `> 0.0` and `< trait_mutation_rate_max`. |
| `trait_mutation_rate_max` | `f32` | `0.5` | Maximum clamp bound for heritable `mutation_rate`. Must be `> trait_mutation_rate_min`. |
| `absorption_efficiency` | `f32` | `0.5` | Fraction of prey energy transferred to predator on successful predation. Must be in `(0.0, 1.0]`. |
| `kin_tolerance` | `f32` | `0.5` | Seed genome default for heritable `kin_tolerance` trait. Controls genetic distance threshold below which predation is suppressed. Must be within `[trait_kin_tolerance_min, trait_kin_tolerance_max]`. |
| `trait_kin_tolerance_min` | `f32` | `0.0` | Minimum clamp bound for heritable `kin_tolerance`. Must be `< trait_kin_tolerance_max`. |
| `trait_kin_tolerance_max` | `f32` | `1.0` | Maximum clamp bound for heritable `kin_tolerance`. Must be `> trait_kin_tolerance_min`. |

### `[bevy]` — `BevyExtras`

Optional. Only consumed by the Bevy visualization binary.

| TOML key | Type | Default | Description |
|---|---|---|---|
| `tick_hz` | `f64` | `10.0` | Simulation ticks per second. Drives `FixedUpdate` timestep. |
| `zoom_min` | `f32` | `0.1` | Minimum camera zoom level. |
| `zoom_max` | `f32` | `10.0` | Maximum camera zoom level. |
| `zoom_speed` | `f32` | `0.1` | Camera zoom speed per scroll event. |
| `pan_speed` | `f32` | `1.0` | Camera pan speed. |
| `color_scale_max` | `f32` | `10.0` | Fixed upper bound for color mapping. Values above this render as full intensity. |
| `stats_update_interval` | `u64` | `10` | Ticks between trait stats recomputations. 0 or 1 = every tick (no throttling). Higher values reduce CPU cost of the stats panel. |

---

## Bevy Runtime Resources

These are not TOML-configurable. They are Bevy `Resource` structs managed at runtime by the visualization layer.

### `PredationCounter`

**File:** `src/viz_bevy/resources.rs`

Tracks per-tick and cumulative predation events for HUD display. Updated once per tick in `tick_simulation`. Read by `update_stats_panel` → `format_trait_stats`.

| Field | Type | Description |
|---|---|---|
| `last_tick` | `usize` | Number of predation events in the most recent completed tick. |
| `total` | `u64` | Cumulative predation events since simulation start. `u64` to avoid overflow on long runs. |

Displayed in the stats panel header line as: `Tick: N  |  Actors: N  |  Predations: N (total: N)`.

### `TraitStats`

**File:** `src/viz_bevy/resources.rs`

Population-level statistics recomputed every `stats_update_interval` ticks by `compute_trait_stats_from_actors`.

| Field | Type | Description |
|---|---|---|
| `actor_count` | `usize` | Number of non-inert actors at computation time. |
| `tick` | `u64` | Simulation tick at computation time. |
| `traits` | `Option<[SingleTraitStats; 9]>` | Per-trait population stats for the 9 heritable traits. `None` when no living actors. |
| `energy_stats` | `Option<SingleTraitStats>` | Population energy statistics (min, p25, p50, p75, max, mean). `None` when no living actors. Stored separately from `traits` because energy is a dynamic state variable, not a heritable trait. |
