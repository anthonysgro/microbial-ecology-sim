# Requirements Document

## Introduction

Actors currently accumulate energy without bound. An actor parked on a renewable chemical source gains `consumption_rate * energy_conversion_factor - base_energy_decay` energy per tick indefinitely. No biological organism has infinite storage capacity. This feature introduces a `max_energy` ceiling on `ActorConfig` that caps actor energy reserves and — critically — makes consumption demand-driven: actors only extract from the environment what they can actually store, preserving cell chemical concentrations for neighboring actors and downstream diffusion.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, with an internal energy reserve. ECS entity stored in `ActorRegistry`.
- **ActorConfig**: Immutable per-tick configuration struct holding metabolic rates, sensing parameters, and energy constants. Deserialized from the `[actor]` TOML section.
- **Max_Energy**: The upper bound on an Actor's energy reserve. Energy cannot exceed this value after any metabolic operation.
- **Metabolism_System**: The `run_actor_metabolism` function that processes chemical consumption and energy balance each tick.
- **Demand_Driven_Consumption**: A consumption model where the Actor computes how much chemical it can actually use (given remaining storage headroom) and extracts only that amount from the cell.
- **Energy_Headroom**: The difference between `max_energy` and the Actor's current energy. Represents remaining storage capacity.
- **Chemical_Write_Buffer**: The write-side double buffer for chemical species concentrations, mutated during metabolism.

## Requirements

### Requirement 1: Max Energy Configuration

**User Story:** As a simulation designer, I want to configure a maximum energy capacity for actors, so that no actor accumulates unbounded energy reserves.

#### Acceptance Criteria

1. THE ActorConfig SHALL contain a `max_energy` field of type `f32` representing the maximum energy an Actor can hold.
2. WHEN `max_energy` is less than or equal to zero, THE configuration validation SHALL return an error.
3. WHEN `initial_energy` exceeds `max_energy`, THE configuration validation SHALL return an error.
4. THE ActorConfig default SHALL set `max_energy` to a finite positive value.

### Requirement 2: Energy Clamping After Metabolism

**User Story:** As a simulation designer, I want actor energy to be clamped to the configured ceiling after each metabolic tick, so that actors on rich sources hit saturation and stop accumulating excess energy.

#### Acceptance Criteria

1. WHEN an active Actor's energy after metabolic computation exceeds `max_energy`, THE Metabolism_System SHALL clamp the Actor's energy to `max_energy`.
2. WHEN an active Actor's energy after metabolic computation is at or below `max_energy`, THE Metabolism_System SHALL leave the Actor's energy unchanged by clamping.
3. THE Metabolism_System SHALL apply the energy clamp after adding consumption-derived energy and subtracting basal metabolic cost.

### Requirement 3: Demand-Driven Consumption

**User Story:** As a simulation designer, I want actors to extract only the chemical they can actually use from the environment, so that saturated actors do not deplete cell resources they cannot benefit from.

#### Acceptance Criteria

1. THE Metabolism_System SHALL compute Energy_Headroom as `max_energy - current_energy` before consumption, clamped to a minimum of zero.
2. THE Metabolism_System SHALL compute the maximum useful chemical as `Energy_Headroom / energy_conversion_factor`, representing the chemical quantity that would fill the Actor to capacity.
3. THE Metabolism_System SHALL compute actual consumption as the minimum of `consumption_rate`, available cell chemical, and maximum useful chemical.
4. THE Metabolism_System SHALL subtract only the actual consumption amount from the Chemical_Write_Buffer.
5. WHEN an Actor's energy is already at or above `max_energy`, THE Metabolism_System SHALL consume zero chemical from the cell.

### Requirement 4: Inert Actor Exclusion

**User Story:** As a simulation designer, I want inert actors to remain unaffected by the energy cap during their decay phase, so that the cap applies only to active metabolic gain.

#### Acceptance Criteria

1. WHILE an Actor is inert, THE Metabolism_System SHALL skip demand-driven consumption logic and energy clamping for that Actor.
2. WHILE an Actor is inert, THE Metabolism_System SHALL continue to subtract basal metabolic cost without applying the max energy ceiling.

### Requirement 5: TOML Configuration Support

**User Story:** As a simulation operator, I want to configure `max_energy` via the TOML configuration file, so that the energy cap is tunable without recompilation.

#### Acceptance Criteria

1. WHEN a `max_energy` field is present in the `[actor]` TOML section, THE configuration loader SHALL deserialize the value into `ActorConfig.max_energy`.
2. WHEN the `max_energy` field is omitted from the TOML file, THE configuration loader SHALL use the compiled default value.
3. THE configuration validation SHALL reject `max_energy` values that are NaN or infinite.

### Requirement 6: Documentation Updates

**User Story:** As a simulation operator, I want the README and example configuration file to reflect the new `max_energy` parameter, so that the documentation stays accurate and discoverable.

#### Acceptance Criteria

1. WHEN the `max_energy` field is added to `ActorConfig`, THE `example_config.toml` SHALL include a `max_energy` entry in the `[actor]` section with a comment explaining its purpose and constraints.
2. WHEN the `max_energy` field is added to `ActorConfig`, THE `README.md` SHALL document the `max_energy` parameter in the actor configuration section if such a section exists.

### Requirement 7: Determinism Preservation

**User Story:** As a simulation engineer, I want the energy cap and demand-driven consumption to preserve simulation determinism, so that replays produce identical results.

#### Acceptance Criteria

1. THE Metabolism_System SHALL apply demand-driven consumption and energy clamping in deterministic slot-index order.
2. IF an Actor's energy becomes NaN or infinite after clamping, THEN THE Metabolism_System SHALL return a `TickError::NumericalError`.
3. THE energy cap computation SHALL use identical arithmetic operations regardless of actor count or grid state, producing bit-identical results across runs with the same seed.

