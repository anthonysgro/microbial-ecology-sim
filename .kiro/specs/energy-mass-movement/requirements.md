# Requirements Document

## Introduction

Energy-as-mass movement physics for actors. An actor's current energy level represents its physical mass, making movement cost proportional to energy. High-energy actors are heavier and pay more to move; low-energy actors are lighter and move cheaply. This creates an emergent tradeoff between energy hoarding (powerful but sluggish) and staying lean (fast but vulnerable), without any top-down behavioral scripting.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, carrying energy reserves and heritable traits. The atomic simulation unit.
- **Movement_Cost_Formula**: The computation `base_movement_cost * (actor.energy / reference_energy)` that determines the actual energy deducted when an actor moves.
- **Base_Movement_Cost**: A configuration parameter (renamed from the current flat `movement_cost`) representing the movement cost at the reference energy level.
- **Reference_Energy**: A configuration parameter representing the "normal" energy level at which actual movement cost equals the base movement cost.
- **Movement_System**: The `run_actor_movement` function in `src/grid/actor_systems.rs` that executes actor relocation each tick.
- **ActorConfig**: The configuration struct holding all actor-related parameters, loaded from TOML at startup.
- **Config_Info_Panel**: The Bevy visualization overlay toggled by pressing `I`, displaying all active configuration values.
- **Inert_Actor**: An actor whose energy has dropped to zero or below, which no longer moves or consumes resources.

## Requirements

### Requirement 1: Energy-Proportional Movement Cost Calculation

**User Story:** As a simulation designer, I want movement cost to scale with an actor's current energy, so that heavier actors pay more to move and lighter actors pay less, creating an emergent mass-mobility tradeoff.

#### Acceptance Criteria

1. WHEN an actor successfully moves to an adjacent cell, THE Movement_System SHALL compute the actual movement cost as `base_movement_cost * (actor.energy / reference_energy)` and deduct that amount from the actor's energy.
2. WHEN an actor's energy is equal to the reference_energy, THE Movement_System SHALL deduct exactly the base_movement_cost.
3. WHEN an actor's energy is above the reference_energy, THE Movement_System SHALL deduct more than the base_movement_cost.
4. WHEN an actor's energy is below the reference_energy, THE Movement_System SHALL deduct less than the base_movement_cost.
5. THE Movement_System SHALL compute the movement cost using the actor's energy value at the time of the move, before the deduction is applied.

### Requirement 2: Movement Cost Floor

**User Story:** As a simulation designer, I want movement cost to have a minimum floor, so that actors with near-zero energy still pay a non-trivial cost to move and cannot exploit negligible movement costs indefinitely.

#### Acceptance Criteria

1. THE Movement_System SHALL enforce a minimum movement cost floor of `base_movement_cost * 0.1` (10% of base cost) for every successful move.
2. WHEN the computed proportional movement cost falls below the floor, THE Movement_System SHALL apply the floor value instead.
3. THE Movement_System SHALL compute the actual movement cost as `max(base_movement_cost * (actor.energy / reference_energy), base_movement_cost * 0.1)`.

### Requirement 3: Configuration Parameters

**User Story:** As a simulation operator, I want to configure the energy-mass movement parameters via TOML, so that I can tune the mass-mobility tradeoff without recompiling.

#### Acceptance Criteria

1. THE ActorConfig SHALL expose a `base_movement_cost` field (f32, default 0.5) replacing the current `movement_cost` field.
2. THE ActorConfig SHALL expose a `reference_energy` field (f32, default 25.0) representing the energy level at which movement cost equals the base cost.
3. WHEN the `reference_energy` field is set to a value less than or equal to zero, THE configuration parser SHALL reject the configuration with a descriptive error.
4. WHEN the `base_movement_cost` field is set to a negative value, THE configuration parser SHALL reject the configuration with a descriptive error.
5. THE ActorConfig SHALL parse both new fields from the `[actor]` section of the TOML configuration file.

### Requirement 4: Numerical Safety

**User Story:** As a simulation engineer, I want the movement cost computation to remain numerically stable, so that the simulation never produces NaN or infinite energy values from the movement formula.

#### Acceptance Criteria

1. IF the computed movement cost is NaN or infinite, THEN THE Movement_System SHALL return a `TickError::NumericalError` identifying the actor's cell index and the invalid value.
2. WHEN an actor's energy is exactly zero, THE Movement_System SHALL apply the floor cost (Requirement 2) and the actor's energy SHALL become negative, triggering inert transition.
3. THE Movement_System SHALL remain deterministic: given identical actor state and configuration, the computed movement cost SHALL be identical across runs.

### Requirement 5: Inert Transition on Movement

**User Story:** As a simulation designer, I want actors that deplete their energy through movement to become inert, so that the existing energy-depletion lifecycle is preserved under the new formula.

#### Acceptance Criteria

1. WHEN an actor's energy drops to zero or below after a movement cost deduction, THE Movement_System SHALL mark the actor as inert.
2. THE Movement_System SHALL preserve the existing inert-skip behavior: inert actors SHALL NOT attempt movement.

### Requirement 6: Documentation and Visualization Updates

**User Story:** As a simulation operator, I want the configuration documentation and visualization to reflect the new movement parameters, so that I can understand and monitor the energy-mass system.

#### Acceptance Criteria

1. THE example_config.toml SHALL include `base_movement_cost` and `reference_energy` fields with explanatory comments in the `[actor]` section.
2. THE example_config.toml SHALL remove the old `movement_cost` field.
3. THE Config_Info_Panel SHALL display the `base_movement_cost` and `reference_energy` values in the Actors section.
4. THE Config_Info_Panel SHALL remove the old `movement_cost` display line.
5. THE config-documentation.md steering file SHALL be updated to reflect the renamed and new fields in the `[actor]` configuration reference table.

### Requirement 7: Backward Compatibility

**User Story:** As a simulation operator, I want existing configurations that use the old `movement_cost` field to fail with a clear error, so that I know to update my config files.

#### Acceptance Criteria

1. WHEN a TOML configuration file contains the old `movement_cost` field under `[actor]`, THE configuration parser SHALL reject it as an unknown key (enforced by `deny_unknown_fields`).
