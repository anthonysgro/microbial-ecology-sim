# Configuration Documentation — Steering Rule

When any spec adds, removes, or modifies a configuration field (in `ActorConfig`, `GridConfig`, `WorldInitConfig`, or any other config struct):

1. **`example_config.toml`** — Update the example config file to include the new/changed field with a comment explaining its purpose and valid range.
2. **`README.md`** — Update the README if it documents configuration parameters.
3. **World config info panel** — Update `format_config_info()` in `src/viz_bevy/setup.rs` to display the new/changed field. This panel is toggled by pressing `I` in the Bevy visualization and must reflect all active configuration values.
4. **This steering file** — Update the config reference below to reflect the new/changed field so this document stays the single source of truth.
5. **Spec requirements** — Include a documentation update requirement in the spec so it appears in the task list and is not forgotten during implementation.

This ensures configuration documentation stays in sync with the code at all times.

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
| `num_chemicals` | `usize` | `1` | Number of chemical species tracked per cell. |
| `diffusion_rate` | `f32` | `0.05` | Chemical diffusion coefficient (discrete Laplacian scaling). Stability: `diffusion_rate * tick_duration * 8 < 1.0`. |
| `thermal_conductivity` | `f32` | `0.05` | Heat radiation coefficient. |
| `ambient_heat` | `f32` | `0.0` | Boundary condition for heat: missing neighbors use this value. |
| `tick_duration` | `f32` | `1.0` | Simulated time per tick (seconds). |
| `num_threads` | `usize` | `4` | Number of spatial partitions (maps to rayon thread count). |
| `chemical_decay_rates` | `Vec<f32>` | `[0.05]` | Per-species decay rate. Length must equal `num_chemicals`. Each value in `[0.0, 1.0]`. Applied as `concentration *= (1.0 - rate)` per tick. |

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

### `[world_init.chemical_source_config]` — `SourceFieldConfig` (chemical)

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
| `initial_energy` | `f32` | `10.0` | Energy assigned to newly spawned actors. Must be `<= max_energy`. |
| `max_energy` | `f32` | `50.0` | Maximum energy an actor can hold. Clamped after each metabolic tick. Must be `> 0.0`, finite, `>= initial_energy`. |
| `initial_actor_capacity` | `usize` | `64` | Pre-allocated slot capacity for the ActorRegistry. |
| `movement_cost` | `f32` | `0.5` | Energy subtracted when an actor successfully moves to an adjacent cell. |
| `removal_threshold` | `f32` | `-5.0` | Energy level below which an inert actor is permanently removed. Must be `<= 0.0`. |

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
