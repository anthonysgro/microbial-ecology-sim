# Design Document: Thermal Fitness

## Overview

This feature introduces a multiplicative thermal fitness factor that degrades actor capabilities (consumption efficiency, movement cost) based on thermal mismatch between the actor's heritable `optimal_temp` and the cell's heat value. The current additive thermal cost remains as a baseline; the new factor layers multiplicative degradation on top.

The fitness factor uses a Gaussian decay curve: `exp(-mismatch² / (2 * width²))`, producing a smooth `[0, 1]` scalar that is `1.0` at zero mismatch and decays toward `0.0` as mismatch grows. The width parameter (`thermal_fitness_width`) controls the comfort zone breadth.

### Design Rationale

**Gaussian over linear/step functions**: The Gaussian curve is continuously differentiable, produces no discontinuities at threshold boundaries, and naturally maps to `[0, 1]` without clamping. It also mirrors real enzyme kinetics where activity follows a bell curve around optimal temperature.

**Multiplicative over additive**: The current additive thermal cost is a flat energy drain independent of what the actor is doing. A multiplicative factor scales with activity — actors that consume more or move more are penalized more heavily in hostile zones. This creates meaningful behavioral pressure without requiring large absolute penalty values.

**No sensing degradation in this iteration**: Sensing degradation (reduced gradient detection under thermal stress) is deferred. The primary selective pressure comes from consumption and movement. Sensing degradation can be added later if thermal niche partitioning is insufficient.

## Architecture

The thermal fitness computation is a pure function called inline within existing HOT-path systems. No new systems, no new ECS components, no new tick phases.

```
┌─────────────────────────────────────────────────────┐
│                  Tick Phase 3: HOT                   │
│                                                      │
│  run_actor_metabolism                                │
│    ├─ compute thermal_fitness(cell_heat, optimal_temp, width)
│    ├─ scale effective_conversion *= thermal_fitness   │
│    └─ (additive thermal_cost unchanged)              │
│                                                      │
│  run_actor_movement                                  │
│    ├─ compute thermal_fitness(cell_heat, optimal_temp, width)
│    └─ scale movement_cost /= max(thermal_fitness, 1/cap)
│                                                      │
└─────────────────────────────────────────────────────┘
```

### Integration Points

1. **`run_actor_metabolism`** in `src/grid/actor_systems.rs` — after computing `effective_conversion`, multiply by `thermal_fitness`. The existing additive `thermal_cost` line remains unchanged.

2. **`run_actor_movement`** in `src/grid/actor_systems.rs` — after computing the proportional movement cost, divide by `thermal_fitness` (capped to prevent division by zero). Requires passing `heat_read` slice and config to the movement function.

3. **`ActorConfig`** in `src/grid/actor_config.rs` — two new fields: `thermal_fitness_width` and `thermal_movement_cap`.

4. **`example_config.toml`** — new fields under the "Thermal Metabolism" section.

5. **`format_config_info`** in `src/viz_bevy/setup.rs` — display new fields.

6. **`config-documentation.md`** — update ActorConfig reference table.

## Components and Interfaces

### Pure Function: `thermal_fitness`

```rust
/// Compute the thermal fitness factor for an actor.
///
/// Returns a value in [0.0, 1.0]:
///   - 1.0 when cell_heat == optimal_temp (zero mismatch)
///   - Decays toward 0.0 as |cell_heat - optimal_temp| increases
///   - Exactly 1.0 when width == 0.0 (mechanic disabled)
///
/// Formula: exp(-mismatch² / (2 * width²))
///
/// HOT PATH: No allocation, no branching beyond the width==0 guard.
/// Deterministic for identical inputs.
#[inline]
pub(crate) fn thermal_fitness(cell_heat: f32, optimal_temp: f32, width: f32) -> f32 {
    if width == 0.0 {
        return 1.0;
    }
    let delta = cell_heat - optimal_temp;
    (-delta * delta / (2.0 * width * width)).exp()
}
```

### Modified Function Signatures

**`run_actor_movement`** — already receives `actor_config: &ActorConfig`. Needs an additional `heat_read: &[f32]` parameter to look up cell heat for the fitness computation.

New signature:
```rust
pub fn run_actor_movement(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    movement_targets: &[Option<usize>],
    actor_config: &ActorConfig,
    heat_read: &[f32],          // NEW: cell heat values for thermal fitness
) -> Result<(), TickError>
```

**`run_actor_metabolism`** — already receives `heat_read: &[f32]` and `config: &ActorConfig`. No signature change needed. The fitness factor is computed inline from existing parameters.

### ActorConfig Additions

```rust
/// Width of the Gaussian thermal fitness curve. Controls how quickly
/// capabilities degrade with thermal mismatch. Larger = wider comfort zone.
/// 0.0 disables the mechanic (fitness always 1.0).
/// Must be >= 0.0 and finite. Default: 0.5.
pub thermal_fitness_width: f32,

/// Maximum movement cost multiplier when thermal fitness approaches zero.
/// Caps the divisor to prevent infinite movement cost.
/// Must be > 1.0 and finite. Default: 5.0.
pub thermal_movement_cap: f32,
```

## Data Models

No new data structures. The thermal fitness factor is computed on-the-fly from existing data:

| Input | Source | Type |
|---|---|---|
| `cell_heat` | `heat_read[actor.cell_index]` | `f32` |
| `optimal_temp` | `actor.traits.optimal_temp` | `f32` |
| `thermal_fitness_width` | `config.thermal_fitness_width` | `f32` |
| `thermal_movement_cap` | `config.thermal_movement_cap` | `f32` |

The computed fitness factor is a transient `f32` local variable — not stored on the actor or in any buffer. This avoids adding state and keeps the computation purely functional.

### Metabolism Energy Balance (Modified)

Current:
```
energy += consumed * effective_conversion - base_energy_decay - thermal_cost - readiness_cost
```

New:
```
fitness = thermal_fitness(cell_heat, optimal_temp, thermal_fitness_width)
energy += consumed * effective_conversion * fitness - base_energy_decay - thermal_cost - readiness_cost
```

The `thermal_cost` (additive) remains. The `fitness` (multiplicative) scales only the consumption gain term.

### Movement Cost (Modified)

Current:
```
proportional = base * (energy / reference) / metabolic_ratio
actual = max(proportional, floor)
```

New:
```
fitness = thermal_fitness(cell_heat, optimal_temp, thermal_fitness_width)
capped_fitness = max(fitness, 1.0 / thermal_movement_cap)
proportional = base * (energy / reference) / metabolic_ratio / capped_fitness
actual = max(proportional, floor)
```

When `fitness == 1.0` (at optimal temp), cost is unchanged. When `fitness → 0.0`, cost is multiplied by `thermal_movement_cap` (default 5×).

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Thermal fitness function correctness

*For any* finite `cell_heat`, finite `optimal_temp`, and finite positive `thermal_fitness_width`:
- The return value is in `[0.0, 1.0]`
- When `cell_heat == optimal_temp`, the return value is exactly `1.0`
- For two mismatch values where `|m1| < |m2|`, `fitness(m1) >= fitness(m2)` (monotonic decrease with distance)
- When `thermal_fitness_width == 0.0`, the return value is `1.0` regardless of mismatch

**Validates: Requirements 1.1, 1.2, 1.3, 4.3**

### Property 2: Metabolism energy balance with thermal fitness

*For any* non-inert actor with positive energy, on a cell with known chemical concentration and heat value, after one metabolism tick:
- The energy delta equals `consumed * effective_conversion * thermal_fitness - base_energy_decay - thermal_cost - readiness_cost`
- Where `thermal_fitness = thermal_fitness(cell_heat, optimal_temp, width)` and `thermal_cost = thermal_sensitivity * (cell_heat - optimal_temp)²`
- When `thermal_fitness == 1.0` (cell_heat == optimal_temp), the energy delta is identical to the pre-feature formula

**Validates: Requirements 2.1, 2.2, 2.3, 2.4**

### Property 3: Movement cost with thermal fitness and cap

*For any* non-inert actor that successfully moves to a new cell with known heat value:
- The energy deducted equals `max(base * (energy / reference) / metabolic_ratio / max(fitness, 1/cap), floor)`
- Where `fitness = thermal_fitness(cell_heat, optimal_temp, width)` and `cap = thermal_movement_cap`
- The effective cost multiplier never exceeds `thermal_movement_cap` (even when fitness == 0.0)
- When `fitness == 1.0`, the cost is identical to the pre-feature formula

**Validates: Requirements 3.1, 3.2, 3.3, 3.4**

## Error Handling

The `thermal_fitness` function returns values in `[0.0, 1.0]` for all finite inputs by construction (`exp` of a non-positive number). The only risk is NaN propagation from upstream (e.g., NaN in `heat_read` or `optimal_temp`).

Existing NaN/Inf guards in `run_actor_metabolism` and `run_actor_movement` already catch propagated NaN after the energy update. No additional error handling is needed — the existing `TickError::NumericalError` path covers this.

The `width == 0.0` guard in `thermal_fitness` prevents division by zero in the Gaussian formula. The `max(fitness, 1.0 / cap)` guard in movement prevents division by zero when fitness approaches 0.0.

Config validation rejects `thermal_fitness_width < 0.0`, non-finite values, `thermal_movement_cap <= 1.0`, and non-finite cap values at load time.

## Testing Strategy

### Property-Based Tests

Use the `proptest` crate (already available in the project's test dependencies or easily added). Each property test runs a minimum of 100 iterations with randomly generated inputs.

**Property 1** — Generate random `(cell_heat, optimal_temp, width)` triples with finite f32 values. Verify all three sub-properties (range, identity at zero mismatch, monotonicity) in a single test. Tag: `Feature: thermal-fitness, Property 1: Thermal fitness function correctness`.

**Property 2** — Generate random actor state (energy, traits), random cell chemical concentration, and random cell heat. Run one metabolism tick and verify the energy delta matches the expected formula. Tag: `Feature: thermal-fitness, Property 2: Metabolism energy balance with thermal fitness`.

**Property 3** — Generate random actor state, random movement target, and random cell heat. Execute one movement step and verify the energy deducted matches the expected formula including the cap. Tag: `Feature: thermal-fitness, Property 3: Movement cost with thermal fitness and cap`.

### Unit Tests

- `thermal_fitness(x, x, w) == 1.0` for specific values (sanity check)
- `thermal_fitness(x, y, 0.0) == 1.0` for specific mismatched values (disabled mechanic)
- Movement cost at fitness == 0.0 equals base_cost * cap (cap enforcement)
- Config validation rejects `thermal_fitness_width = -1.0`
- Config validation rejects `thermal_movement_cap = 0.5`
- Existing metabolism and movement tests pass unchanged when `thermal_fitness_width = 0.0` (regression)

### Integration

Existing tests in `actor_systems.rs` use `default_config()` which will get `thermal_fitness_width = 0.5` (or can be set to `0.0` to preserve pre-feature behavior). Tests that set `heat_read` to `optimal_temp` already produce fitness == 1.0 and should pass without modification.
