# World Configuration Presets

Curated starting configurations that push the simulation into qualitatively different regimes. Each preset is designed to highlight specific evolutionary dynamics and environmental pressures.

Pick one, copy it to your working config, and tweak from there.

## Presets

| Preset | Grid | Key Dynamic | What to Watch |
|---|---|---|---|
| `archipelago.toml` | 400×400 | Isolated resource islands with empty ocean between | Memory evolution, long-range foraging, migration patterns |
| `thermal-crucible.toml` | 400×400 | Extreme heat gradients from massive emitters | Thermal speciation, spatial niche partitioning |
| `feast-and-famine.toml` | 400×400 | All non-renewable sources, no respawn | Population boom/crash cycles, bottleneck survival strategies |
| `dense-petri-dish.toml` | 60×60 | Packed grid, abundant resources, high contact | Predation arms race, kin defense, social dynamics |
| `sparse-savanna.toml` | 400×400 | Large grid, few weak sources, wide spacing | K-strategy selection, efficient foraging, low reproduction |
| `cognitive-pressure.toml` | 400×400 | Respawning clustered sources, active predation | Memory-biased sensing payoff, site fidelity vs avoidance |

## Usage

```bash
# Run with a preset
cargo run --release -- --config configs/archipelago.toml

# Or with the Bevy visualizer
cargo run --release --bin viz -- --config configs/thermal-crucible.toml
```

## Tuning Tips

- Start with a preset and run for 10k–50k ticks to see the initial dynamics
- Adjust `seed` to explore different random layouts with the same parameters
- The `[bevy]` section is optional — omit it for headless runs
- Check `example_config.toml` for full documentation of every field
