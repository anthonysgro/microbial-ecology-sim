# Requirements Document

## Introduction

Add Lévy flight-based random foraging to actor movement. Currently, when no neighbor has a positive chemical gradient, actors stay put and eventually starve. This feature introduces a tumble mode: when the sensing system determines no neighbor is metabolically worth pursuing (all concentrations at or below the break-even threshold), the actor picks a random direction and commits to it for a number of steps drawn from a heavy-tailed power-law distribution. This produces the characteristic Lévy flight pattern — many short moves punctuated by occasional long straight-line runs — which is the theoretically optimal search strategy for sparse, randomly distributed resources.

## Glossary

- **Actor**: The atomic biological organism entity in the simulation. An ECS entity with physical components (energy, cell position, inert flag, tumble state).
- **ActorConfig**: Plain data struct holding all actor metabolism, sensing, and lifecycle configuration parameters. Immutable after construction.
- **Sensing_System**: The `run_actor_sensing` function in `src/grid/actor_systems.rs` that computes movement targets for all active actors each tick.
- **Movement_System**: The `run_actor_movement` function in `src/grid/actor_systems.rs` that relocates actors toward their movement targets each tick.
- **Tick_Orchestrator**: The `run_actor_phases` function in `src/grid/tick.rs` that sequences all actor phases per tick.
- **Break_Even_Concentration**: The chemical concentration at which energy gained from consumption exactly equals energy lost. Computed as `base_energy_decay / (energy_conversion_factor - extraction_cost)`.
- **Tumble_State**: Two fields on the Actor struct (`tumble_direction: u8`, `tumble_remaining: u16`) that track the current Lévy flight run direction and remaining steps.
- **Lévy_Exponent**: The power-law exponent α controlling the step-length distribution. Higher values produce shorter average runs; lower values (closer to 1.0) produce more long runs.
- **Max_Tumble_Steps**: The upper clamp on the number of steps in a single tumble run.
- **Config_Validator**: The `validate_world_config` function in `src/io/config_file.rs` that checks cross-field invariants on `WorldConfig`.
- **Info_Panel**: The Bevy visualization overlay toggled by pressing `I`, rendered by `format_config_info()` in `src/viz_bevy/setup.rs`.

## Requirements

### Requirement 1: Tumble State on Actor

**User Story:** As a simulation designer, I want each actor to carry tumble state, so that actors can commit to a random direction for multiple ticks during Lévy flight foraging.

#### Acceptance Criteria

1. THE Actor struct SHALL include a `tumble_direction` field of type `u8` encoding direction (0=North, 1=South, 2=West, 3=East).
2. THE Actor struct SHALL include a `tumble_remaining` field of type `u16` representing the number of steps left in the current tumble run.
3. WHEN a new Actor is created, THE Actor SHALL initialize `tumble_direction` to 0 and `tumble_remaining` to 0.

### Requirement 2: Break-Even Threshold Sensing

**User Story:** As a simulation designer, I want the sensing system to evaluate neighbors against the metabolic break-even concentration, so that actors only follow gradients that are actually profitable.

#### Acceptance Criteria

1. WHEN the Sensing_System evaluates an actor's neighbors, THE Sensing_System SHALL compute the break-even concentration as `base_energy_decay / (energy_conversion_factor - extraction_cost)` using values from ActorConfig.
2. WHEN all Von Neumann neighbors AND the current cell have chemical concentration at or below the break-even concentration, THE Sensing_System SHALL determine that no gradient is worth following.
3. WHEN at least one neighbor has chemical concentration above the break-even concentration AND has a positive gradient relative to the current cell, THE Sensing_System SHALL select the neighbor with the maximum positive gradient as the movement target (existing behavior preserved).

### Requirement 3: Lévy Flight Tumble Initiation

**User Story:** As a simulation designer, I want actors to enter a Lévy flight tumble when no worthwhile gradient exists, so that actors explore the grid instead of starving in place.

#### Acceptance Criteria

1. WHEN the Sensing_System determines no gradient is worth following AND the actor's `tumble_remaining` is 0, THE Sensing_System SHALL sample a new tumble direction uniformly from {North, South, West, East} and a step count from the power-law distribution.
2. WHEN sampling a tumble step count, THE Sensing_System SHALL draw from a discrete power-law distribution `P(steps = k) ∝ k^(-α)` where α is `levy_exponent` from ActorConfig, clamped to `[1, max_tumble_steps]`.
3. WHEN a new tumble is initiated, THE Sensing_System SHALL set the actor's `tumble_direction` and `tumble_remaining` fields and compute the movement target as the adjacent cell in the chosen direction.

### Requirement 4: Tumble Continuation and Termination

**User Story:** As a simulation designer, I want tumbling actors to continue in their chosen direction until the run completes or an obstacle is hit, so that the Lévy flight pattern produces coherent straight-line runs.

#### Acceptance Criteria

1. WHILE an actor's `tumble_remaining` is greater than 0 AND no neighbor has concentration above the break-even threshold, THE Sensing_System SHALL compute the movement target as the adjacent cell in `tumble_direction` and decrement `tumble_remaining` by 1.
2. WHEN a tumbling actor's target cell is out of bounds (grid boundary), THE Sensing_System SHALL reset `tumble_remaining` to 0 and set the movement target to None for that tick.
3. WHEN a tumbling actor senses at least one neighbor with concentration above the break-even threshold, THE Sensing_System SHALL reset `tumble_remaining` to 0 and select the best gradient neighbor as the movement target (gradient-following takes priority over tumble).
4. WHEN the Movement_System finds a tumbling actor's target cell is occupied, THE Movement_System SHALL leave the actor in place, and the Sensing_System on the next tick SHALL detect the failed move and reset `tumble_remaining` to 0 if the target cell in `tumble_direction` remains blocked.

### Requirement 5: Lévy Flight Step Distribution

**User Story:** As a simulation designer, I want the tumble step count to follow a power-law distribution, so that actors exhibit the biologically optimal mix of short exploratory moves and long ballistic runs.

#### Acceptance Criteria

1. THE Sensing_System SHALL implement inverse transform sampling for the power-law distribution: draw `u ~ Uniform(0,1)`, compute `steps = floor(u^(-1/(α-1)))`, clamp to `[1, max_tumble_steps]`.
2. FOR ALL valid `levy_exponent` values (α > 1.0) and `max_tumble_steps` values (≥ 1), THE step distribution SHALL produce values in the range `[1, max_tumble_steps]`.
3. THE step distribution SHALL produce shorter runs more frequently than longer runs, consistent with a heavy-tailed power-law distribution.

### Requirement 6: Deterministic RNG for Tumble Sampling

**User Story:** As a simulation designer, I want tumble sampling to be fully deterministic, so that simulation replay produces identical actor trajectories given the same seed.

#### Acceptance Criteria

1. WHEN the Tick_Orchestrator begins actor phases, THE Tick_Orchestrator SHALL create a per-tick RNG seeded deterministically from the simulation master seed and the current tick number.
2. THE Sensing_System SHALL accept the per-tick RNG as a parameter and use it for all tumble direction and step count sampling.
3. FOR ALL simulation runs with the same seed and configuration, THE tumble sampling SHALL produce identical sequences of directions and step counts.

### Requirement 7: Lévy Flight Configuration Fields

**User Story:** As a simulation designer, I want to configure the Lévy exponent and maximum tumble steps, so that I can tune the foraging behavior for different resource distributions.

#### Acceptance Criteria

1. THE ActorConfig SHALL include a `levy_exponent` field of type `f32` with a default value of 1.5.
2. THE ActorConfig SHALL include a `max_tumble_steps` field of type `u16` with a default value of 20.
3. WHEN a TOML configuration file contains `levy_exponent` or `max_tumble_steps` under `[actor]`, THE ActorConfig SHALL deserialize the provided values.
4. WHEN a TOML configuration file omits these fields, THE ActorConfig SHALL use the default values.

### Requirement 8: Lévy Flight Configuration Validation

**User Story:** As a simulation designer, I want the system to reject invalid Lévy flight configuration values at load time, so that I can catch errors before the simulation runs.

#### Acceptance Criteria

1. WHEN `levy_exponent` is less than or equal to 1.0, THE Config_Validator SHALL return a validation error indicating that `levy_exponent` must be greater than 1.0.
2. WHEN `max_tumble_steps` is 0, THE Config_Validator SHALL return a validation error indicating that `max_tumble_steps` must be at least 1.
3. WHEN `levy_exponent` is greater than 1.0 AND `max_tumble_steps` is at least 1, THE Config_Validator SHALL accept the configuration.

### Requirement 9: Sensing System Signature Update

**User Story:** As a simulation designer, I want the sensing system to receive the additional parameters it needs for Lévy flight, so that it can compute break-even thresholds and sample tumble state.

#### Acceptance Criteria

1. THE Sensing_System SHALL accept a mutable reference to the ActorRegistry (to update tumble state on actors).
2. THE Sensing_System SHALL accept a reference to ActorConfig (to read break-even threshold parameters and Lévy flight configuration).
3. THE Sensing_System SHALL accept a mutable reference to an RNG implementing the `Rng` trait (for tumble sampling).

### Requirement 10: Configuration Documentation Updates

**User Story:** As a simulation designer, I want all configuration documentation to reflect the new Lévy flight fields, so that I can understand and tune the parameters.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include `levy_exponent` and `max_tumble_steps` fields under `[actor]` with comments explaining their purpose and valid ranges.
2. THE Info_Panel SHALL display `levy_exponent` and `max_tumble_steps` values when the actor config is present.
3. THE `config-documentation.md` steering file SHALL include `levy_exponent` and `max_tumble_steps` in the `[actor]` — `ActorConfig` table with type, default, and description.
4. THE `README.md` SHALL include `levy_exponent` and `max_tumble_steps` in the ActorConfig parameter table with type and description.
