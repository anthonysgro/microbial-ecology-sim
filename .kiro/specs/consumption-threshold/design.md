# Design Document: Consumption Threshold

## Overview

This feature adds a `consumption_threshold` field to `ActorConfig` — a minimum chemical concentration below which actors refuse to consume. The change touches two WARM-path systems (`run_actor_metabolism`, `run_actor_sensing`), the config struct, validation logic, and documentation artifacts.

The implementation is minimal: a single `f32` field, two conditional checks in existing system functions, one validation clause, and documentation updates. No new structs, no new systems, no architectural changes.

### Design Rationale

Biologically, organisms require a minimum resource density to justify extraction. Below that density, the metabolic cost of harvesting exceeds the energy yield. By making the sensing system also respect the threshold, actors perceive sub-threshold cells as empty and compute gradients accordingly — they won't chase resources they can't extract.

Default of `0.0` preserves backward compatibility: existing configs produce identical simulation output.

## Architecture

No architectural changes. The feature threads a single new config value through two existing system functions.

```
┌─────────────┐
│ ActorConfig  │  + consumption_threshold: f32 (default 0.0)
└──────┬──────┘
       │ read by
       ├──────────────────────────┐
       ▼                          ▼
┌──────────────────┐   ┌───────────────────────┐
│ run_actor_sensing│   │ run_actor_metabolism   │
│ (WARM path)      │   │ (WARM path)           │
│                  │   │                       │
│ Treat cells with │   │ Skip consumption when │
│ conc < threshold │   │ conc < threshold;     │
│ as 0.0 for       │   │ apply only basal_decay│
│ gradient calc    │   │                       │
└──────────────────┘   └───────────────────────┘
```

### Tick Phase Impact

| Phase | Change |
|---|---|
| 1. Input Collection | None |
| 2. Precomputation (Sensing) | `run_actor_sensing` reads `consumption_threshold` from config; clamps sub-threshold concentrations to 0.0 before gradient computation |
| 3. Metabolism | `run_actor_metabolism` checks concentration against threshold before computing `consumed` |
| 4. Merge / Movement | None |
| 5. Post-Tick Validation | None |

## Components and Interfaces

### Modified: `ActorConfig` (src/grid/actor_config.rs)

Add one field:

```rust
/// Minimum chemical concentration (species 0) below which an Actor
/// treats the cell as empty. Must be >= 0.0 and finite.
pub consumption_threshold: f32,
```

Default: `0.0`. Added to `Default` impl.

### Modified: `run_actor_sensing` (src/grid/actor_systems.rs)

New signature adds `config: &ActorConfig` parameter:

```rust
pub fn run_actor_sensing(
    actors: &ActorRegistry,
    chemical_read: &[f32],
    grid_width: u32,
    grid_height: u32,
    movement_targets: &mut [Option<usize>],
    config: &ActorConfig,  // NEW
)
```

Behavior change: before computing gradients, apply effective concentration:

```rust
let threshold = config.consumption_threshold;
let effective = |conc: f32| -> f32 {
    if conc < threshold { 0.0 } else { conc }
};
let current_val = effective(chemical_read[ci]);
// ... neighbor gradients use effective(chemical_read[ni]) ...
```

This is a branch per neighbor read in a WARM path (actor count, not cell count), which is acceptable. No heap allocation, no dynamic dispatch.

### Modified: `run_actor_metabolism` (src/grid/actor_systems.rs)

No signature change — `config: &ActorConfig` is already passed.

Behavior change: before the consumption calculation for active actors, check threshold:

```rust
let available = chemical_read[ci];
if available < config.consumption_threshold {
    // Sub-threshold: no consumption, only basal decay.
    actor.energy -= config.base_energy_decay;
    // ... NaN/Inf check, inert transition ...
    continue;
}
// ... existing consumption logic unchanged ...
```

### Modified: `validate_world_config` (src/io/config_file.rs)

Add validation clause after existing actor config checks:

```rust
if actor.consumption_threshold < 0.0 || !actor.consumption_threshold.is_finite() {
    return Err(ConfigError::Validation {
        reason: format!(
            "consumption_threshold ({}) must be >= 0.0 and finite",
            actor.consumption_threshold,
        ),
    });
}
```

### Modified: `run_actor_phases` (src/grid/tick.rs)

Pass `&actor_config` to `run_actor_sensing`:

```rust
run_actor_sensing(
    &actors,
    chemical_read,
    grid.width(),
    grid.height(),
    &mut movement_targets,
    &actor_config,  // NEW
);
```

### Modified: `format_config_info` (src/viz_bevy/setup.rs)

Add line in the Actors section:

```rust
writeln!(out, "consumption_threshold: {:.4}", ac.consumption_threshold).ok();
```

## Data Models

### ActorConfig (updated)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActorConfig {
    pub consumption_rate: f32,
    pub energy_conversion_factor: f32,
    pub base_energy_decay: f32,
    pub initial_energy: f32,
    pub max_energy: f32,
    pub initial_actor_capacity: usize,
    pub movement_cost: f32,
    pub removal_threshold: f32,
    pub consumption_threshold: f32,  // NEW — default 0.0
}
```

No new structs. No schema changes to Actor, ActorRegistry, or any grid types.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: TOML consumption_threshold round-trip

*For any* finite non-negative `f32` value, serializing an `ActorConfig` with that `consumption_threshold` to TOML and deserializing the result SHALL produce an `ActorConfig` with the same `consumption_threshold` value.

**Validates: Requirements 1.3**

### Property 2: Validation accepts iff non-negative and finite

*For any* `f32` value assigned to `consumption_threshold`, `validate_world_config` SHALL accept the configuration if and only if the value is >= 0.0 and finite. Negative, NaN, and infinite values SHALL be rejected.

**Validates: Requirements 2.1, 2.2, 2.3**

### Property 3: Sub-threshold concentration skips consumption

*For any* active Actor on a cell where `chemical_concentration < consumption_threshold`, running `run_actor_metabolism` SHALL leave the chemical write buffer unchanged for that cell and SHALL decrease the Actor's energy by exactly `base_energy_decay`.

**Validates: Requirements 3.1**

### Property 4: At-or-above-threshold concentration permits consumption

*For any* active Actor on a cell where `chemical_concentration >= consumption_threshold` and `chemical_concentration > 0.0`, running `run_actor_metabolism` SHALL decrease the chemical write buffer for that cell by the consumed amount and SHALL update the Actor's energy by `consumed * energy_conversion_factor - base_energy_decay`.

**Validates: Requirements 3.2**

### Property 5: Sensing treats sub-threshold cells as zero

*For any* grid configuration, actor position, and `consumption_threshold > 0.0`, `run_actor_sensing` SHALL produce the same movement target as if all cells with concentration below the threshold had concentration 0.0.

**Validates: Requirements 4.1, 4.2, 4.3**

## Error Handling

No new error types. The feature reuses existing error infrastructure:

| Condition | Error Type | Source |
|---|---|---|
| `consumption_threshold < 0.0` or non-finite | `ConfigError::Validation` | `validate_world_config` |
| NaN/Inf energy after sub-threshold basal decay | `TickError::NumericalError` | `run_actor_metabolism` (existing check) |

The sub-threshold early-continue path in metabolism must include the same NaN/Inf energy check and inert transition that the existing code paths use. This is not new error handling — it reuses the pattern already present for inert actors.

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate (already idiomatic for Rust simulation projects). Each property test runs a minimum of 256 iterations (configurable via `proptest::test_runner::Config`).

Each property maps 1:1 to a property-based test:

| Property | Test Strategy | Generator |
|---|---|---|
| 1: TOML round-trip | Generate random non-negative finite f32, build ActorConfig, serialize to TOML, deserialize, compare field | `0.0..1000.0f32` |
| 2: Validation | Generate arbitrary f32 (including NaN, Inf, negatives), build WorldConfig with that threshold, run validate, assert result matches predicate | `prop::num::f32::ANY` |
| 3: Sub-threshold skip | Generate ActorConfig with threshold in `(0.01..10.0)`, concentration in `(0.0..threshold)`, run metabolism, assert no chemical change and energy delta == -base_energy_decay | Composite strategy |
| 4: Above-threshold consumption | Generate ActorConfig with threshold in `(0.0..5.0)`, concentration in `(threshold..threshold+10.0)`, run metabolism, assert chemical decreased and energy updated correctly | Composite strategy |
| 5: Sensing effective concentration | Generate small grid (3×3 to 5×5), random concentrations, random threshold, run sensing, compare against reference run with clamped concentrations | Composite strategy |

Tag format: `// Feature: consumption-threshold, Property N: <title>`

### Unit Tests

Unit tests complement property tests for specific examples and edge cases:

- Threshold exactly 0.0 backward compatibility (metabolism produces same result as before)
- Threshold exactly equal to concentration (boundary: should consume)
- Inert actors unaffected by threshold (they already skip consumption)
- `format_config_info` output contains "consumption_threshold" when actor config is present
- TOML deserialization with omitted field yields default 0.0
- Validation rejects NaN, +Inf, -Inf specifically (edge cases from Property 2)
