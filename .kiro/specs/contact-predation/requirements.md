# Requirements Document

## Introduction

Contact predation enables actors to consume adjacent actors based on energy dominance, gated by kin recognition via genetic distance. When two actors occupy neighboring cells, the higher-energy actor may predate the lower-energy one, absorbing a configurable fraction of the prey's energy. Predation is suppressed between genetically similar actors, determined by normalized Euclidean distance across the heritable trait vector compared against a per-actor heritable kin tolerance threshold. This creates emergent predator-prey dynamics, kin selection, and speciation from purely local physics.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, carrying energy and heritable traits.
- **Predator**: The actor with strictly higher energy in an adjacent pair that initiates predation.
- **Prey**: The actor with strictly lower energy in an adjacent pair that is consumed during predation.
- **Genetic_Distance**: The normalized Euclidean distance between two actors' heritable trait vectors, where each trait is normalized to [0, 1] using its configured clamp bounds.
- **Kin_Tolerance**: A per-actor heritable trait controlling the genetic distance threshold below which predation is suppressed. Low values produce xenophobic actors; high values produce cosmopolitan actors.
- **Absorption_Efficiency**: A global configuration parameter in [0, 1] controlling the fraction of prey energy transferred to the predator upon successful predation.
- **Predation_Phase**: A deterministic phase in the tick cycle where contact predation is evaluated for all adjacent actor pairs.
- **Adjacency**: The 4-neighborhood (North, South, East, West) of a grid cell.
- **HeritableTraits**: The struct carrying per-actor heritable trait values, currently 8 traits, extended to 9 with `kin_tolerance`.
- **ActorConfig**: The configuration struct holding global actor parameters and per-trait clamp bounds.
- **TraitStats**: Pre-computed population statistics for heritable traits displayed in the visualization.

## Requirements

### Requirement 1: Kin Tolerance Heritable Trait

**User Story:** As a simulation designer, I want each actor to carry a heritable kin tolerance trait, so that predation selectivity evolves independently per lineage.

#### Acceptance Criteria

1. THE HeritableTraits struct SHALL include a `kin_tolerance` field of type `f32`.
2. WHEN a new actor is created from config defaults, THE HeritableTraits constructor SHALL initialize `kin_tolerance` from a new `kin_tolerance` field on ActorConfig.
3. WHEN an actor reproduces, THE mutation system SHALL apply proportional gaussian mutation to `kin_tolerance` using the actor's `mutation_rate`, then clamp the result to `[trait_kin_tolerance_min, trait_kin_tolerance_max]`.
4. THE ActorConfig SHALL include `kin_tolerance` (default seed value), `trait_kin_tolerance_min` (default 0.0), and `trait_kin_tolerance_max` (default 1.0) fields.
5. THE ActorConfig SHALL include an `absorption_efficiency` field of type `f32` with default value 0.5, constrained to the range (0.0, 1.0].

### Requirement 2: Genetic Distance Computation

**User Story:** As a simulation designer, I want genetic distance between actors computed as normalized Euclidean distance across the trait vector, so that kin recognition is grounded in measurable phenotypic similarity.

#### Acceptance Criteria

1. WHEN computing genetic distance between two actors, THE Predation_Phase SHALL normalize each of the 9 heritable traits to [0, 1] by applying `(value - trait_min) / (trait_max - trait_min)` using the configured clamp bounds for each trait.
2. WHEN computing genetic distance, THE Predation_Phase SHALL compute the Euclidean distance across the 9-dimensional normalized trait vector divided by `sqrt(9)` to produce a value in [0, 1].
3. IF a trait's clamp range has `trait_max == trait_min`, THEN THE Predation_Phase SHALL treat the normalized value for that trait as 0.0 for both actors.

### Requirement 3: Predation Eligibility

**User Story:** As a simulation designer, I want predation gated by energy dominance and genetic distance, so that only energetically superior actors predate genetically distant neighbors.

#### Acceptance Criteria

1. WHEN two actors occupy adjacent cells (4-neighborhood), THE Predation_Phase SHALL evaluate predation eligibility between the pair.
2. WHEN evaluating a pair, THE Predation_Phase SHALL designate the actor with strictly higher energy as the potential predator and the other as the potential prey.
3. WHEN two adjacent actors have equal energy, THE Predation_Phase SHALL suppress predation between the pair.
4. WHEN the Genetic_Distance between predator and prey is less than the predator's `kin_tolerance`, THE Predation_Phase SHALL suppress predation for that pair.
5. WHEN the Genetic_Distance between predator and prey is greater than or equal to the predator's `kin_tolerance`, THE Predation_Phase SHALL permit predation.
6. WHILE an actor is marked inert, THE Predation_Phase SHALL exclude the inert actor from predation evaluation as both predator and prey.

### Requirement 4: Predation Execution

**User Story:** As a simulation designer, I want successful predation to transfer energy from prey to predator and remove the prey, so that predator-prey dynamics emerge from local physics.

#### Acceptance Criteria

1. WHEN predation is permitted, THE Predation_Phase SHALL add `prey.energy * absorption_efficiency` to the predator's energy, clamped to `max_energy`.
2. WHEN predation is permitted, THE Predation_Phase SHALL mark the prey as inert and queue the prey for deferred removal.
3. WHEN predation occurs, THE Predation_Phase SHALL ensure each actor participates in at most one predation event per tick, either as predator or as prey.
4. THE Predation_Phase SHALL resolve predation order deterministically by iterating actors in ascending registry slot index order.

### Requirement 5: Tick Phase Integration

**User Story:** As a simulation designer, I want predation to execute as a deterministic phase in the tick cycle, so that predation interacts correctly with other actor systems.

#### Acceptance Criteria

1. THE Predation_Phase SHALL execute after the deferred spawn phase and before the movement phase within `run_actor_phases`.
2. THE Predation_Phase SHALL use a deterministic RNG seeded from the grid seed and tick number for any tie-breaking or randomized selection.
3. WHEN the Predation_Phase completes, THE system SHALL run deferred removal for predated actors before proceeding to the movement phase.
4. THE Predation_Phase SHALL perform zero heap allocations during per-actor iteration by reusing pre-allocated buffers.

### Requirement 6: Configuration

**User Story:** As a simulation operator, I want predation behavior controlled by configuration parameters, so that I can tune predator-prey dynamics without code changes.

#### Acceptance Criteria

1. THE ActorConfig SHALL expose `absorption_efficiency` as a TOML-configurable field under the `[actor]` section.
2. THE ActorConfig SHALL expose `kin_tolerance`, `trait_kin_tolerance_min`, and `trait_kin_tolerance_max` as TOML-configurable fields under the `[actor]` section.
3. WHEN `absorption_efficiency` is outside the range (0.0, 1.0], THE configuration parser SHALL reject the configuration with a descriptive error.
4. WHEN `trait_kin_tolerance_min >= trait_kin_tolerance_max`, THE configuration parser SHALL reject the configuration with a descriptive error.

### Requirement 7: Visualization Updates

**User Story:** As a simulation observer, I want the visualization to display kin tolerance statistics and predation-related actor state, so that I can monitor emergent speciation and predation dynamics.

#### Acceptance Criteria

1. THE TraitStats array SHALL be extended from 8 to 9 elements to include `kin_tolerance` statistics.
2. WHEN computing trait statistics, THE stats system SHALL include `kin_tolerance` in the population statistics (min, max, mean, p25, p50, p75).
3. WHEN displaying the population stats panel, THE formatting function SHALL include a `kin_tolerance` row.
4. WHEN displaying the actor inspector panel, THE formatting function SHALL include the selected actor's `kin_tolerance` value.
5. WHEN displaying the config info panel, THE formatting function SHALL include `absorption_efficiency`, `kin_tolerance`, `trait_kin_tolerance_min`, and `trait_kin_tolerance_max`.

### Requirement 8: Documentation Updates

**User Story:** As a developer, I want configuration documentation kept in sync with the new fields, so that the example config and steering files remain the single source of truth.

#### Acceptance Criteria

1. WHEN the `kin_tolerance` trait and `absorption_efficiency` config fields are added, THE `example_config.toml` SHALL include the new fields with comments explaining purpose and valid range.
2. WHEN the `kin_tolerance` trait is added, THE `config-documentation.md` steering file SHALL be updated to list the new config fields in the Configuration Reference tables and add `kin_tolerance` to the heritable trait list.
