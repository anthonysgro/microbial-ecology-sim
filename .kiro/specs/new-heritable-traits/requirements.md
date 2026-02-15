# Requirements Document

## Introduction

Promote three currently-global actor configuration values — `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` — into per-actor heritable traits. These traits are inherited from parent to offspring during binary fission with gaussian mutation and clamping, matching the existing pattern for `consumption_rate`, `base_energy_decay`, `levy_exponent`, and `reproduction_threshold`. This enables r/K selection strategies and tumble-length variation to emerge from local actor-to-actor dynamics rather than being imposed globally.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, carrying energy reserves and heritable traits. Represented by the `Actor` struct.
- **HeritableTraits**: A plain data struct stored inline in each Actor, holding per-actor trait values that are inherited during fission with mutation.
- **ActorConfig**: The global configuration struct providing default trait values, mutation parameters, and clamp bounds for all heritable traits.
- **Binary_Fission**: The reproduction event where a parent Actor spawns an offspring Actor in an adjacent cell, deducting energy from the parent and assigning energy to the offspring.
- **Gaussian_Mutation**: The process of adding normally-distributed noise (mean 0, std-dev `mutation_stddev`) to each heritable trait value during fission, then clamping to configured bounds.
- **Trait_Clamp_Bounds**: Per-trait minimum and maximum values in ActorConfig that constrain heritable trait values after mutation.
- **Sensing_System**: The `run_actor_sensing` function that computes movement targets using chemical gradients and Lévy flight tumble state.
- **Reproduction_System**: The `run_actor_reproduction` function that checks reproduction eligibility and populates the spawn buffer.
- **Spawn_System**: The `run_deferred_spawn` function that inserts offspring Actors into the registry with mutated traits.
- **Trait_Stats**: Population-level statistics (min, max, mean, percentiles) computed per heritable trait for visualization.
- **Config_Info_Panel**: The Bevy UI panel toggled by pressing `I` that displays all active configuration values.
- **Stats_Panel**: The Bevy UI panel toggled by pressing `T` that displays population trait statistics.
- **Actor_Inspector**: The Bevy UI panel that displays a selected actor's full state including trait values.

## Requirements

### Requirement 1: Extend HeritableTraits Struct

**User Story:** As a simulation developer, I want `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` stored as per-actor heritable traits, so that these values can diverge across the population through mutation.

#### Acceptance Criteria

1. THE HeritableTraits struct SHALL contain seven fields: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`, `max_tumble_steps`, `reproduction_cost`, and `offspring_energy`.
2. WHEN HeritableTraits is constructed from ActorConfig via `from_config`, THE HeritableTraits struct SHALL initialize `max_tumble_steps` from `ActorConfig.max_tumble_steps`, `reproduction_cost` from `ActorConfig.reproduction_cost`, and `offspring_energy` from `ActorConfig.offspring_energy`.
3. THE HeritableTraits struct SHALL store `max_tumble_steps` as `u16`, `reproduction_cost` as `f32`, and `offspring_energy` as `f32`.

### Requirement 2: Heritable Trait Mutation

**User Story:** As a simulation developer, I want the three new traits to undergo gaussian mutation during binary fission, so that offspring exhibit variation in tumble length, reproduction cost, and offspring provisioning.

#### Acceptance Criteria

1. WHEN `HeritableTraits::mutate` is called, THE HeritableTraits struct SHALL apply gaussian noise to all seven trait fields and clamp each to its configured range.
2. WHEN mutating `max_tumble_steps`, THE HeritableTraits struct SHALL convert the `u16` value to `f32`, add gaussian noise, round to the nearest integer, and clamp to the range `[trait_max_tumble_steps_min, trait_max_tumble_steps_max]` before converting back to `u16`.
3. WHEN mutating `reproduction_cost`, THE HeritableTraits struct SHALL add gaussian noise and clamp to the range `[trait_reproduction_cost_min, trait_reproduction_cost_max]`.
4. WHEN mutating `offspring_energy`, THE HeritableTraits struct SHALL add gaussian noise and clamp to the range `[trait_offspring_energy_min, trait_offspring_energy_max]`.
5. WHEN `mutation_stddev` is `0.0`, THE HeritableTraits struct SHALL leave all seven trait fields unchanged.

### Requirement 3: ActorConfig Clamp Bounds

**User Story:** As a simulation operator, I want configurable clamp bounds for the three new heritable traits, so that I can constrain the evolutionary search space.

#### Acceptance Criteria

1. THE ActorConfig struct SHALL contain `trait_max_tumble_steps_min` (`u16`) and `trait_max_tumble_steps_max` (`u16`) fields with defaults `1` and `50` respectively.
2. THE ActorConfig struct SHALL contain `trait_reproduction_cost_min` (`f32`) and `trait_reproduction_cost_max` (`f32`) fields with defaults `0.1` and `100.0` respectively.
3. THE ActorConfig struct SHALL contain `trait_offspring_energy_min` (`f32`) and `trait_offspring_energy_max` (`f32`) fields with defaults `0.1` and `100.0` respectively.
4. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `trait_max_tumble_steps_min < 1`.
5. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `trait_max_tumble_steps_min >= trait_max_tumble_steps_max`.
6. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `trait_reproduction_cost_min <= 0.0` or `trait_reproduction_cost_min >= trait_reproduction_cost_max`.
7. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `trait_offspring_energy_min <= 0.0` or `trait_offspring_energy_min >= trait_offspring_energy_max`.
8. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `trait_offspring_energy_max > max_energy`.
9. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where the default `max_tumble_steps` value falls outside `[trait_max_tumble_steps_min, trait_max_tumble_steps_max]`.
10. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where the default `reproduction_cost` value falls outside `[trait_reproduction_cost_min, trait_reproduction_cost_max]`.
11. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where the default `offspring_energy` value falls outside `[trait_offspring_energy_min, trait_offspring_energy_max]`.

### Requirement 4: Sensing System Uses Per-Actor max_tumble_steps

**User Story:** As a simulation developer, I want the sensing system to read `max_tumble_steps` from each actor's heritable traits instead of the global config, so that tumble length varies per actor.

#### Acceptance Criteria

1. WHEN `run_actor_sensing` initiates a new Lévy flight tumble for an actor, THE Sensing_System SHALL pass `actor.traits.max_tumble_steps` to `sample_tumble_steps` instead of `config.max_tumble_steps`.

### Requirement 5: Reproduction System Uses Per-Actor reproduction_cost

**User Story:** As a simulation developer, I want the reproduction system to read `reproduction_cost` from each actor's heritable traits instead of the global config, so that fission cost varies per actor.

#### Acceptance Criteria

1. WHEN `run_actor_reproduction` deducts energy from a parent actor during fission, THE Reproduction_System SHALL use `actor.traits.reproduction_cost` instead of `config.reproduction_cost`.

### Requirement 6: Spawn System Uses Per-Actor offspring_energy

**User Story:** As a simulation developer, I want the spawn system to read `offspring_energy` from the parent's heritable traits instead of the global config, so that offspring provisioning varies per actor.

#### Acceptance Criteria

1. WHEN `run_actor_reproduction` pushes a spawn request to the spawn buffer, THE Reproduction_System SHALL use `actor.traits.offspring_energy` as the offspring energy value instead of `config.offspring_energy`.
2. WHEN `run_deferred_spawn` creates an offspring Actor, THE Spawn_System SHALL assign the energy value from the spawn buffer (derived from parent traits) to the offspring.

### Requirement 7: Trait Visualization Update

**User Story:** As a simulation observer, I want the stats panel, actor inspector, and config info panel to display the three new heritable traits, so that I can monitor evolutionary dynamics.

#### Acceptance Criteria

1. WHEN computing population trait statistics, THE Trait_Stats computation SHALL include `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` alongside the existing four traits, producing statistics for all seven traits.
2. WHEN displaying the stats panel, THE Stats_Panel SHALL show rows for all seven heritable traits with min, p25, p50, p75, max, and mean values.
3. WHEN displaying a selected actor's state, THE Actor_Inspector SHALL show the actor's `max_tumble_steps`, `reproduction_cost`, and `offspring_energy` trait values.
4. WHEN displaying the config info panel, THE Config_Info_Panel SHALL show the new clamp bound fields (`trait_max_tumble_steps_min/max`, `trait_reproduction_cost_min/max`, `trait_offspring_energy_min/max`).
5. THE TraitStats resource SHALL use an array of size 7 for per-trait statistics instead of size 4.

### Requirement 8: Configuration File and Documentation Update

**User Story:** As a simulation operator, I want the example config file and documentation to reflect the new clamp bound fields, so that I can configure the new heritable traits.

#### Acceptance Criteria

1. WHEN the example configuration file is updated, THE example_config.toml SHALL include `trait_max_tumble_steps_min`, `trait_max_tumble_steps_max`, `trait_reproduction_cost_min`, `trait_reproduction_cost_max`, `trait_offspring_energy_min`, and `trait_offspring_energy_max` fields with comments explaining their purpose.
2. WHEN the config-documentation steering file is updated, THE config-documentation.md SHALL list all six new ActorConfig fields in the `[actor]` configuration reference table.
3. WHEN the config-documentation steering file is updated, THE config-documentation.md SHALL list all seven heritable traits in the Heritable Trait Update Rule section.
