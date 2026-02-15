# Requirements Document

## Introduction

The `HeritableTraits::mutate()` method currently applies additive gaussian noise (`trait + Normal(0, σ)`) to all seven heritable traits during binary fission. This model is scale-dependent: a `mutation_stddev` of `0.05` produces meaningful variation for small `f32` traits (e.g., `base_energy_decay = 0.05`) but is negligible for large-magnitude traits (e.g., `max_tumble_steps = 20` as `u16`, where rounding eliminates the noise entirely).

This spec makes two changes:

1. **Proportional mutation**: Switch all trait mutations to a multiplicative model: `trait * (1 + Normal(0, σ))`. Under this model, `mutation_stddev` represents a fraction of the current trait value, making noise scale-invariant across all traits regardless of magnitude or type.

2. **Heritable mutation rate**: Promote `mutation_stddev` from a global `ActorConfig` value to a per-actor heritable trait (`mutation_rate`). Each actor carries its own mutation rate, which is itself subject to proportional mutation during fission. This enables evolvability selection — lineages can evolve to be more or less mutationally volatile, with natural selection determining the optimal mutation pressure for the current environment.

## Glossary

- **Mutate_Method**: The `HeritableTraits::mutate()` function in `src/grid/actor.rs` that applies gaussian noise to heritable traits during binary fission.
- **Additive_Mutation**: The current model where noise is added directly: `trait_value + Normal(0, σ)`.
- **Proportional_Mutation**: The replacement model where noise scales with the trait value: `trait_value * (1.0 + Normal(0, σ))`.
- **mutation_rate**: The new per-actor heritable trait (f32) that replaces the global `mutation_stddev` as the standard deviation for proportional mutation. Stored in `HeritableTraits`.
- **mutation_stddev**: The existing `ActorConfig` field. Retains its role as the seed genome default for `mutation_rate` and is used to initialize the trait via `from_config`.
- **HeritableTraits**: The struct containing per-actor heritable fields. Currently seven fields; this spec adds an eighth (`mutation_rate`).
- **Clamp_Bounds**: The per-trait `[min, max]` range configured in `ActorConfig` that constrains post-mutation values.

## Requirements

### Requirement 1: Proportional Mutation Formula

**User Story:** As a simulation engineer, I want trait mutations to scale proportionally with trait magnitude, so that all heritable traits experience meaningful evolutionary variation regardless of their numeric range.

#### Acceptance Criteria

1. WHEN the Mutate_Method applies noise to an f32 trait, THE Mutate_Method SHALL compute the mutated value as `trait_value * (1.0 + Normal(0, actor_mutation_rate))` instead of `trait_value + Normal(0, mutation_stddev)`, where `actor_mutation_rate` is the actor's per-actor `mutation_rate` trait.
2. WHEN the Mutate_Method applies noise to `max_tumble_steps`, THE Mutate_Method SHALL compute the mutated value as `(max_tumble_steps as f32 * (1.0 + Normal(0, actor_mutation_rate))).round()` and cast the result to `u16`.
3. WHEN the actor's `mutation_rate` is `0.0`, THE Mutate_Method SHALL leave all trait fields unchanged (the multiplicative factor `1.0 + 0.0` produces identity, matching the existing early-return behavior).

### Requirement 2: Post-Mutation Clamping Preservation

**User Story:** As a simulation engineer, I want mutated traits to remain within configured bounds after the formula change, so that the simulation maintains valid actor state.

#### Acceptance Criteria

1. WHEN the Mutate_Method computes a proportionally mutated value for any f32 trait, THE Mutate_Method SHALL clamp the result to the trait's configured `[min, max]` Clamp_Bounds.
2. WHEN the Mutate_Method computes a proportionally mutated value for `max_tumble_steps`, THE Mutate_Method SHALL clamp the rounded result to `[trait_max_tumble_steps_min, trait_max_tumble_steps_max]` before casting to `u16`.

### Requirement 3: Scale-Invariant Variation for max_tumble_steps

**User Story:** As a simulation engineer, I want `max_tumble_steps` to exhibit observable mutation variation under typical `mutation_rate` values, so that this trait participates in evolutionary dynamics like all other heritable traits.

#### Acceptance Criteria

1. WHEN `mutation_rate` is `0.05` and `max_tumble_steps` is `20`, THE Mutate_Method SHALL produce a noise magnitude of approximately `1.0` (5% of 20), which after rounding yields values different from the original with non-negligible probability.
2. WHEN `mutation_rate` is greater than `0.0` and the trait value is greater than `0.0`, THE Mutate_Method SHALL produce noise whose magnitude scales with the trait value for all heritable traits.

### Requirement 4: Determinism Preservation

**User Story:** As a simulation engineer, I want the proportional mutation model to remain fully deterministic given the same seed, so that simulation replay and debugging are unaffected.

#### Acceptance Criteria

1. WHEN the same `ActorConfig`, `HeritableTraits` state, and RNG seed are provided, THE Mutate_Method SHALL produce identical post-mutation trait values across repeated invocations.

### Requirement 5: Heritable Mutation Rate Trait

**User Story:** As a simulation engineer, I want each actor to carry its own mutation rate as a heritable trait, so that evolvability itself is subject to natural selection — lineages can evolve to be more or less mutationally volatile.

#### Acceptance Criteria

1. THE `HeritableTraits` struct SHALL include a new `mutation_rate: f32` field, bringing the total heritable trait count from 7 to 8.
2. THE `HeritableTraits::from_config` method SHALL initialize `mutation_rate` from `config.mutation_stddev`.
3. THE Mutate_Method SHALL use the actor's own `mutation_rate` trait value as the standard deviation for proportional mutation of all eight traits (including `mutation_rate` itself).
4. THE Mutate_Method SHALL apply proportional mutation to `mutation_rate` itself: `mutation_rate * (1.0 + Normal(0, mutation_rate))`, clamped to `[trait_mutation_rate_min, trait_mutation_rate_max]`.
5. THE compile-time size assert for `HeritableTraits` SHALL be updated to reflect the new struct size (32 bytes: 7×f32 + 1×u16 + 2 bytes padding + 1×f32 for mutation_rate, or as determined by actual layout).

### Requirement 6: Mutation Rate Config and Validation

**User Story:** As a simulation engineer, I want configurable clamp bounds for the heritable mutation rate, so that I can prevent degenerate mutation dynamics while allowing evolution to find the optimal rate.

#### Acceptance Criteria

1. THE `ActorConfig` struct SHALL include two new fields: `trait_mutation_rate_min: f32` (default: `0.001`) and `trait_mutation_rate_max: f32` (default: `0.5`).
2. THE config validation SHALL verify `trait_mutation_rate_min > 0.0`.
3. THE config validation SHALL verify `trait_mutation_rate_min < trait_mutation_rate_max`.
4. THE config validation SHALL verify `mutation_stddev` (the seed default) is within `[trait_mutation_rate_min, trait_mutation_rate_max]`.
5. THE `mutation_stddev` field SHALL remain in `ActorConfig` as the seed genome default for the `mutation_rate` trait. Its semantics change from "global stddev" to "initial per-actor mutation rate".

### Requirement 7: Mutate Method Signature Change

**User Story:** As a simulation engineer, I want the mutate method to use the actor's own mutation rate instead of reading from config, so that the per-actor trait is the single source of truth for mutation intensity.

#### Acceptance Criteria

1. THE Mutate_Method SHALL no longer read `config.mutation_stddev` to determine noise magnitude. It SHALL use `self.mutation_rate` instead.
2. THE Mutate_Method SHALL still accept `config: &ActorConfig` for clamp bound values only.
3. THE early-return guard SHALL check `self.mutation_rate == 0.0` instead of `config.mutation_stddev == 0.0`.

### Requirement 8: Visualization Updates

**User Story:** As a simulation engineer, I want the trait visualization to include the new mutation_rate trait, so that I can observe how mutation rates evolve across the population.

#### Acceptance Criteria

1. THE `TraitStats.traits` array SHALL grow from `[SingleTraitStats; 7]` to `[SingleTraitStats; 8]`.
2. THE `compute_trait_stats_from_actors` function SHALL collect `mutation_rate` values and compute statistics at array index 7.
3. THE `TRAIT_NAMES` array SHALL include `"mutation_rate"` as the 8th entry.
4. THE `format_actor_info` function SHALL display the actor's `mutation_rate` value.
5. THE `format_trait_stats` function SHALL display population statistics for `mutation_rate`.
6. THE `format_config_info` function SHALL display `trait_mutation_rate_min` and `trait_mutation_rate_max`.

### Requirement 9: Documentation Updates

**User Story:** As a simulation engineer, I want all configuration documentation to reflect the new mutation model and heritable mutation rate, so that the docs stay in sync with the code.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include `trait_mutation_rate_min` and `trait_mutation_rate_max` fields with explanatory comments.
2. THE `example_config.toml` SHALL update the `mutation_stddev` comment to clarify it now serves as the seed genome default for the per-actor `mutation_rate` trait.
3. THE config-documentation steering file SHALL be updated to include the two new clamp bound fields and the updated semantics of `mutation_stddev`.

### Requirement 10: Existing Design Documentation Reference

**User Story:** As a simulation engineer, I want the new proportional mutation model and heritable mutation rate documented in this spec's design, so that the mutation semantics are clearly recorded.

#### Acceptance Criteria

1. THE design document for this spec SHALL document the proportional mutation formula, heritable mutation rate, and their rationale.
2. THE design document for this spec SHALL NOT modify the existing `new-heritable-traits` design document.
