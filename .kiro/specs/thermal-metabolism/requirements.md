# Requirements Document

## Introduction

Add thermal performance curves to actor metabolism so that each actor pays an extra energy cost based on the mismatch between its local cell heat and its preferred temperature. The preferred temperature (`optimal_temp`) is a heritable trait subject to mutation and natural selection. A configurable `thermal_sensitivity` parameter controls the steepness of the quadratic penalty curve. Over generations, populations specialize on local thermal conditions — thermophiles near heat sources, cold-adapted populations in cooler regions — and track shifting thermal landscapes as heat sources deplete and respawn.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, with internal energy reserves and heritable traits.
- **HeritableTraits**: Plain data struct stored inline in each Actor, carrying per-actor trait values inherited from parent during fission with proportional gaussian mutation.
- **ActorConfig**: Immutable configuration struct for actor metabolism, sensing, and spawning parameters. Includes seed genome defaults and trait clamp bounds.
- **Thermal_Performance_Curve**: A quadratic penalty function `extra_cost = thermal_sensitivity * (cell_heat - optimal_temp)^2` that converts thermal mismatch into an additional per-tick energy cost.
- **optimal_temp**: A heritable f32 trait on each Actor representing the cell heat value at which the actor's metabolism is most efficient (zero thermal penalty).
- **thermal_sensitivity**: An f32 configuration parameter on ActorConfig controlling the steepness of the quadratic thermal penalty curve. Higher values impose harsher penalties for thermal mismatch.
- **cell_heat**: The per-cell heat value read from the grid's heat buffer at the actor's `cell_index`.
- **Metabolism_System**: The `run_actor_metabolism` function in `src/grid/actor_systems.rs`, a HOT-path system that processes energy balance for all actors each tick.

## Requirements

### Requirement 1: Heritable optimal_temp Trait

**User Story:** As a simulation designer, I want each actor to carry a heritable preferred temperature trait, so that natural selection can drive thermal specialization across populations.

#### Acceptance Criteria

1. THE HeritableTraits struct SHALL include an `optimal_temp: f32` field representing the actor's preferred temperature.
2. WHEN a new actor is created from config defaults, THE `HeritableTraits::from_config` function SHALL initialize `optimal_temp` from the `ActorConfig::optimal_temp` seed genome default.
3. WHEN an actor reproduces, THE `HeritableTraits::mutate` function SHALL apply proportional gaussian mutation to `optimal_temp` and clamp the result to `[trait_optimal_temp_min, trait_optimal_temp_max]`.
4. THE ActorConfig struct SHALL include `optimal_temp: f32` as the seed genome default for the heritable trait.
5. THE ActorConfig struct SHALL include `trait_optimal_temp_min: f32` and `trait_optimal_temp_max: f32` clamp bound fields.
6. THE ActorConfig SHALL validate that `trait_optimal_temp_min < trait_optimal_temp_max` and that `optimal_temp` falls within `[trait_optimal_temp_min, trait_optimal_temp_max]`.

### Requirement 2: Thermal Sensitivity Configuration

**User Story:** As a simulation designer, I want a configurable parameter controlling how steeply thermal mismatch penalizes actors, so that I can tune the selection pressure for thermal specialization.

#### Acceptance Criteria

1. THE ActorConfig struct SHALL include a `thermal_sensitivity: f32` field controlling the steepness of the quadratic thermal penalty curve.
2. THE ActorConfig SHALL validate that `thermal_sensitivity` is non-negative and finite.
3. WHEN `thermal_sensitivity` is `0.0`, THE Metabolism_System SHALL impose zero thermal penalty on all actors regardless of thermal mismatch.

### Requirement 3: Thermal Performance Curve in Metabolism

**User Story:** As a simulation designer, I want actors to pay an extra energy cost based on the distance between their local cell heat and their preferred temperature, so that thermal mismatch creates selective pressure for local adaptation.

#### Acceptance Criteria

1. WHEN the Metabolism_System processes an active (non-inert) actor, THE Metabolism_System SHALL read the cell heat value from the grid's heat buffer at the actor's `cell_index`.
2. WHEN the Metabolism_System processes an active actor, THE Metabolism_System SHALL compute the thermal penalty as `thermal_sensitivity * (cell_heat - actor.traits.optimal_temp).powi(2)`.
3. WHEN the Metabolism_System processes an active actor, THE Metabolism_System SHALL subtract the computed thermal penalty from the actor's energy in addition to the existing basal energy decay.
4. WHEN the Metabolism_System processes an inert actor, THE Metabolism_System SHALL apply no thermal penalty.
5. THE Metabolism_System SHALL perform zero heap allocations when computing the thermal penalty (HOT path constraint).
6. IF the thermal penalty computation produces NaN or infinite energy, THEN THE Metabolism_System SHALL return a `TickError::NumericalError`.

### Requirement 4: Genetic Distance Update

**User Story:** As a simulation designer, I want the genetic distance computation to include `optimal_temp`, so that thermal specialization contributes to kin recognition and predation decisions.

#### Acceptance Criteria

1. THE `genetic_distance` function SHALL include `optimal_temp` in the normalized Euclidean distance computation using `trait_optimal_temp_min` and `trait_optimal_temp_max` as normalization bounds.
2. THE `TRAIT_COUNT` constant SHALL be updated from 9 to 10 to reflect the addition of `optimal_temp`.

### Requirement 5: Visualization and Stats Updates

**User Story:** As a simulation observer, I want to see `optimal_temp` population statistics and per-actor values in the visualization panels, so that I can monitor thermal specialization dynamics.

#### Acceptance Criteria

1. THE `TraitStats.traits` array SHALL grow from `[SingleTraitStats; 9]` to `[SingleTraitStats; 10]`.
2. THE `compute_trait_stats_from_actors` function SHALL collect `optimal_temp` values and compute statistics at the new array index.
3. THE `TRAIT_NAMES` array SHALL include `"optimal_temp"` as the 10th entry.
4. THE `format_actor_info` function SHALL display the actor's `optimal_temp` value.
5. THE `format_trait_stats` function SHALL display population statistics for `optimal_temp`.

### Requirement 6: Configuration Documentation Updates

**User Story:** As a simulation user, I want the example config, config info panel, and config documentation steering file to reflect the new thermal metabolism fields, so that configuration stays in sync with the code.

#### Acceptance Criteria

1. THE `example_config.toml` file SHALL include `optimal_temp`, `thermal_sensitivity`, `trait_optimal_temp_min`, and `trait_optimal_temp_max` fields with comments explaining purpose and valid ranges.
2. THE `format_config_info` function SHALL display `thermal_sensitivity`, `optimal_temp` seed default, and `trait_optimal_temp` clamp range.
3. THE `config-documentation.md` steering file SHALL document all new ActorConfig fields (`optimal_temp`, `thermal_sensitivity`, `trait_optimal_temp_min`, `trait_optimal_temp_max`) in the configuration reference table.
4. THE `config-documentation.md` heritable trait list SHALL include `optimal_temp`.
