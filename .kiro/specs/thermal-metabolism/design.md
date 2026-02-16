# Design Document: Thermal Metabolism

## Overview

This feature adds a quadratic thermal performance curve to actor metabolism. Each actor carries a heritable `optimal_temp` trait. During the metabolism phase, the squared difference between the actor's local cell heat and its `optimal_temp` is multiplied by a global `thermal_sensitivity` coefficient to produce an extra per-tick energy cost. This creates selective pressure for thermal specialization: populations near heat sources evolve higher `optimal_temp`, while populations in cooler regions evolve lower values. As heat sources deplete and respawn, populations track the shifting thermal landscape.

The implementation touches:
- `HeritableTraits` struct (new field + mutation + from_config)
- `ActorConfig` (seed default, clamp bounds, sensitivity parameter)
- `run_actor_metabolism` (thermal penalty computation in the HOT path)
- `genetic_distance` (include `optimal_temp` in distance, bump `TRAIT_COUNT`)
- Visualization layer (stats collection, panel formatting, actor inspector)
- Configuration documentation (example_config.toml, config info panel, steering file)

Design decision: `thermal_sensitivity` is a global `ActorConfig` parameter, not a heritable trait. Rationale: the sensitivity controls the shape of the fitness landscape itself. Making it heritable would allow actors to evolve away the penalty entirely (by driving sensitivity toward zero), defeating the purpose of thermal selection pressure. A global parameter keeps the selection pressure externally tunable.

## Architecture

The thermal penalty integrates into the existing metabolism HOT path with minimal structural change:

```
┌─────────────────────────────────────────────────────────┐
│                  run_actor_phases (tick.rs)              │
│                                                         │
│  Phase 2: Metabolism                                    │
│  ┌───────────────────────────────────────────────────┐  │
│  │  grid.read_heat() ──► heat_read: &[f32]           │  │
│  │                                                   │  │
│  │  for each active actor:                           │  │
│  │    cell_heat = heat_read[actor.cell_index]        │  │
│  │    delta = cell_heat - actor.traits.optimal_temp  │  │
│  │    thermal_cost = config.thermal_sensitivity      │  │
│  │                   * delta * delta                 │  │
│  │                                                   │  │
│  │    energy += consumed * effective_conversion       │  │
│  │           - base_energy_decay                     │  │
│  │           - thermal_cost          ◄── NEW         │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

1. `run_actor_phases` in `tick.rs` reads the heat buffer via `grid.read_heat()` and passes `heat_read: &[f32]` as a new parameter to `run_actor_metabolism`.
2. `run_actor_metabolism` indexes `heat_read[actor.cell_index]` for each active actor — same pattern as `chemical_read[ci]`.
3. The thermal cost is a pure arithmetic computation: one subtraction, one multiply, one multiply. Zero allocations, no branching beyond the existing inert check.

### Trait Inheritance Flow

```
ActorConfig::optimal_temp (seed default)
        │
        ▼
HeritableTraits::from_config() ──► initial optimal_temp
        │
        ▼ (on reproduction)
HeritableTraits::mutate() ──► proportional gaussian mutation
        │                      clamped to [trait_optimal_temp_min, trait_optimal_temp_max]
        ▼
offspring.traits.optimal_temp
```

## Components and Interfaces

### Modified: `HeritableTraits` (src/grid/actor.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeritableTraits {
    // ... existing 9 fields ...
    pub consumption_rate: f32,
    pub base_energy_decay: f32,
    pub levy_exponent: f32,
    pub reproduction_threshold: f32,
    pub max_tumble_steps: u16,
    pub reproduction_cost: f32,
    pub offspring_energy: f32,
    pub mutation_rate: f32,
    pub kin_tolerance: f32,
    /// Preferred cell heat for minimal thermal penalty.
    pub optimal_temp: f32,  // NEW — 10th heritable trait
}
```

Size increases from 36 to 40 bytes (one f32 added). The static size assertion must be updated.

`from_config`: reads `config.optimal_temp` for the seed value.
`mutate`: applies `optimal_temp * (1.0 + Normal(0, mutation_rate))`, clamped to `[trait_optimal_temp_min, trait_optimal_temp_max]`.

### Modified: `ActorConfig` (src/grid/actor_config.rs)

New fields:

| Field | Type | Default | Description |
|---|---|---|---|
| `thermal_sensitivity` | `f32` | `0.01` | Quadratic penalty coefficient. Must be >= 0.0 and finite. |
| `optimal_temp` | `f32` | `0.5` | Seed genome default for heritable `optimal_temp`. |
| `trait_optimal_temp_min` | `f32` | `0.0` | Minimum clamp bound for heritable `optimal_temp`. |
| `trait_optimal_temp_max` | `f32` | `2.0` | Maximum clamp bound for heritable `optimal_temp`. |

Default `optimal_temp = 0.5` sits in the middle of the typical initial heat range (`0.1..0.6` in example_config.toml). Default `thermal_sensitivity = 0.01` produces a mild penalty — at a mismatch of 1.0 heat unit, the extra cost is 0.01 energy/tick, comparable to ~10-20% of `base_energy_decay` at default settings. This is enough to create selection pressure without immediately killing mismatched actors.

Default `trait_optimal_temp_max = 2.0` provides headroom above the typical `max_initial_heat` (0.6) and `max_emission_rate` (3.0–5.0) to allow actors to evolve toward high-heat niches near strong sources.

### Modified: `run_actor_metabolism` (src/grid/actor_systems.rs)

New parameter: `heat_read: &[f32]` — the grid's heat read buffer.

For active (non-inert) actors, after computing `consumed * effective_conversion`, the thermal cost is subtracted alongside `base_energy_decay`:

```rust
let cell_heat = heat_read[ci];
let delta = cell_heat - actor.traits.optimal_temp;
let thermal_cost = config.thermal_sensitivity * delta * delta;

actor.energy += consumed * effective_conversion
    - actor.traits.base_energy_decay
    - thermal_cost;
```

No new branches. No allocations. The existing NaN/Inf check covers the thermal cost path.

### Modified: `genetic_distance` (src/grid/actor_systems.rs)

`TRAIT_COUNT` changes from 9 to 10. The traits array gains one entry:

```rust
(a.optimal_temp, b.optimal_temp, config.trait_optimal_temp_min, config.trait_optimal_temp_max),
```

### Modified: `run_actor_phases` (src/grid/tick.rs)

Phase 2 (Metabolism) reads the heat buffer before calling `run_actor_metabolism`:

```rust
let heat_read = grid.read_heat();
run_actor_metabolism(
    &mut actors,
    chemical_read,
    chemical_write,
    heat_read,       // NEW parameter
    &actor_config,
    &mut removal_buffer,
)?;
```

### Modified: Visualization Layer

| File | Change |
|---|---|
| `src/viz_bevy/resources.rs` | `TraitStats.traits`: `[SingleTraitStats; 9]` → `[SingleTraitStats; 10]` |
| `src/viz_bevy/systems.rs` | `compute_trait_stats_from_actors`: add `optimal_temp` buffer (10th), collect values |
| `src/viz_bevy/setup.rs` | `TRAIT_NAMES`: append `"optimal_temp"` (10 entries) |
| `src/viz_bevy/setup.rs` | `format_actor_info`: append `optimal_temp` line |
| `src/viz_bevy/setup.rs` | `format_config_info`: append `thermal_sensitivity`, `optimal_temp`, `trait_optimal_temp` range |

`format_trait_stats` iterates `TRAIT_NAMES` — no logic change beyond the array size.

## Data Models

### HeritableTraits (updated)

```
HeritableTraits (40 bytes)
├── consumption_rate: f32
├── base_energy_decay: f32
├── levy_exponent: f32
├── reproduction_threshold: f32
├── max_tumble_steps: u16
├── (2 bytes padding)
├── reproduction_cost: f32
├── offspring_energy: f32
├── mutation_rate: f32
├── kin_tolerance: f32
└── optimal_temp: f32          ◄── NEW
```

Adding `optimal_temp` after `kin_tolerance` keeps the struct naturally aligned (f32 after f32, no extra padding). Size goes from 36 → 40 bytes. The compile-time size assertion must be updated to `assert!(size_of::<HeritableTraits>() == 40)`.

### ActorConfig (new fields)

```
ActorConfig (existing fields + 4 new)
├── ... existing fields ...
├── thermal_sensitivity: f32     ◄── NEW (global, not heritable)
├── optimal_temp: f32            ◄── NEW (seed genome default)
├── trait_optimal_temp_min: f32  ◄── NEW (clamp bound)
└── trait_optimal_temp_max: f32  ◄── NEW (clamp bound)
```

Serde default functions follow the existing pattern:
- `fn default_thermal_sensitivity() -> f32 { 0.01 }`
- `fn default_optimal_temp() -> f32 { 0.5 }`
- `fn default_trait_optimal_temp_min() -> f32 { 0.0 }`
- `fn default_trait_optimal_temp_max() -> f32 { 2.0 }`


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: from_config initializes optimal_temp from config

*For any* valid `ActorConfig`, calling `HeritableTraits::from_config(&config)` should produce a `HeritableTraits` where `optimal_temp == config.optimal_temp`.

**Validates: Requirements 1.2**

### Property 2: mutate clamps optimal_temp within configured bounds

*For any* `HeritableTraits` and any RNG state, after calling `mutate(&config, &mut rng)`, the resulting `optimal_temp` must satisfy `config.trait_optimal_temp_min <= optimal_temp <= config.trait_optimal_temp_max`.

**Validates: Requirements 1.3**

### Property 3: zero thermal_sensitivity produces zero thermal penalty

*For any* set of actors with arbitrary `optimal_temp` values and any heat buffer, when `config.thermal_sensitivity == 0.0`, the energy change from `run_actor_metabolism` should be identical to the energy change computed without any thermal penalty term (i.e., `consumed * effective_conversion - base_energy_decay`).

**Validates: Requirements 2.3**

### Property 4: thermal penalty follows quadratic formula

*For any* active actor with known `optimal_temp`, any cell heat value, and any `thermal_sensitivity >= 0.0`, the energy delta from `run_actor_metabolism` should equal `consumed * effective_conversion - base_energy_decay - thermal_sensitivity * (cell_heat - optimal_temp)^2`, clamped to `max_energy`.

**Validates: Requirements 3.2, 3.3**

### Property 5: inert actors receive no thermal penalty

*For any* inert actor and any heat buffer values, the energy change from `run_actor_metabolism` should equal exactly `-base_energy_decay` with no thermal component.

**Validates: Requirements 3.4**

### Property 6: genetic_distance reflects optimal_temp differences

*For any* two `HeritableTraits` that are identical except for `optimal_temp`, `genetic_distance` should return a value greater than 0.0 (proportional to the normalized difference in `optimal_temp`).

**Validates: Requirements 4.1**

### Property 7: trait stats include optimal_temp statistics

*For any* non-empty set of non-inert actors, `compute_trait_stats_from_actors` should produce a `TraitStats` where `traits[9]` (the 10th element) has `min <= mean <= max` and `min` equals the minimum `optimal_temp` across all non-inert actors.

**Validates: Requirements 5.2**

### Property 8: actor inspector displays optimal_temp

*For any* `Actor`, `format_actor_info` should produce a string containing `"optimal_temp"` and the actor's `optimal_temp` value.

**Validates: Requirements 5.4**

### Property 9: config info panel displays thermal metabolism fields

*For any* `ActorConfig`, `format_config_info` should produce a string containing `"thermal_sensitivity"`, `"optimal_temp"`, and `"trait_optimal_temp"`.

**Validates: Requirements 6.2**

## Error Handling

All error handling follows existing patterns in the metabolism system:

1. **NaN/Inf energy after thermal penalty**: The existing `actor.energy.is_nan() || actor.energy.is_infinite()` check in `run_actor_metabolism` covers the thermal cost path. No new error variants needed — the existing `TickError::NumericalError` with `system: "actor_metabolism"` is returned.

2. **Config validation**: `thermal_sensitivity` must be `>= 0.0` and finite. `optimal_temp` must be within `[trait_optimal_temp_min, trait_optimal_temp_max]`. `trait_optimal_temp_min < trait_optimal_temp_max`. Validation follows the existing pattern — either at deserialization time or via debug assertions.

3. **Heat buffer indexing**: `heat_read[ci]` uses the same `cell_index` already validated by the actor registry. No new bounds checking needed — the heat buffer has the same length as the chemical buffers (one entry per cell).

4. **No new error types**: The feature introduces no new failure modes beyond what the existing NaN/Inf check and config validation handle.

## Testing Strategy

### Property-Based Tests (proptest)

Each correctness property maps to a single `proptest` test. Minimum 100 iterations per test (proptest default is 256, which exceeds this).

| Property | Test Description | Generator Strategy |
|---|---|---|
| 1 | from_config optimal_temp | Generate random ActorConfig with valid optimal_temp within clamp bounds |
| 2 | mutate clamp invariant | Generate random HeritableTraits + ActorConfig + RNG seed; verify post-mutate bounds |
| 3 | zero sensitivity = no penalty | Generate actors + heat values with thermal_sensitivity=0.0; compare energy to baseline |
| 4 | quadratic formula | Generate single actor + heat value + config; verify energy delta matches formula |
| 5 | inert no thermal penalty | Generate inert actor + heat values; verify energy delta = -base_energy_decay |
| 6 | genetic_distance includes optimal_temp | Generate two identical HeritableTraits, vary only optimal_temp; verify distance > 0 |
| 7 | stats include optimal_temp | Generate N actors with random optimal_temp; verify stats[9].min = actual min |
| 8 | actor inspector shows optimal_temp | Generate random Actor; verify format_actor_info output contains "optimal_temp" |
| 9 | config info shows fields | Generate random ActorConfig; verify format_config_info output contains field names |

### Unit Tests

Unit tests complement property tests for specific examples and edge cases:

- **Edge case**: `thermal_sensitivity = 0.0` with extreme heat mismatch — verify zero penalty
- **Edge case**: `optimal_temp == cell_heat` — verify zero thermal cost
- **Edge case**: NaN energy after thermal penalty — verify `TickError::NumericalError` returned
- **Edge case**: Config with `trait_optimal_temp_min >= trait_optimal_temp_max` — verify rejection
- **Example**: Known values — actor with `optimal_temp=0.5`, `cell_heat=1.5`, `thermal_sensitivity=0.01` → `thermal_cost = 0.01 * 1.0^2 = 0.01`

### Test Organization

Property tests and unit tests for the thermal penalty live in `src/grid/actor_systems.rs` (existing test module). Visualization tests live in `src/viz_bevy/setup.rs` or `src/viz_bevy/systems.rs` test modules.

Property-based testing library: `proptest` (already available in the project's test dependencies or to be added).

Each property test is tagged with a comment:
```rust
// Feature: thermal-metabolism, Property N: <property_text>
```
