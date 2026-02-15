# Requirements Document

## Introduction

Add a configurable extraction cost to actor metabolism. Currently, chemical consumption is free — actors gain energy proportional to consumed chemical minus basal decay. This means actors sit indefinitely on nearly-depleted sources because even trace amounts yield net positive energy. The extraction cost introduces an energy cost per unit of chemical consumed, creating a natural break-even concentration below which consumption is unprofitable, forcing actors to migrate from depleted sources.

## Glossary

- **Actor**: The atomic biological organism entity in the simulation. An ECS entity with physical components (energy, cell position, inert flag).
- **ActorConfig**: Plain data struct holding all actor metabolism, sensing, and lifecycle configuration parameters. Immutable after construction.
- **Extraction_Cost**: A configurable `f32` field on `ActorConfig` representing the energy cost per unit of chemical consumed during metabolism.
- **Metabolism_System**: The `run_actor_metabolism` function in `src/grid/actor_systems.rs` that executes per-tick energy balance for all actors.
- **Break_Even_Concentration**: The chemical concentration at which energy gained from consumption exactly equals energy lost (extraction cost + basal decay). Computed as `base_energy_decay / (energy_conversion_factor - extraction_cost)`.
- **Config_Validator**: The `validate_world_config` function in `src/io/config_file.rs` that checks cross-field invariants on `WorldConfig`.
- **Info_Panel**: The Bevy visualization overlay toggled by pressing `I`, rendered by `format_config_info()` in `src/viz_bevy/setup.rs`.

## Requirements

### Requirement 1: Extraction Cost Configuration Field

**User Story:** As a simulation designer, I want to configure an extraction cost per unit of chemical consumed, so that I can control the metabolic profitability of consumption and create natural break-even dynamics.

#### Acceptance Criteria

1. THE ActorConfig SHALL include an `extraction_cost` field of type `f32` with a default value of `0.2`.
2. WHEN a TOML configuration file contains an `extraction_cost` field under `[actor]`, THE ActorConfig SHALL deserialize the provided value.
3. WHEN a TOML configuration file omits the `extraction_cost` field, THE ActorConfig SHALL use the default value of `0.2`.

### Requirement 2: Extraction Cost Validation

**User Story:** As a simulation designer, I want the system to reject nonsensical extraction cost values at load time, so that I can catch configuration errors before the simulation runs.

#### Acceptance Criteria

1. WHEN `extraction_cost` is negative, THE Config_Validator SHALL return a validation error indicating that `extraction_cost` must be >= 0.0.
2. WHEN `extraction_cost` is greater than or equal to `energy_conversion_factor`, THE Config_Validator SHALL return a validation error indicating that `extraction_cost` must be less than `energy_conversion_factor`.
3. WHEN `extraction_cost` is in the range `[0.0, energy_conversion_factor)`, THE Config_Validator SHALL accept the configuration.

### Requirement 3: Metabolism Equation Update

**User Story:** As a simulation designer, I want the metabolism system to deduct extraction cost from energy gained per unit consumed, so that actors face a real cost for consuming chemical and leave depleted sources.

#### Acceptance Criteria

1. WHEN an active actor consumes chemical during the metabolism phase, THE Metabolism_System SHALL compute energy delta as `consumed * (energy_conversion_factor - extraction_cost) - base_energy_decay`.
2. WHEN an actor is inert, THE Metabolism_System SHALL apply only basal decay without any extraction cost (inert actors do not consume).
3. FOR ALL valid ActorConfig values, THE Metabolism_System SHALL produce an energy delta equal to `consumed * (energy_conversion_factor - extraction_cost) - base_energy_decay` for active actors.

### Requirement 4: Demand-Driven Consumption with Extraction Cost

**User Story:** As a simulation designer, I want the max-useful consumption calculation to account for extraction cost, so that actors near max energy do not over-consume when extraction cost reduces net energy gain per unit.

#### Acceptance Criteria

1. WHEN computing the maximum useful consumption for an active actor, THE Metabolism_System SHALL use `headroom / (energy_conversion_factor - extraction_cost)` instead of `headroom / energy_conversion_factor`.
2. THE Metabolism_System SHALL ensure that the consumed amount does not cause actor energy to exceed `max_energy` after applying the extraction-cost-adjusted conversion.

### Requirement 5: Configuration Documentation Updates

**User Story:** As a simulation designer, I want all configuration documentation to reflect the new extraction cost field, so that I can understand and tune the parameter.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include the `extraction_cost` field with a comment explaining its purpose and valid range.
2. THE Info_Panel SHALL display the `extraction_cost` value when the actor config is present.
3. THE `config-documentation.md` steering file SHALL include `extraction_cost` in the `[actor]` — `ActorConfig` table with its type, default, and description.
4. THE `README.md` SHALL include `extraction_cost` in the ActorConfig parameter table with its type and description.
