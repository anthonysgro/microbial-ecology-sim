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

Every field is documented with inline comments in `example_config.toml`. Press `i` in the Bevy visualization to see all active config values at runtime.

## Config Analyzer

A standalone CLI tool that statically analyzes a simulation config and prints a characterization of expected dynamics — without running the simulation. It computes numerical stability, chemical/energy budgets, carrying capacity, source density, and diffusion characteristics.

```sh
cargo run --release --bin config-analyzer -- --config example_config.toml
```

Sample output:

```
=== Config Analysis Report ===
Grid: 30x30 (900 cells)  |  Seed: 773  |  Tick: 1s  |  Actors: enabled

--- Numerical Stability ---
  Diffusion number:          0.8000
  [OK]   Chemical diffusion is stable

--- Chemical Budget ---
  Net chemical/tick:         8.0750
  [OK]   Chemical budget is positive

--- Energy Budget ---
  Net energy/tick:           -0.0050
  [WARN] Actors lose energy under average conditions
...
```

Lines prefixed `[WARN]` flag potential issues. `[OK]` confirms healthy parameters. The tool reuses the same TOML parsing and validation as the main binary, so any config file that works with the simulation works here.

## Project structure

```
src/
├── bin/             # Standalone binaries (config-analyzer)
├── grid/           # Environment grid, cells, diffusion, heat, actors, tick orchestration
├── io/             # CLI parsing, TOML config loading, validation, static analysis
├── viz_bevy/       # Bevy GPU visualization
├── lib.rs          # Public API surface
└── main.rs         # Bevy binary entry point
```

## License

Unlicensed — private project.
