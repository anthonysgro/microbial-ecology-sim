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
| `q` / `Esc` | Quit |

## Configuration

Grid parameters are set in code at the binary entry points (`src/main.rs` and `src/bin/bevy_viz.rs`). Key knobs:

| Parameter | Description |
|-----------|-------------|
| `width` / `height` | Grid dimensions |
| `num_chemicals` | Number of chemical species tracked per cell |
| `diffusion_rate` | Chemical diffusion coefficient |
| `thermal_conductivity` | Heat radiation coefficient |
| `min_actors` / `max_actors` | Actor count range at initialization |
| `tick_hz` | Simulation ticks per second (Bevy only) |

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
