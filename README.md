# Emergent Sovereignty

A headless, high-performance biological simulation engine written in Rust. All macro-structures — species, ecosystems, symbiotic networks — emerge purely from micro-interactions between physical actors on a cellular automaton grid. There are no top-down abstractions: no health bars, no species classes, no global economy. Just physics, chemistry, and local perception.

## What it does

The simulation runs a 2D grid of cells, each carrying persistent physical state (chemical gradients, heat). Actors — autonomous biological organisms — inhabit the grid and interact locally: consuming resources, exchanging compounds, sensing neighbors, and competing for territory through chemical signals. Diffusion, thermal radiation, and metabolic processes run as independent systems every tick.

## Building

Requires Rust 1.75+ (2024 edition).

```sh
cargo build --release
```

## Running

```sh
cargo run --release
```

Opens a GPU-accelerated Bevy window with camera controls and a HUD overlay.

## Seeds

The binary accepts an optional integer seed as a positional CLI argument. The seed controls all RNG: source placement, initial field values, actor spawning. Same seed = same world, deterministically.

```sh
cargo run --release -- 99
```

Default seed is `42` if omitted.

### Keyboard & Mouse Controls

| Key / Input | Action |
|-------------|--------|
| `1`–`9` | Switch to chemical overlay (species index) |
| `h` | Heat overlay |
| `Space` | Pause / resume simulation |
| `↑` | Increase tick rate |
| `↓` | Decrease tick rate |
| `r` | Reset tick rate to default |
| Scroll wheel | Zoom in / out |
| Middle mouse drag | Pan camera |
| `i` | Show / hide config info panel |
| `q` / `Esc` | Quit |

## Configuration

The simulation is configured via a TOML file. Pass it with `--config`:

```sh
cargo run --release -- --config example_config.toml
```

An optional positional seed argument overrides the seed in the TOML file:

```sh
cargo run --release -- --config example_config.toml 99
```

If no `--config` is provided, compiled defaults are used. Every field in the TOML file is optional — omit any field or entire section to use its default. Unknown keys are rejected at parse time.

See `example_config.toml` for a complete annotated example with all defaults.

### Precedence

1. CLI seed argument (highest)
2. TOML file values
3. Compiled defaults (lowest)

### TOML Structure

The config file has four top-level sections:

```toml
seed = 42          # Global RNG seed

[grid]             # Environment physics
[world_init]       # Procedural generation parameters
[actor]            # Actor metabolism & lifecycle (omit for no actors)
[bevy]             # Bevy visualization settings (ignored by headless binary)
```

Three config structs control world initialization:

### GridConfig — Environment Physics

| Parameter | Type | Description |
|-----------|------|-------------|
| `width` | `u32` | Grid width in cells |
| `height` | `u32` | Grid height in cells |
| `num_chemicals` | `usize` | Number of chemical species tracked per cell |
| `diffusion_rate` | `f32` | Chemical diffusion coefficient (discrete Laplacian scaling factor) |
| `thermal_conductivity` | `f32` | Heat radiation coefficient |
| `ambient_heat` | `f32` | Boundary condition for heat — missing neighbors use this value |
| `tick_duration` | `f32` | Simulated time per tick (seconds) |
| `num_threads` | `usize` | Spatial partition count (maps to rayon thread count) |
| `chemical_decay_rates` | `Vec<f32>` | Per-species decay rate in `[0.0, 1.0]`. Applied as `concentration *= (1.0 - rate)` each tick. Length must equal `num_chemicals` |

### WorldInitConfig — Procedural Generation

Controls how the world is seeded. All ranges are inclusive `[min, max]`.

| Parameter | Type | Description |
|-----------|------|-------------|
| `min_initial_heat` / `max_initial_heat` | `f32` | Per-cell heat value range at spawn |
| `min_initial_concentration` / `max_initial_concentration` | `f32` | Per-cell chemical concentration range at spawn (all species) |
| `min_actors` / `max_actors` | `u32` | Actor count range to seed. Both `0` = no actors |

Each of `heat_source_config` and `chemical_source_config` is a `SourceFieldConfig`:

| Parameter | Type | Description |
|-----------|------|-------------|
| `min_sources` / `max_sources` | `u32` | Number of sources to place |
| `min_emission_rate` / `max_emission_rate` | `f32` | Source emission rate (units per tick) |
| `renewable_fraction` | `f32` | Fraction of sources that are renewable `[0.0, 1.0]` |
| `min_reservoir_capacity` / `max_reservoir_capacity` | `f32` | Initial reservoir for non-renewable sources |
| `min_deceleration_threshold` / `max_deceleration_threshold` | `f32` | When non-renewable sources begin tapering `[0.0, 1.0]` |

### ActorConfig — Metabolism & Lifecycle

Optional — pass `None` to `initialize()` to skip actor systems entirely.

| Parameter | Type | Description |
|-----------|------|-------------|
| `consumption_rate` | `f32` | Chemical units consumed per tick from the actor's current cell (species 0) |
| `energy_conversion_factor` | `f32` | Energy gained per unit of chemical consumed |
| `base_energy_decay` | `f32` | Basal metabolic cost subtracted each tick |
| `initial_energy` | `f32` | Energy assigned to newly spawned actors |
| `initial_actor_capacity` | `usize` | Pre-allocated slot capacity for the actor registry |
| `movement_cost` | `f32` | Energy subtracted on successful move to an adjacent cell |
| `removal_threshold` | `f32` | Energy level below which an inert actor is permanently removed (must be ≤ 0.0) |

### BevyExtras — Visualization Settings

Only consumed by the Bevy binary. Ignored if running headless. Lives under `[bevy]` in the TOML file.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `tick_hz` | `f64` | `10.0` | Simulation ticks per second |
| `zoom_min` | `f32` | `0.1` | Minimum camera zoom level |
| `zoom_max` | `f32` | `10.0` | Maximum camera zoom level |
| `zoom_speed` | `f32` | `0.1` | Scroll-wheel zoom sensitivity |
| `pan_speed` | `f32` | `1.0` | Middle-mouse pan sensitivity |
| `color_scale_max` | `f32` | `10.0` | Upper bound for overlay color normalization |

## Project structure

```
src/
├── grid/           # Environment grid, cells, diffusion, heat, actors, tick orchestration
├── io/             # CLI parsing, TOML config loading, validation
├── viz_bevy/       # Bevy GPU visualization
├── lib.rs          # Public API surface
└── main.rs         # Bevy binary entry point
```

## License

Unlicensed — private project.
