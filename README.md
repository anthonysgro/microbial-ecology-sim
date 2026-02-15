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

### Bevy (graphical window)

```sh
cargo run --release --bin bevy_viz [seed]
```

Opens a GPU-accelerated window with a 128×128 grid, camera controls, and a HUD overlay.

## Seeds

Both binaries accept an optional integer seed as the first CLI argument. The seed controls all RNG: source placement, initial field values, actor spawning. Same seed = same world, deterministically.

```sh
cargo run --release --bin bevy_viz 99
```

Default seed is `42` if omitted.

### Bevy mode

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

All parameters are set in code at the binary entry points (`src/main.rs` and `src/bin/bevy_viz.rs`). There is no file-based config — changing values requires recompilation.

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

### Bevy-Only Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `tick_hz` | `f64` | Simulation ticks per second (controls simulation speed) |

## Project structure

```
src/
├── grid/           # Environment grid, cells, diffusion, heat, actors, tick orchestration
├── viz/            # Terminal visualization (crossterm)
├── viz_bevy/       # Bevy GPU visualization
├── bin/bevy_viz.rs # Bevy binary entry point
├── lib.rs          # Public API surface
└── main.rs         # Terminal binary entry point
```

## License

Unlicensed — private project.
