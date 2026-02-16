# Requirements Document

## Introduction

The simulation currently allows actors to reproduce every tick as long as they meet the energy threshold, leading to runaway population growth. Two compounding problems exist:

1. **Uncapped reproduction frequency**: actors can fission every tick when energy is sufficient, saturating the grid faster than predation can cull.
2. **No r/K trade-off enforcement**: `reproduction_cost` and `offspring_energy` evolve independently with no metabolic coupling, allowing actors to simultaneously invest heavily per offspring AND reproduce rapidly — an evolutionarily unrealistic strategy.

This feature introduces two coupled mechanisms:

- **Reproduction cooldown**: a heritable trait specifying the minimum ticks between successive reproductions, creating a temporal gate on fission frequency.
- **Reproductive readiness cost**: a continuous per-tick metabolic drain that scales with how quickly an actor can reproduce and how much it invests per reproduction event. This models the biological cost of maintaining reproductive machinery in a ready state.

Together, these create a three-dimensional evolutionary trade-off: actors that reproduce frequently with high investment per offspring pay a steep ongoing metabolic cost, forcing genuine r/K strategy differentiation.

## Glossary

- **Actor**: The atomic biological entity in the simulation. An ECS entity with physical components (energy, position, heritable traits).
- **HeritableTraits**: A plain data struct on each Actor containing all traits subject to mutation during reproduction.
- **Reproduction_Cooldown**: A heritable trait specifying the minimum number of ticks an actor must wait after reproducing before it can reproduce again.
- **Cooldown_Timer**: A per-actor runtime counter tracking remaining ticks until the actor is eligible to reproduce again. Not heritable — it is transient state.
- **Reproductive_Readiness_Cost**: A continuous per-tick energy drain modeling the metabolic cost of maintaining reproductive machinery. Scales with reproductive investment and inversely with cooldown duration.
- **Reproductive_Investment**: The sum `reproduction_cost + offspring_energy` — the total energy an actor commits per fission event.
- **Reference_Cooldown**: A global configuration parameter defining the neutral cooldown at which the readiness cost multiplier equals 1.0. Actors with shorter cooldowns pay more; actors with longer cooldowns pay less.
- **ActorConfig**: The configuration struct controlling actor behavior defaults and trait clamp bounds.
- **Reproduction_System**: The `run_actor_reproduction` function that checks eligibility and queues offspring for deferred spawning.
- **Deferred_Spawn_System**: The `run_deferred_spawn` function that creates offspring actors from the spawn buffer.
- **Metabolism_System**: The `run_actor_metabolism` function that computes per-tick energy balance including consumption, basal decay, thermal penalty, and reproductive readiness cost.
- **Genetic_Distance**: The `genetic_distance` function computing normalized Euclidean distance between two actors' heritable trait vectors.
- **Stats_Panel**: The Bevy visualization panel displaying population-level trait statistics.
- **Actor_Inspector**: The Bevy visualization panel displaying individual actor trait values.
- **Config_Info_Panel**: The Bevy visualization panel displaying active configuration values.

## Requirements

### Requirement 1: Reproduction Cooldown Heritable Trait

**User Story:** As a simulation designer, I want reproduction cooldown to be a heritable trait on each actor, so that evolutionary pressure can shape reproductive timing strategies across the population.

#### Acceptance Criteria

1. THE HeritableTraits struct SHALL include a `reproduction_cooldown` field of type `u16` representing the minimum ticks between successive reproductions.
2. WHEN a new actor is created from configuration defaults, THE HeritableTraits SHALL initialize `reproduction_cooldown` from the `reproduction_cooldown` field in ActorConfig.
3. WHEN an offspring's traits are mutated during reproduction, THE Deferred_Spawn_System SHALL apply proportional mutation to `reproduction_cooldown` using the same mutation mechanism as `max_tumble_steps` (mutate in f32 space, round, clamp to u16 bounds).
4. THE ActorConfig SHALL include a `reproduction_cooldown` seed default field with a default value of `5`.
5. THE ActorConfig SHALL include `trait_reproduction_cooldown_min` (default `1`, type `u16`) and `trait_reproduction_cooldown_max` (default `100`, type `u16`) clamp bound fields.
6. WHEN `trait_reproduction_cooldown_min` equals `trait_reproduction_cooldown_max`, THE mutation system SHALL produce zero difference for the `reproduction_cooldown` dimension.

### Requirement 2: Cooldown Timer Runtime State

**User Story:** As a simulation designer, I want each actor to track its remaining cooldown ticks, so that the reproduction system can enforce the cooldown constraint.

#### Acceptance Criteria

1. THE Actor struct SHALL include a `cooldown_remaining` field of type `u16` initialized to `0` for newly spawned actors.
2. WHEN an actor successfully reproduces, THE Reproduction_System SHALL set the parent actor's `cooldown_remaining` to the parent's heritable `reproduction_cooldown` value.
3. WHEN `cooldown_remaining` is greater than zero, THE Reproduction_System SHALL skip the actor for reproduction eligibility and decrement `cooldown_remaining` by one.
4. WHEN `cooldown_remaining` reaches zero, THE Reproduction_System SHALL allow the actor to be evaluated for reproduction using the existing energy-based criteria.
5. THE Reproduction_System SHALL evaluate cooldown eligibility before energy threshold checks to avoid unnecessary computation.

### Requirement 3: Reproductive Readiness Metabolic Cost

**User Story:** As a simulation designer, I want actors to pay a continuous metabolic cost for maintaining reproductive readiness, so that fast-reproducing actors with high per-offspring investment face genuine evolutionary pressure.

#### Acceptance Criteria

1. THE Metabolism_System SHALL compute a per-tick reproductive readiness cost for each active (non-inert) actor using the formula: `readiness_cost = readiness_sensitivity * (reproduction_cost + offspring_energy) / max(reproduction_cooldown, 1) / reference_cooldown`.
2. THE readiness cost SHALL be subtracted from the actor's energy each tick alongside `base_energy_decay` and `thermal_cost`.
3. THE ActorConfig SHALL include a `readiness_sensitivity` field (type `f32`, default `0.01`) controlling the global strength of the readiness penalty. Must be `>= 0.0` and finite.
4. THE ActorConfig SHALL include a `reference_cooldown` field (type `f32`, default `5.0`) defining the neutral cooldown at which the readiness multiplier equals `1.0 / reference_cooldown`. Must be `> 0.0` and finite.
5. WHEN `readiness_sensitivity` equals `0.0`, THE readiness cost SHALL be exactly `0.0`, effectively disabling the mechanic.
6. INERT actors SHALL NOT incur any reproductive readiness cost.
7. THE existing NaN/Inf energy check in the Metabolism_System SHALL cover the readiness cost computation — no new error paths are introduced.

### Requirement 4: Genetic Distance Update

**User Story:** As a simulation designer, I want `reproduction_cooldown` included in genetic distance computation, so that kin recognition accounts for reproductive strategy divergence.

#### Acceptance Criteria

1. THE `genetic_distance` function SHALL include `reproduction_cooldown` in the normalized Euclidean distance computation using `trait_reproduction_cooldown_min` and `trait_reproduction_cooldown_max` as normalization bounds.
2. THE `TRAIT_COUNT` constant SHALL be updated from 10 to 11 to reflect the addition of `reproduction_cooldown`.

### Requirement 5: Configuration and Documentation

**User Story:** As a simulation operator, I want to configure reproduction cooldown defaults, bounds, and readiness cost parameters via TOML, so that I can tune reproductive dynamics without recompiling.

#### Acceptance Criteria

1. THE configuration parser SHALL accept `reproduction_cooldown`, `trait_reproduction_cooldown_min`, `trait_reproduction_cooldown_max`, `readiness_sensitivity`, and `reference_cooldown` fields under the `[actor]` TOML section.
2. WHEN `trait_reproduction_cooldown_max` is less than `trait_reproduction_cooldown_min`, THE configuration parser SHALL reject the configuration with a descriptive error.
3. WHEN `reproduction_cooldown` is outside the range `[trait_reproduction_cooldown_min, trait_reproduction_cooldown_max]`, THE configuration parser SHALL reject the configuration with a descriptive error.
4. WHEN `readiness_sensitivity` is negative or non-finite, THE configuration parser SHALL reject the configuration with a descriptive error.
5. WHEN `reference_cooldown` is not positive or non-finite, THE configuration parser SHALL reject the configuration with a descriptive error.
6. THE `example_config.toml` file SHALL include the five new fields with comments explaining their purpose and valid ranges.
7. THE `format_config_info` function in `src/viz_bevy/setup.rs` SHALL display the five new configuration fields in the config info panel.
8. THE `config-documentation.md` steering file SHALL be updated to document the five new fields in the ActorConfig reference table and add `reproduction_cooldown` to the heritable trait list.

### Requirement 6: Visualization and Stats Updates

**User Story:** As a simulation observer, I want to see reproduction cooldown statistics in the stats panel and individual cooldown values in the actor inspector, so that I can monitor how reproductive timing evolves across the population.

#### Acceptance Criteria

1. THE `TraitStats.traits` array SHALL grow from `[SingleTraitStats; 10]` to `[SingleTraitStats; 11]`.
2. THE `compute_trait_stats_from_actors` function SHALL collect `reproduction_cooldown` values and compute statistics at the new array index.
3. THE `TRAIT_NAMES` array SHALL include `"repro_cooldown"` as the 11th entry.
4. THE `format_actor_info` function SHALL display the actor's heritable `reproduction_cooldown` trait value and current `cooldown_remaining` timer.
