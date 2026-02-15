# Design Document: Proportional Mutation with Heritable Mutation Rate

## Overview

Two coupled changes to the mutation system:

1. **Proportional mutation**: Replace additive noise (`trait + Normal(0, σ)`) with multiplicative noise (`trait * (1 + Normal(0, σ))`). This makes mutation scale-invariant — a stddev of `0.05` means "5% of current value" regardless of trait magnitude. Fixes the `max_tumble_steps` dead-mutation bug where additive noise of `±0.05` on a `u16` value of `20` rounds back to `20` every time.

2. **Heritable mutation rate**: Promote the global `mutation_stddev` config value to a per-actor heritable trait (`mutation_rate: f32`) in `HeritableTraits`. Each actor's `mutation_rate` determines the stddev used for proportional mutation of all eight traits — including `mutation_rate` itself. This enables evolvability selection: lineages evolve their own mutation pressure, with natural selection finding the optimal rate for the current environment.

The global `config.mutation_stddev` field remains as the seed genome default (same pattern as `reproduction_cost`, `offspring_energy`, etc.).

## Architecture

No architectural changes. The existing data flow is preserved with one modification — the noise source shifts from global config to per-actor trait:

```
ActorConfig → HeritableTraits::from_config() → Actor.traits
                                                    ↓
                                        Binary Fission (parent.traits)
                                                    ↓
                              parent.traits.mutation_rate → σ for Normal(0, σ)
                                                    ↓
                              trait * (1.0 + Normal(0, σ)) → clamp → offspring.traits
```

The `mutate()` method no longer reads `config.mutation_stddev` for noise magnitude. It reads `self.mutation_rate`. The `config` parameter is retained solely for clamp bound values.

Visualization pipeline grows from 7 to 8 traits:

```
compute_trait_stats_from_actors: 7 trait buffers → 8 trait buffers
TraitStats.traits: [SingleTraitStats; 7] → [SingleTraitStats; 8]
TRAIT_NAMES: 7 entries → 8 entries
format_trait_stats / format_actor_info: display 8 traits
format_config_info: display 2 new clamp bound fields
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
    pub max_tumble_steps: u16,
    pub reproduction_cost: f32,
    pub offspring_energy: f32,
    pub mutation_rate: f32,        // NEW — 8th heritable trait
}
```

Field ordering: `mutation_rate` is appended after `offspring_energy`. The struct grows from 28 bytes to 32 bytes (7×f32 + 1×u16 + 2 bytes padding = 32). The compile-time size assert updates to `== 32`.

### HeritableTraits::from_config (modified)

```rust
pub fn from_config(config: &ActorConfig) -> Self {
    Self {
        // ... existing 7 fields unchanged ...
        mutation_rate: config.mutation_stddev,  // NEW
    }
}
```

### HeritableTraits::mutate (rewritten)

The core change. Two modifications:
1. Noise source: `self.mutation_rate` instead of `config.mutation_stddev`
2. Noise model: proportional (`trait * (1 + noise)`) instead of additive (`trait + noise`)

```rust
pub fn mutate(&mut self, config: &ActorConfig, rng: &mut impl Rng) {
    if self.mutation_rate == 0.0 {
        return;
    }

    let normal = Normal::new(0.0_f64, self.mutation_rate as f64)
        .expect("mutation_rate validated non-negative at config load");

    // Proportional mutation for all f32 traits:
    // new_value = old_value * (1.0 + Normal(0, mutation_rate))
    self.consumption_rate = (self.consumption_rate * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_consumption_rate_min, config.trait_consumption_rate_max);

    self.base_energy_decay = (self.base_energy_decay * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_base_energy_decay_min, config.trait_base_energy_decay_max);

    self.levy_exponent = (self.levy_exponent * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_levy_exponent_min, config.trait_levy_exponent_max);

    self.reproduction_threshold = (self.reproduction_threshold * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_reproduction_threshold_min, config.trait_reproduction_threshold_max);

    // max_tumble_steps: proportional in f32 space, round, clamp to u16 range.
    let tumble_f32 = self.max_tumble_steps as f32 * (1.0 + normal.sample(rng) as f32);
    self.max_tumble_steps = tumble_f32
        .round()
        .clamp(config.trait_max_tumble_steps_min as f32, config.trait_max_tumble_steps_max as f32)
        as u16;

    self.reproduction_cost = (self.reproduction_cost * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_reproduction_cost_min, config.trait_reproduction_cost_max);

    self.offspring_energy = (self.offspring_energy * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_offspring_energy_min, config.trait_offspring_energy_max);

    // mutation_rate mutates itself — self-referential proportional mutation.
    self.mutation_rate = (self.mutation_rate * (1.0 + normal.sample(rng) as f32))
        .clamp(config.trait_mutation_rate_min, config.trait_mutation_rate_max);
}
```

Key detail: `mutation_rate` is mutated last, using the pre-mutation `self.mutation_rate` as the stddev (since the `Normal` distribution was constructed at the top of the function from the original value). This is intentional — the parent's mutation rate determines the offspring's mutation intensity for all traits including the rate itself.

### ActorConfig (modified)

Two new fields with serde defaults:

```rust
fn default_trait_mutation_rate_min() -> f32 { 0.001 }
fn default_trait_mutation_rate_max() -> f32 { 0.5 }

// In ActorConfig:
#[serde(default = "default_trait_mutation_rate_min")]
pub trait_mutation_rate_min: f32,
#[serde(default = "default_trait_mutation_rate_max")]
pub trait_mutation_rate_max: f32,
```

The existing `mutation_stddev` field is unchanged in type and serde behavior. Its semantic role shifts from "global noise stddev" to "seed genome default for the `mutation_rate` trait."

### Config Validation (modified)

New validation rules appended to `validate_world_config`:

```rust
// trait_mutation_rate clamp range
if actor.trait_mutation_rate_min <= 0.0 { ... }
if actor.trait_mutation_rate_min >= actor.trait_mutation_rate_max { ... }

// mutation_stddev (seed default) must be within clamp range
if actor.mutation_stddev < actor.trait_mutation_rate_min
    || actor.mutation_stddev > actor.trait_mutation_rate_max { ... }
```

The existing `mutation_stddev >= 0.0` check is replaced by the range check above (since `trait_mutation_rate_min > 0.0` implies `mutation_stddev > 0.0` when within range). However, if the user sets `mutation_stddev = 0.0` and `trait_mutation_rate_min = 0.001`, validation will reject it. This is correct — a zero mutation rate is no longer valid as a seed default when the clamp minimum is positive. If the user wants to disable mutation entirely, they should set `trait_mutation_rate_min = 0.0` and `mutation_stddev = 0.0`.

Wait — Requirement 1.3 says zero mutation_rate should be identity. And `trait_mutation_rate_min` defaults to `0.001`, which means actors can't evolve to zero. This is intentional: the clamp prevents mutation rate from collapsing to zero through drift, which would permanently freeze a lineage. If the user explicitly wants to allow zero mutation rate, they set `trait_mutation_rate_min = 0.0`.

For the validation of `mutation_stddev >= 0.0` (the existing check 3g): we keep it but also add the range check. If `trait_mutation_rate_min > 0.0`, the range check subsumes the non-negative check. If the user sets `trait_mutation_rate_min = 0.0`, the non-negative check still catches `mutation_stddev = -1.0`.

### Visualization (modified)

- `TraitStats.traits`: `Option<[SingleTraitStats; 7]>` → `Option<[SingleTraitStats; 8]>`
- `compute_trait_stats_from_actors`: 7 buffers → 8 buffers, collecting `mutation_rate`
- `TRAIT_NAMES`: append `"mutation_rate"` (8 entries)
- `format_actor_info`: append `mutation_rate` line
- `format_trait_stats`: automatically handles 8 entries via `TRAIT_NAMES` iteration
- `format_config_info`: append `trait_mutation_rate: {min}..{max}` line

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
28      4     mutation_rate: f32
─────────────
Total: 32 bytes
```

The struct size assert changes from `== 28` to `== 32`.

### TraitStats Array

```rust
pub traits: Option<[SingleTraitStats; 8]>,
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
| 7 | mutation_rate |

## Correctness Properties

### Property 1: Proportional mutation produces scale-dependent noise

*For any* valid `HeritableTraits` with `mutation_rate > 0.0` and valid `ActorConfig`, after calling `mutate`, the expected absolute deviation of each f32 trait SHALL be proportional to the trait's pre-mutation value. Specifically, for two `HeritableTraits` instances differing only in one trait's magnitude (one at value `V`, one at value `2V`), the mean absolute deviation after many mutations SHALL be approximately twice as large for the `2V` instance.

**Validates: Requirements 1.1, 3.2**

### Property 2: All eight traits clamped to configured bounds after mutation

*For any* valid `HeritableTraits` (with all fields within their respective clamp ranges) and valid `ActorConfig` with `mutation_rate > 0.0`, after calling `mutate`, all eight trait fields SHALL be within their respective clamp ranges: the seven existing traits in their established ranges, and `mutation_rate` in `[trait_mutation_rate_min, trait_mutation_rate_max]`.

**Validates: Requirements 2.1, 2.2, 5.4**

### Property 3: Zero mutation_rate is identity

*For any* valid `HeritableTraits` with `mutation_rate == 0.0` and valid `ActorConfig`, calling `mutate` SHALL leave all eight trait fields unchanged (bitwise equal to the pre-mutation values).

**Validates: Requirements 1.3, 7.3**

### Property 4: from_config initializes mutation_rate from mutation_stddev

*For any* valid `ActorConfig`, calling `HeritableTraits::from_config` SHALL produce a `HeritableTraits` where `mutation_rate == config.mutation_stddev` (in addition to the existing seven fields matching their config counterparts).

**Validates: Requirements 5.2**

### Property 5: Deterministic mutation

*For any* valid `HeritableTraits` and valid `ActorConfig`, calling `mutate` with the same RNG seed SHALL produce identical post-mutation trait values across repeated invocations.

**Validates: Requirements 4.1**

### Property 6: mutate uses per-actor mutation_rate, not config.mutation_stddev

*For any* valid `ActorConfig` and two `HeritableTraits` instances identical except for `mutation_rate` (one with `mutation_rate = 0.01`, one with `mutation_rate = 0.2`), calling `mutate` with the same RNG seed SHALL produce different post-mutation values for the other seven traits, demonstrating that the per-actor rate — not the config value — controls noise magnitude.

**Validates: Requirements 5.3, 7.1**

### Property 7: Validation rejects invalid mutation rate clamp configurations

*For any* `ActorConfig` that violates exactly one of: `trait_mutation_rate_min <= 0.0`, `trait_mutation_rate_min >= trait_mutation_rate_max`, or `mutation_stddev` outside `[trait_mutation_rate_min, trait_mutation_rate_max]`, `validate_world_config` SHALL return `Err`.

**Validates: Requirements 6.2, 6.3, 6.4**

### Property 8: Trait stats computation covers all eight traits

*For any* non-empty set of non-inert actors, `compute_trait_stats_from_actors` SHALL return a `TraitStats` with `traits == Some(array)` where `array.len() == 8`, and the statistics at index 7 (mutation_rate) SHALL have `min <= mean <= max` and `min <= p25 <= p50 <= p75 <= max`.

**Validates: Requirements 8.1, 8.2**

### Property 9: Formatting includes all eight trait names and values

*For any* `TraitStats` with `Some` traits, `format_trait_stats` SHALL produce a string containing all eight trait names including `"mutation_rate"`. *For any* non-inert `Actor`, `format_actor_info` SHALL produce a string containing the actor's `mutation_rate` value.

**Validates: Requirements 8.4, 8.5**

## Error Handling

### Mutation Numerical Safety

Proportional mutation computes `trait * (1.0 + noise)`. If `trait` is `0.0`, the result is `0.0 * (1.0 + noise) = 0.0` — the trait is frozen at zero. This is acceptable for most traits because their clamp minimums are positive (e.g., `trait_consumption_rate_min = 0.1`), so the clamp will push them back up. For `mutation_rate`, the default `trait_mutation_rate_min = 0.001` prevents zero-trapping.

The `Normal::new(0.0, mutation_rate)` call cannot produce NaN. The `.clamp()` call handles edge-case infinities. For `max_tumble_steps`, the `as u16` cast after `.round().clamp(min, max)` is safe because the clamp bounds are valid `u16` values.

### Config Validation

The existing `mutation_stddev >= 0.0` check is retained. The new range check (`mutation_stddev` within `[trait_mutation_rate_min, trait_mutation_rate_max]`) is added after the clamp range validation. If `trait_mutation_rate_min = 0.0` is set by the user, `mutation_stddev = 0.0` is valid (disables mutation for the seed genome, but offspring could evolve non-zero rates if the clamp max allows it — though in practice they can't since zero rate means no mutation occurs).

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate. Each property test runs a minimum of 100 iterations.

Generators needed:
- `arb_valid_actor_config()`: Generates `ActorConfig` instances satisfying all validation constraints, including the two new `trait_mutation_rate_min/max` fields. Must ensure `mutation_stddev` is within `[trait_mutation_rate_min, trait_mutation_rate_max]`.
- `arb_heritable_traits(config)`: Generates `HeritableTraits` instances with all eight fields within their respective clamp ranges from the given config, including `mutation_rate` in `[trait_mutation_rate_min, trait_mutation_rate_max]`.
- `arb_invalid_mutation_rate_config()`: Generates `ActorConfig` instances that violate exactly one of the new mutation rate clamp constraints.

Each property test must be tagged with a comment referencing the design property:
```rust
// Feature: proportional-mutation, Property N: <property_text>
```

### Unit Testing

Unit tests complement property tests for specific examples and edge cases:
- `from_config` with default `ActorConfig` produces `mutation_rate == 0.05` (example for Req 5.2).
- `max_tumble_steps` at value `20` with `mutation_rate = 0.05` produces values ≠ 20 in at least 1 of 100 mutations (example for Req 3.1).
- `format_config_info` output contains `trait_mutation_rate` (example for Req 8.6).
- `format_actor_info` output contains `mutation_rate` (example for Req 8.4).
- Validation rejects `trait_mutation_rate_min = 0.0` with default config (edge case for Req 6.2 — default min is 0.001, but if user sets 0.0 it should be rejected since the constraint is `> 0.0`).
- Validation rejects `mutation_stddev = 0.0` when `trait_mutation_rate_min = 0.001` (edge case for Req 6.4).

### Test Organization

- Property tests for `HeritableTraits` (Properties 1–6): in `src/grid/actor.rs` test module.
- Property tests for config validation (Property 7): in `src/io/config_file.rs` test module.
- Property tests for trait stats (Property 8): in `src/viz_bevy/systems.rs` test module.
- Property tests for formatting (Property 9): in `src/viz_bevy/setup.rs` test module.
