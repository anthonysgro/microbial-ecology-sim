# Requirements Document

## Introduction

Actors currently consume any amount of chemical from their cell, even vanishingly small concentrations. This creates degenerate equilibrium states where actors park on depleting sources indefinitely, extracting trickles of chemical with no incentive to leave until the concentration reaches exactly zero. A configurable consumption threshold introduces a minimum chemical concentration below which actors refuse to eat, forcing migration when local resources drop below viable extraction density. This is biologically realistic: organisms require a minimum resource density to justify the metabolic cost of extraction.

## Glossary

- **Actor**: The atomic biological organism entity in the simulation. An ECS entity with physical components (energy, cell position, inert flag).
- **ActorConfig**: The plain-data configuration struct holding all actor metabolism, sensing, and spawning parameters. Immutable after construction.
- **Consumption_Threshold**: A minimum chemical concentration (species 0) below which an Actor treats the cell as empty and refuses to consume.
- **Metabolism_System**: The `run_actor_metabolism` function that executes chemical consumption and energy balance for all Actors each tick.
- **Sensing_System**: The `run_actor_sensing` function that computes movement targets based on local chemical gradients.
- **Chemical_Concentration**: The floating-point value representing the amount of chemical species 0 present in a grid cell.
- **Basal_Decay**: The fixed energy cost subtracted from every Actor each tick regardless of consumption.
- **Config_Validator**: The `validate_world_config` function that checks cross-field invariants on the loaded configuration.

## Requirements

### Requirement 1: Consumption Threshold Configuration Field

**User Story:** As a simulation operator, I want to configure a minimum chemical concentration for actor consumption, so that I can tune how aggressively actors exploit low-density resources.

#### Acceptance Criteria

1. THE ActorConfig SHALL include a `consumption_threshold` field of type `f32` with a default value of `0.0`.
2. WHEN a TOML configuration file omits the `consumption_threshold` field, THE ActorConfig SHALL use the default value of `0.0`, preserving backward compatibility with existing configurations.
3. WHEN a TOML configuration file includes the `consumption_threshold` field, THE ActorConfig SHALL deserialize the provided value.

### Requirement 2: Consumption Threshold Validation

**User Story:** As a simulation operator, I want invalid consumption threshold values to be rejected at startup, so that I can catch configuration errors before the simulation runs.

#### Acceptance Criteria

1. WHEN the `consumption_threshold` value is negative, THEN THE Config_Validator SHALL return a validation error indicating the value must be >= 0.0.
2. WHEN the `consumption_threshold` value is NaN or infinite, THEN THE Config_Validator SHALL return a validation error indicating the value must be finite.
3. WHEN the `consumption_threshold` value is >= 0.0 and finite, THE Config_Validator SHALL accept the value.

### Requirement 3: Metabolism Threshold Enforcement

**User Story:** As a simulation designer, I want actors to skip consumption when local chemical concentration is below the threshold, so that actors are forced to migrate away from depleted sources.

#### Acceptance Criteria

1. WHEN an active Actor's cell has Chemical_Concentration below the Consumption_Threshold, THE Metabolism_System SHALL skip chemical consumption for that Actor and apply only Basal_Decay.
2. WHEN an active Actor's cell has Chemical_Concentration at or above the Consumption_Threshold, THE Metabolism_System SHALL execute normal consumption logic (consuming up to `consumption_rate` from available chemical).
3. WHEN the Consumption_Threshold is `0.0`, THE Metabolism_System SHALL behave identically to the current implementation with no threshold check.

### Requirement 4: Sensing Threshold Awareness

**User Story:** As a simulation designer, I want the sensing system to treat sub-threshold cells as empty, so that actors do not waste movement chasing resources they cannot extract.

#### Acceptance Criteria

1. WHEN computing chemical gradients, THE Sensing_System SHALL treat any cell with Chemical_Concentration below the Consumption_Threshold as having zero concentration.
2. WHEN all neighboring cells and the current cell have Chemical_Concentration below the Consumption_Threshold, THE Sensing_System SHALL produce no movement target (actor stays in place).
3. WHEN the Consumption_Threshold is `0.0`, THE Sensing_System SHALL behave identically to the current implementation with no threshold adjustment.

### Requirement 5: Configuration Documentation Updates

**User Story:** As a simulation operator, I want the consumption threshold documented in all configuration references, so that I can discover and understand the parameter.

#### Acceptance Criteria

1. THE example configuration file (`example_config.toml`) SHALL include the `consumption_threshold` field with a comment explaining its purpose and valid range.
2. THE README configuration table SHALL include a row documenting `consumption_threshold` with its type, default, and description.
3. THE Bevy info panel (`format_config_info`) SHALL display the `consumption_threshold` value when actor configuration is present.
4. THE configuration documentation steering file (`config-documentation.md`) SHALL include `consumption_threshold` in the ActorConfig reference table.
