# Design Document: Energy-Mass Movement

## Overview

This feature replaces the flat movement cost with an energy-proportional formula where an actor's current energy acts as its physical mass. The core computation is:

```
actual_cost = max(base_movement_cost * (actor.energy / reference_energy), base_movement_cost * 0.1)
```

The change is localized to the movement system (`run_actor_movement`), the configuration struct (`ActorConfig`), and documentation/visualization surfaces. No new systems, components, or data structures are introduced. The movement system remains WARM-path classified.

## Architecture

The change touches three layers:

1. **Configuration layer** — `ActorConfig` gains `base_movement_cost` and `reference_energy`, drops `movement_cost`.
2. **Simulation layer** — `run_actor_movement` receives `&ActorConfig` instead of a single `f32`, computes per-actor cost.
3. **Visualization/documentation layer** — `format_config_info`, `example_config.toml`, and `config-documentation.md` reflect the new fields.

No new modules, no new systems, no new ECS components. The formula is pure arithmetic on existing actor state.

### Data Flow

```mermaid
graph LR
    A[ActorConfig] -->|base_movement_cost, reference_energy| B[run_actor_movement]
    C[Actor.energy] -->|current energy at move time| B
    B -->|actual_cost = max\(base * energy/ref, base * 0.1\)| D[Actor.energy -= actual_cost]
    D -->|energy <= 0| E[actor.inert = true]
```

## Components and Interfaces

### Modified: `ActorConfig` (`src/grid/actor_config.rs`)

Remove `movement_cost: f32` field. Add:

```rust
/// Base energy cost for movement at the reference energy level.
/// Actual cost scales proportionally with actor energy.
/// Must be >= 0.0. Default: 0.5.
pub base_movement_cost: f32,

/// Energy level at which movement cost equals base_movement_cost.
/// Actors above this pay more; actors below pay less.
/// Must be > 0.0. Default: 25.0.
pub reference_energy: f32,
```

The `Default` impl sets `base_movement_cost: 0.5` and `reference_energy: 25.0`. The `deny_unknown_fields` serde attribute on `ActorConfig` ensures old configs with `movement_cost` are rejected at parse time.

### Modified: `run_actor_movement` (`src/grid/actor_systems.rs`)

Current signature:

```rust
pub fn run_actor_movement(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    movement_targets: &[Option<usize>],
    movement_cost: f32,
) -> Result<(), TickError>
```

New signature:

```rust
pub fn run_actor_movement(
    actors: &mut ActorRegistry,
    occupancy: &mut [Option<usize>],
    movement_targets: &[Option<usize>],
    actor_config: &ActorConfig,
) -> Result<(), TickError>
```

Passing `&ActorConfig` instead of a single `f32` gives the function access to both `base_movement_cost` and `reference_energy`. The per-actor cost computation inside the loop:

```rust
let base = actor_config.base_movement_cost;
let reference = actor_config.reference_energy;
let floor = base * 0.1;

// Per-actor inside the loop, after successful move:
let proportional_cost = base * (actor.energy / reference);
let actual_cost = if proportional_cost > floor { proportional_cost } else { floor };
```

This uses a branch rather than `f32::max()` to avoid any platform-dependent NaN propagation behavior from `max`. The NaN check immediately after catches any numerical issues.

### Modified: Call site in `tick.rs`

```rust
// Before:
run_actor_movement(&mut actors, &mut occupancy, &movement_targets, actor_config.movement_cost)?;

// After:
run_actor_movement(&mut actors, &mut occupancy, &movement_targets, &actor_config)?;
```

### Modified: `format_config_info` (`src/viz_bevy/setup.rs`)

Replace the `movement_cost` display line with `base_movement_cost` and `reference_energy`.

### Modified: `format_actor_info` (`src/viz_bevy/setup.rs`)

No changes needed — this function displays per-actor heritable traits, and movement cost parameters are global config, not per-actor traits.

## Data Models

No new data models. The only structural change is two fields on `ActorConfig`:

| Field | Type | Default | Constraint |
|---|---|---|---|
| `base_movement_cost` | `f32` | `0.5` | `>= 0.0` |
| `reference_energy` | `f32` | `25.0` | `> 0.0` |

The removed field:

| Field | Type | Was Default |
|---|---|---|
| `movement_cost` | `f32` | `0.5` |

### Why not make `base_movement_cost` heritable?

This is intentionally left as a global config parameter for this spec. Making it heritable would add a ninth trait to `HeritableTraits`, change the struct size (currently exactly 32 bytes), require new clamp bounds, and expand the mutation surface. That's a separate design decision that can be evaluated after observing the energy-mass dynamics. The formula already creates per-actor variation through the energy term — actors with different energy levels naturally pay different costs.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Movement cost formula correctness

*For any* actor with energy `e > 0`, and any valid `ActorConfig` with `base_movement_cost >= 0` and `reference_energy > 0`, the energy after a successful move shall equal `e - max(base_movement_cost * (e / reference_energy), base_movement_cost * 0.1)`.

This is the core formula property. It subsumes the individual proportionality checks (above/below/at reference energy) and the floor guarantee. By verifying the complete formula across randomly generated energy values and config parameters, we validate the entire cost computation in one property.

**Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3**

### Property 2: Configuration validation rejects invalid parameters

*For any* `reference_energy <= 0.0` or `base_movement_cost < 0.0`, parsing a TOML configuration containing that value shall produce an error.

Combines the two validation requirements into a single property over the space of invalid config values.

**Validates: Requirements 3.3, 3.4**

### Property 3: Configuration TOML round-trip

*For any* valid `ActorConfig` with `base_movement_cost >= 0.0` and `reference_energy > 0.0`, serializing to TOML and parsing back shall produce an `ActorConfig` with identical `base_movement_cost` and `reference_energy` values.

**Validates: Requirements 3.5**

### Property 4: Inert transition on energy depletion

*For any* actor whose energy after the movement cost deduction is `<= 0.0`, the movement system shall set `actor.inert = true`.

**Validates: Requirements 5.1**

### Property 5: Inert actors are immobile

*For any* actor where `actor.inert == true` before the movement system runs, the actor's `cell_index` and `energy` shall remain unchanged after the movement system completes.

**Validates: Requirements 5.2**

## Error Handling

The movement system already returns `Result<(), TickError>`. The error handling strategy remains unchanged:

1. **NaN/Inf check** — After computing and applying the movement cost, the existing NaN/Inf check on `actor.energy` catches any numerical instability. The formula `base * (energy / reference)` can only produce NaN if `reference` is NaN (rejected at config validation) or if `energy` is already NaN (caught by upstream metabolism checks). Division by a positive finite `reference_energy` with a finite `energy` produces a finite result.

2. **Config validation** — `reference_energy > 0.0` is enforced at parse time. This prevents division-by-zero in the formula. `base_movement_cost >= 0.0` prevents negative costs. Validation should be added as a post-deserialization check in `ActorConfig`, consistent with how other constraints (e.g., `max_energy > 0.0`) are validated.

3. **No new error variants** — The existing `TickError::NumericalError` is sufficient. No new error types needed.

## Testing Strategy

### Property-Based Tests

Use the `proptest` crate (already available in the Rust ecosystem, zero runtime overhead, deterministic shrinking). Each property test runs a minimum of 256 iterations.

Each test is tagged with a comment referencing the design property:

```rust
// Feature: energy-mass-movement, Property 1: Movement cost formula correctness
// Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3
```

**Property 1** — Generate random `(energy, base_movement_cost, reference_energy)` tuples within valid ranges. Construct a minimal actor and config, run the movement logic, verify `energy_after == energy_before - expected_cost`.

**Property 2** — Generate random invalid config values (reference_energy in `(-1000.0, 0.0]`, base_movement_cost in `(-1000.0, 0.0)`). Attempt to validate, assert error.

**Property 3** — Generate random valid `ActorConfig` values. Serialize to TOML string, parse back, assert `base_movement_cost` and `reference_energy` match.

**Property 4** — Generate random actors with energy levels near the deduction threshold (energy close to the expected cost). Run movement, verify inert flag is set when energy drops to zero or below.

**Property 5** — Generate random inert actors with movement targets. Run movement, verify cell_index and energy are unchanged.

### Unit Tests

Unit tests cover specific examples and edge cases not worth randomizing:

- Actor at exactly `reference_energy` pays exactly `base_movement_cost`
- Actor at zero energy pays the floor cost and becomes inert
- Old `movement_cost` field in TOML is rejected as unknown key
- `format_config_info` output contains `base_movement_cost` and `reference_energy` strings
- Default `ActorConfig` has correct default values for new fields
