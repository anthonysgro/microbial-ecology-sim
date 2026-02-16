# Requirements Document

## Introduction

Metabolic scaling transforms `base_energy_decay` into the central evolutionary tradeoff axis — the "metabolic rate" of each actor. Currently, `base_energy_decay` is pure cost with no upside, so evolution always minimizes it. This feature ties metabolic rate to actor capabilities: consumption efficiency, movement cost, and predation power all scale with metabolic rate.

The goal is rich biodiversity. High-metabolism actors are powerful foragers and predators but burn energy fast — they thrive near resource sources and dominate through aggression. Low-metabolism actors are efficient survivors that persist in resource-scarce zones — they move slowly, extract less per bite, but outlast competitors. Mid-range actors occupy intermediate niches. This creates the phenotypic diversity needed for a player to encounter meaningfully different organisms: fast aggressive hunters, slow resilient settlers, and everything in between.

The scaling is linear, anchored at a configurable reference metabolic rate. All formulas use simple arithmetic (multiply/divide) — no transcendental functions, no heap allocation, no new branching in hot loops.

## Glossary

- **Actor**: The atomic biological agent in the simulation. An ECS entity with physical components (energy, position, heritable traits).
- **Metabolic_Rate**: The per-actor heritable `base_energy_decay` trait value, reinterpreted as the central scaling axis for actor capabilities.
- **Reference_Metabolic_Rate**: A global config value (`reference_metabolic_rate`) defining the neutral point for scaling computations. At this rate, the scaling multiplier equals 1.0.
- **Metabolic_Ratio**: The dimensionless scaling factor `actor.traits.base_energy_decay / config.reference_metabolic_rate`. Values above 1.0 indicate high-metabolism actors; below 1.0 indicate low-metabolism actors.
- **Consumption_Efficiency_Scaling**: The mechanism by which Metabolic_Ratio modulates the effective energy gained per unit of chemical consumed. Higher ratio → more energy per unit consumed.
- **Movement_Cost_Scaling**: The mechanism by which Metabolic_Ratio modulates the energy cost of movement. Higher ratio → cheaper movement (inverse scaling).
- **Predation_Power_Scaling**: The mechanism by which Metabolic_Ratio modulates predation energy absorption. Higher ratio → more energy captured from prey.
- **Break_Even_Concentration**: The minimum chemical concentration at which consumption yields net positive energy. Used by the Sensing_System to decide whether a gradient is worth following.
- **ActorConfig**: The global configuration struct for actor parameters (`src/grid/actor_config.rs`).
- **Metabolism_System**: The `run_actor_metabolism` function in `src/grid/actor_systems.rs`.
- **Movement_System**: The `run_actor_movement` function in `src/grid/actor_systems.rs`.
- **Sensing_System**: The `run_actor_sensing` function in `src/grid/actor_systems.rs`.
- **Predation_System**: The `run_contact_predation` function in `src/grid/actor_systems.rs`.

## Requirements

### Requirement 1: Reference Metabolic Rate Configuration

**User Story:** As a simulation operator, I want to configure a reference metabolic rate, so that I can tune the neutral point of the metabolic scaling system and control the balance between high-metabolism and low-metabolism strategies.

#### Acceptance Criteria

1. THE ActorConfig SHALL include a `reference_metabolic_rate` field of type `f32` with a default value of 0.05.
2. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `reference_metabolic_rate` is not strictly positive.
3. WHEN validating ActorConfig, THE Config_Validator SHALL reject configurations where `reference_metabolic_rate` is not finite.
4. THE `example_config.toml` SHALL document the `reference_metabolic_rate` field with a comment explaining its purpose and valid range.
5. THE config info panel (`format_config_info`) SHALL display the `reference_metabolic_rate` value in the Actors section.

### Requirement 2: Consumption Efficiency Scaling

**User Story:** As a simulation designer, I want higher metabolic rate to yield better energy extraction from chemical, so that fast-burning actors gain a foraging advantage in resource-rich areas while slow-burning actors extract less per unit consumed.

#### Acceptance Criteria

1. WHEN computing energy gain in the Metabolism_System, THE Metabolism_System SHALL multiply the net energy conversion factor `(energy_conversion_factor - extraction_cost)` by the Metabolic_Ratio to produce the effective conversion factor.
2. WHEN an actor's Metabolic_Rate is above the Reference_Metabolic_Rate, THE Metabolism_System SHALL produce higher energy gain per unit consumed than an actor at the reference rate.
3. WHEN an actor's Metabolic_Rate is below the Reference_Metabolic_Rate, THE Metabolism_System SHALL produce lower energy gain per unit consumed than an actor at the reference rate.
4. THE Metabolism_System SHALL compute the `max_useful` consumption cap using the metabolically-scaled effective conversion factor so that actors do not over-consume beyond their energy headroom.
5. THE Metabolism_System SHALL compute the scaled energy gain using only multiplication and division operations.

### Requirement 3: Movement Cost Scaling

**User Story:** As a simulation designer, I want higher metabolic rate to reduce movement cost, so that fast-burning actors can forage effectively across the grid while slow-burning actors pay more to relocate, reinforcing the settler-vs-forager tradeoff.

#### Acceptance Criteria

1. WHEN computing movement cost in the Movement_System, THE Movement_System SHALL divide the proportional movement cost by the Metabolic_Ratio.
2. WHEN an actor's Metabolic_Rate is above the Reference_Metabolic_Rate, THE Movement_System SHALL produce lower movement cost than an actor at the reference rate with the same energy level.
3. WHEN an actor's Metabolic_Rate is below the Reference_Metabolic_Rate, THE Movement_System SHALL produce higher movement cost than an actor at the reference rate with the same energy level.
4. THE Movement_System SHALL maintain a floor on movement cost at `base_movement_cost * 0.1` to prevent zero-cost movement.
5. THE Movement_System SHALL compute the scaled movement cost using only multiplication and division operations.

### Requirement 4: Predation Power Scaling

**User Story:** As a simulation designer, I want higher metabolic rate to improve predation energy capture, so that aggressive high-metabolism actors gain more from hunting while low-metabolism actors are weaker predators, creating distinct predator and scavenger niches.

#### Acceptance Criteria

1. WHEN computing energy gain from predation in the Predation_System, THE Predation_System SHALL multiply the base `absorption_efficiency` by the Metabolic_Ratio of the predator to produce the effective absorption efficiency.
2. WHEN a predator's Metabolic_Rate is above the Reference_Metabolic_Rate, THE Predation_System SHALL transfer more energy from prey to predator than at the reference rate.
3. WHEN a predator's Metabolic_Rate is below the Reference_Metabolic_Rate, THE Predation_System SHALL transfer less energy from prey to predator than at the reference rate.
4. THE Predation_System SHALL clamp the effective absorption efficiency to a maximum of 1.0 to prevent energy creation from predation.
5. THE Predation_System SHALL compute the scaled absorption using only multiplication and division operations.

### Requirement 5: Sensing Break-Even Scaling

**User Story:** As a simulation designer, I want the sensing system's break-even concentration to account for metabolic scaling, so that actors make gradient-following decisions consistent with their actual metabolically-scaled energy economics.

#### Acceptance Criteria

1. WHEN computing the break-even concentration in the Sensing_System, THE Sensing_System SHALL use the metabolically-scaled effective conversion factor instead of the flat global conversion factor.
2. WHEN an actor has a high Metabolic_Rate, THE Sensing_System SHALL compute a lower break-even concentration (willing to follow weaker gradients because extraction is more efficient).
3. WHEN an actor has a low Metabolic_Rate, THE Sensing_System SHALL compute a higher break-even concentration (requires richer patches because extraction is less efficient).
4. THE Sensing_System SHALL compute the break-even concentration using only multiplication and division operations.

### Requirement 6: Scaling Continuity and Determinism

**User Story:** As a simulation engineer, I want the scaling formulas to be continuous, deterministic, and free of discontinuities, so that the simulation produces reproducible results and actors do not experience sudden behavioral jumps.

#### Acceptance Criteria

1. THE Metabolic_Ratio SHALL be a continuous function of the actor's Metabolic_Rate for all positive Metabolic_Rate values.
2. THE consumption efficiency scaling SHALL be a monotonically increasing function of Metabolic_Rate.
3. THE movement cost scaling SHALL be a monotonically decreasing function of Metabolic_Rate.
4. THE predation power scaling SHALL be a monotonically increasing function of Metabolic_Rate.
5. THE scaling computations SHALL introduce no new sources of nondeterminism.
6. THE scaling computations SHALL produce no NaN or Infinity values for any valid Metabolic_Rate within the configured trait clamp range.

### Requirement 7: Performance Constraints

**User Story:** As a systems engineer, I want the scaling computations to add minimal overhead to the per-tick actor loop, so that simulation throughput is not degraded.

#### Acceptance Criteria

1. THE scaling computations SHALL introduce zero heap allocations in the Metabolism_System, Movement_System, Sensing_System, and Predation_System.
2. THE scaling computations SHALL use only arithmetic operations (addition, subtraction, multiplication, division) with no transcendental functions in the scaling path.
3. THE scaling computations SHALL not introduce new branching in the per-actor inner loops beyond the existing control flow, except for the absorption efficiency clamp in the Predation_System.

### Requirement 8: Documentation Updates

**User Story:** As a simulation operator, I want all documentation and visualization to reflect the new metabolic scaling parameters, so that I can understand and configure the system correctly.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include the `reference_metabolic_rate` field with an explanatory comment.
2. THE config info panel in `format_config_info` SHALL display the `reference_metabolic_rate` value in the Actors section.
3. THE config-documentation steering file SHALL be updated with the new `reference_metabolic_rate` field in the ActorConfig reference table.
