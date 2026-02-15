# Design Document: New Heritable Traits

## Overview

This feature promotes three global `ActorConfig` values — `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` — into per-actor heritable traits stored in the `HeritableTraits` struct. The change follows the established pattern: traits are initialized from config defaults, inherited during binary fission, mutated with gaussian noise, and clamped to configurable bounds.

The core architectural change is small: `HeritableTraits` grows from 4 fields (16 bytes) to 7 fields, and three system functions switch from reading a global config value to reading from `actor.traits`. The ripple effects are in config validation, visualization stats, and documentation.

Key design constraint: `max_tumble_steps` is `u16` in the Actor but mutation operates in `f32` space. The mutation path converts to `f32`, adds noise, rounds, and clamps back to `u16`.

## Architecture

No architectural changes. The existing data flow is preserved:

```
ActorConfig → HeritableTraits::from_config() → Actor.traits
                                                    ↓
                                        Binary Fission (parent.traits)
                                                    ↓
                                        HeritableTraits::mutate() → offspring.traits
```

Systems read from `actor.traits` instead of `config`:

```
run_actor_sensing:       config.max_tumble_steps  → actor.traits.max_tumble_steps
run_actor_reproduction:  config.reproduction_cost  → actor.traits.reproduction_cost
                         config.offspring_energy   → actor.traits.offspring_energy
run_deferred_spawn:      config.offspring_energy   → spawn_buffer energy (from parent traits)
```

Visualization pipeline:

```
compute_trait_stats_from_actors: 4 trait buffers → 7 trait buffers
TraitStats.traits: [SingleTraitStats; 4] → [SingleTraitStats; 7]
TRAIT_NAMES: 4 entries → 7 entries
format_trait_stats / format_actor_info: display 7 traits
format_config_info: display 6 new clamp bound fields
```

## Components and Interfaces

### HeritableTraits (modified)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeritableTraits {
    pub consumption_rate: f32,
    pub base_energy_decay: f32,
    pub levy_exponent: f32,
    pub reproduction_threshold: f32,
    pub max_tumble_steps: u16,       // NEW
    pub reproduction_cost: f32,       // NEW
    pub offspring_energy: f32,        // NEW
}
```

Field ordering rationale: the four existing `f32` fields stay first for cache-line alignment. `max_tumble_steps` (`u16`) is placed before the two new `f32` fields. With padding, the struct grows from 16 bytes to 28 bytes (4×f32 + 1×u16 + 2 bytes padding + 2×f32). The compile-time size assert must be updated.

### HeritableTraits::from_config (modified)

Initializes the three new fields from `ActorConfig` defaults. No behavioral change to existing fields.

### HeritableTraits::mutate (modified)

Adds mutation logic for the three new fields:

```rust
// max_tumble_steps: mutate in f32 space, round, clamp to u16 range
let tumble_f32 = self.max_tumble_steps as f32 + normal.sample(rng) as f32;
self.max_tumble_steps = tumble_f32
    .round()
    .clamp(config.trait_max_tumble_steps_min as f32, config.trait_max_tumble_steps_max as f32)
    as u16;

// reproduction_cost: standard f32 mutation + clamp
self.reproduction_cost = (self.reproduction_cost + normal.sample(rng) as f32)
    .clamp(config.trait_reproduction_cost_min, config.trait_reproduction_cost_max);

// offspring_energy: standard f32 mutation + clamp
self.offspring_energy = (self.offspring_energy + normal.sample(rng) as f32)
    .clamp(config.trait_offspring_energy_min, config.trait_offspring_energy_max);
```

### ActorConfig (modified)

Six new fields with serde defaults:

```rust
pub trait_max_tumble_steps_min: u16,    // default: 1
pub trait_max_tumble_steps_max: u16,    // default: 50
pub trait_reproduction_cost_min: f32,   // default: 0.1
pub trait_reproduction_cost_max: f32,   // default: 100.0
pub trait_offspring_energy_min: f32,    // default: 0.1
pub trait_offspring_energy_max: f32,    // default: 100.0
```

### run_actor_sensing (modified)

Single change: `config.max_tumble_steps` → `actor.traits.max_tumble_steps` in the tumble initiation branch.

### run_actor_reproduction (modified)

Two changes:
- `config.reproduction_cost` → `actor.traits.reproduction_cost` for parent energy deduction.
- `config.offspring_energy` → `actor.traits.offspring_energy` for spawn buffer energy value.

### run_deferred_spawn (unchanged logic)

The spawn buffer already carries the energy value from `run_actor_reproduction`. Since reproduction now writes `actor.traits.offspring_energy` into the buffer, spawn automatically uses the per-actor value. No code change needed in `run_deferred_spawn` itself.

### Config Validation (modified)

New validation rules in `validate_world_config`:
- `trait_max_tumble_steps_min >= 1`
- `trait_max_tumble_steps_min < trait_max_tumble_steps_max`
- `trait_reproduction_cost_min > 0.0`, `trait_reproduction_cost_min < trait_reproduction_cost_max`
- `trait_offspring_energy_min > 0.0`, `trait_offspring_energy_min < trait_offspring_energy_max`
- `trait_offspring_energy_max <= max_energy`
- Default values within clamp ranges for all three new traits

### Visualization (modified)

- `TraitStats.traits`: `Option<[SingleTraitStats; 4]>` → `Option<[SingleTraitStats; 7]>`
- `compute_trait_stats_from_actors`: 4 buffers → 7 buffers, collecting `max_tumble_steps as f32`, `reproduction_cost`, `offspring_energy`
- `TRAIT_NAMES`: append `"max_tumble_steps"`, `"reproduction_cost"`, `"offspring_energy"`
- `format_actor_info`: append three new trait lines
- `format_config_info`: append six new clamp bound lines

## Data Models

### HeritableTraits Memory Layout

```
Offset  Size  Field
0       4     consumption_rate: f32
4       4     base_energy_decay: f32
8       4     levy_exponent: f32
12      4     reproduction_threshold: f32
16      2     max_tumble_steps: u16
18      2     (padding)
20      4     reproduction_cost: f32
24      4     offspring_energy: f32
─────────────
Total: 28 bytes
```

The struct size assert changes from `== 16` to `== 28`.

### Spawn Buffer Tuple

Currently `Vec<(usize, f32, HeritableTraits)>`. No change to the tuple structure — the `f32` energy field now comes from `actor.traits.offspring_energy` instead of `config.offspring_energy`.

### TraitStats Array

```rust
pub traits: Option<[SingleTraitStats; 7]>,
```

Array index mapping:
| Index | Trait |
|-------|-------|
| 0 | consumption_rate |
| 1 | base_energy_decay |
| 2 | levy_exponent |
| 3 | reproduction_threshold |
| 4 | max_tumble_steps |
| 5 | reproduction_cost |
| 6 | offspring_energy |



## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: from_config initializes all seven traits from config defaults

*For any* valid `ActorConfig`, calling `HeritableTraits::from_config` SHALL produce a `HeritableTraits` where `max_tumble_steps == config.max_tumble_steps`, `reproduction_cost == config.reproduction_cost`, and `offspring_energy == config.offspring_energy` (in addition to the existing four fields matching their config counterparts).

**Validates: Requirements 1.2**

### Property 2: Mutation clamps all seven traits to configured bounds

*For any* valid `HeritableTraits` and valid `ActorConfig` with `mutation_stddev > 0.0`, after calling `mutate`, all seven trait fields SHALL be within their respective clamp ranges: `consumption_rate` in `[trait_consumption_rate_min, trait_consumption_rate_max]`, `base_energy_decay` in `[trait_base_energy_decay_min, trait_base_energy_decay_max]`, `levy_exponent` in `[trait_levy_exponent_min, trait_levy_exponent_max]`, `reproduction_threshold` in `[trait_reproduction_threshold_min, trait_reproduction_threshold_max]`, `max_tumble_steps` in `[trait_max_tumble_steps_min, trait_max_tumble_steps_max]`, `reproduction_cost` in `[trait_reproduction_cost_min, trait_reproduction_cost_max]`, and `offspring_energy` in `[trait_offspring_energy_min, trait_offspring_energy_max]`.

**Validates: Requirements 2.1, 2.2, 2.3, 2.4**

### Property 3: Zero-stddev mutation is identity

*For any* valid `HeritableTraits` and valid `ActorConfig` with `mutation_stddev == 0.0`, calling `mutate` SHALL leave all seven trait fields unchanged (bitwise equal to the pre-mutation values).

**Validates: Requirements 2.5**

### Property 4: Validation rejects invalid new trait clamp configurations

*For any* `ActorConfig` that violates exactly one of the new trait clamp constraints (e.g., `trait_max_tumble_steps_min < 1`, `trait_max_tumble_steps_min >= trait_max_tumble_steps_max`, `trait_reproduction_cost_min <= 0.0`, `trait_offspring_energy_max > max_energy`, or default values outside clamp ranges), `validate_world_config` SHALL return `Err`.

**Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11**

### Property 5: Sensing uses per-actor max_tumble_steps

*For any* actor with `max_tumble_steps = M` on a grid where no positive chemical gradient exists (forcing tumble initiation), the sampled tumble step count SHALL be in `[1, M]`, regardless of the value of `config.max_tumble_steps`.

**Validates: Requirements 4.1**

### Property 6: Reproduction uses per-actor reproduction_cost and offspring_energy

*For any* actor with energy above its `reproduction_threshold` and an available adjacent cell, after `run_actor_reproduction`, the parent's energy SHALL decrease by exactly `actor.traits.reproduction_cost`, and the spawn buffer entry SHALL contain `actor.traits.offspring_energy` as the offspring energy value, regardless of `config.reproduction_cost` and `config.offspring_energy`.

**Validates: Requirements 5.1, 6.1, 6.2**

### Property 7: Trait stats computation covers all seven traits

*For any* non-empty set of non-inert actors, `compute_trait_stats_from_actors` SHALL return a `TraitStats` with `traits == Some(array)` where `array.len() == 7`, and the statistics at index 4 (max_tumble_steps), index 5 (reproduction_cost), and index 6 (offspring_energy) SHALL have `min <= mean <= max` and `min <= p25 <= p50 <= p75 <= max`.

**Validates: Requirements 7.1, 7.5**

### Property 8: Formatting includes all seven trait names and values

*For any* `TraitStats` with `Some` traits, `format_trait_stats` SHALL produce a string containing all seven trait names. *For any* non-inert `Actor`, `format_actor_info` SHALL produce a string containing the actor's `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` values.

**Validates: Requirements 7.2, 7.3**

## Error Handling

### Mutation Numerical Safety

`HeritableTraits::mutate` operates in `f32` space. The gaussian noise sample comes from `Normal::new(0.0, stddev)` which cannot produce NaN. The `.clamp()` call handles any edge-case infinities by clamping to finite bounds. For `max_tumble_steps`, the `as u16` cast after `.round().clamp(min, max)` is safe because the clamp bounds are valid `u16` values.

### Config Validation Errors

All six new clamp bound fields are validated in `validate_world_config`. Invalid configurations produce `ConfigError::Validation` with a descriptive reason string. Validation order: new trait clamp ranges are checked after existing trait ranges, following the established pattern.

### System Function Errors

`run_actor_reproduction` already returns `Result<(), TickError>` with NaN/Inf checks on parent energy after deduction. No new error paths are introduced — the deduction source changes from `config.reproduction_cost` to `actor.traits.reproduction_cost`, but the NaN/Inf guard remains.

`run_actor_sensing` does not return `Result` — it cannot fail. The change from `config.max_tumble_steps` to `actor.traits.max_tumble_steps` introduces no new failure mode since both are `u16` values validated at config load time and clamped during mutation.

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate. Each property test runs a minimum of 100 iterations.

Generators needed:
- `arb_valid_actor_config()`: Generates `ActorConfig` instances satisfying all validation constraints, including the six new clamp bound fields.
- `arb_heritable_traits(config)`: Generates `HeritableTraits` instances with all seven fields within their respective clamp ranges from the given config.
- `arb_invalid_new_trait_config()`: Generates `ActorConfig` instances that violate exactly one of the new trait clamp constraints.

Each property test must be tagged with a comment referencing the design property:
```rust
// Feature: new-heritable-traits, Property N: <property_text>
```

### Unit Testing

Unit tests complement property tests for specific examples and edge cases:
- `from_config` with default `ActorConfig` produces expected field values (example for Req 3.1–3.3).
- `format_config_info` output contains the six new clamp bound field names (example for Req 7.4).
- Validation rejects `trait_max_tumble_steps_min = 0` (edge case for Req 3.4).
- Existing sensing/metabolism/reproduction tests updated to use the new `HeritableTraits` struct (7 fields instead of 4).

### Test Organization

- Property tests for `HeritableTraits` (Properties 1–3): in `src/grid/actor.rs` test module or a dedicated `tests/` file.
- Property tests for config validation (Property 4): in `src/io/config_file.rs` test module.
- Property tests for sensing (Property 5): in `src/grid/actor_systems.rs` test module.
- Property tests for reproduction (Property 6): in `src/grid/actor_systems.rs` test module.
- Property tests for trait stats (Property 7): in `src/viz_bevy/systems.rs` test module.
- Property tests for formatting (Property 8): in `src/viz_bevy/setup.rs` test module.
