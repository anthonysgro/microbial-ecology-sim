# Requirements Document

## Introduction

Actor reproduction via binary fission: when an Actor accumulates sufficient energy, it physically splits into two organisms. The parent retains a portion of its energy, and a new offspring Actor is placed in an adjacent empty cell. This is a biologically physical process driven entirely by local energy state and spatial availability — no abstract game mechanics, no global population controllers.

## Glossary

- **Actor**: A mobile biological agent occupying exactly one grid cell, with internal energy reserves and physical components. The atomic unit of simulation.
- **Binary_Fission**: The physical splitting of one Actor into two. The parent Actor divides, producing an offspring Actor in an adjacent cell.
- **Reproduction_Threshold**: The minimum energy level an Actor must reach before binary fission is triggered.
- **Reproduction_Cost**: The total energy deducted from the parent Actor upon fission. Must be greater than or equal to Offspring_Energy.
- **Offspring_Energy**: The energy assigned to the newly created offspring Actor at the moment of fission.
- **Von_Neumann_Neighborhood**: The four orthogonally adjacent cells (North, South, West, East) surrounding a given cell.
- **Occupancy_Map**: A per-cell index mapping that enforces the one-actor-per-cell constraint.
- **Spawn_Buffer**: A pre-allocated buffer collecting deferred Actor insertions, processed after the reproduction scan completes.
- **ActorConfig**: The configuration struct holding all per-tick Actor parameters.
- **ActorRegistry**: Generational slot-based storage for all active Actors.
## Requirements

### Requirement 1: Reproduction Eligibility

**User Story:** As a simulation operator, I want Actors to reproduce only when they have accumulated enough energy, so that reproduction is a meaningful metabolic investment rather than a trivial event.

#### Acceptance Criteria

1. WHEN an Actor's energy is greater than or equal to the Reproduction_Threshold AND the Actor is not inert, THE Reproduction_System SHALL consider that Actor eligible for binary fission.
2. WHEN an Actor's energy is below the Reproduction_Threshold, THE Reproduction_System SHALL skip that Actor without modifying its state.
3. WHEN an Actor is inert, THE Reproduction_System SHALL skip that Actor regardless of its energy level.

### Requirement 2: Offspring Placement

**User Story:** As a simulation operator, I want offspring to be placed in adjacent empty cells using a deterministic scan order, so that reproduction respects spatial constraints and maintains simulation determinism.

#### Acceptance Criteria

1. WHEN an eligible Actor reproduces, THE Reproduction_System SHALL scan the Von_Neumann_Neighborhood in fixed order (North, South, West, East) and select the first unoccupied cell for offspring placement.
2. WHEN all four Von_Neumann_Neighborhood cells are occupied or out of bounds, THE Reproduction_System SHALL block reproduction for that Actor and leave its state unchanged.
3. WHEN the offspring cell is at a grid boundary, THE Reproduction_System SHALL treat out-of-bounds neighbors as unavailable and continue scanning remaining directions.

### Requirement 3: Energy Transfer

**User Story:** As a simulation operator, I want fission to cost the parent a configurable amount of energy and grant the offspring a configurable starting energy, so that reproduction is a significant metabolic event that prevents runaway growth.

#### Acceptance Criteria

1. WHEN binary fission occurs, THE Reproduction_System SHALL deduct Reproduction_Cost from the parent Actor's energy.
2. WHEN binary fission occurs, THE Reproduction_System SHALL assign Offspring_Energy to the new Actor.
3. THE ActorConfig SHALL enforce that Reproduction_Cost is greater than or equal to Offspring_Energy.
4. THE ActorConfig SHALL enforce that Reproduction_Cost is greater than zero.
5. THE ActorConfig SHALL enforce that Offspring_Energy is greater than zero and less than or equal to the Actor's max_energy.

### Requirement 4: Offspring Initial State

**User Story:** As a simulation operator, I want offspring Actors to start in a clean physical state, so that they behave as fresh organisms without inheriting the parent's movement state.

#### Acceptance Criteria

1. WHEN a new offspring Actor is created, THE Reproduction_System SHALL set its tumble_remaining to zero.
2. WHEN a new offspring Actor is created, THE Reproduction_System SHALL set its inert flag to false.
3. WHEN a new offspring Actor is created, THE Reproduction_System SHALL set its cell_index to the selected offspring cell.

### Requirement 5: Deferred Spawning

**User Story:** As a simulation operator, I want offspring creation to be deferred until after the reproduction scan completes, so that newly spawned Actors do not interfere with the current tick's reproduction decisions.

#### Acceptance Criteria

1. WHILE the Reproduction_System iterates over Actors, THE Reproduction_System SHALL collect spawn requests into the Spawn_Buffer without modifying the ActorRegistry.
2. WHEN the reproduction scan completes, THE Reproduction_System SHALL insert all buffered offspring into the ActorRegistry and update the Occupancy_Map.
3. THE Spawn_Buffer SHALL be pre-allocated at grid construction time to avoid heap allocation during tick execution.

### Requirement 6: Tick Phase Ordering

**User Story:** As a simulation operator, I want reproduction to run after metabolism but before movement, so that newly fed Actors can reproduce before relocating.

#### Acceptance Criteria

1. THE Tick_Orchestrator SHALL execute the Reproduction_System after the metabolism phase and deferred removal phase, but before the movement phase.
2. WHEN reproduction spawns new Actors, THE Reproduction_System SHALL update the Occupancy_Map before the movement phase executes.

### Requirement 7: Deterministic Execution

**User Story:** As a simulation operator, I want reproduction to be fully deterministic given the same seed and configuration, so that simulation replays produce identical results.

#### Acceptance Criteria

1. THE Reproduction_System SHALL iterate Actors in ascending slot-index order when evaluating reproduction eligibility.
2. THE Reproduction_System SHALL scan Von_Neumann_Neighborhood directions in the fixed order North, South, West, East for every eligible Actor.
3. WHEN multiple Actors are eligible for reproduction in the same tick, THE Reproduction_System SHALL process them in ascending slot-index order, with each Actor's offspring placement reflecting the occupancy state updated by all previously spawned offspring in that tick.

### Requirement 8: Configuration Validation

**User Story:** As a simulation operator, I want invalid reproduction configuration to be rejected at grid construction time, so that misconfigured simulations fail fast with clear error messages.

#### Acceptance Criteria

1. IF Reproduction_Threshold is less than or equal to zero, THEN THE Grid constructor SHALL return an error.
2. IF Reproduction_Cost is less than or equal to zero, THEN THE Grid constructor SHALL return an error.
3. IF Offspring_Energy is less than or equal to zero, THEN THE Grid constructor SHALL return an error.
4. IF Reproduction_Cost is less than Offspring_Energy, THEN THE Grid constructor SHALL return an error.
5. IF Offspring_Energy is greater than max_energy, THEN THE Grid constructor SHALL return an error.
6. IF Reproduction_Threshold is less than Reproduction_Cost, THEN THE Grid constructor SHALL return an error indicating that the parent would not retain positive energy after fission.

### Requirement 9: Configuration Documentation

**User Story:** As a simulation operator, I want all new reproduction configuration fields documented in the example config, README, and visualization info panel, so that configuration stays in sync with the code.

#### Acceptance Criteria

1. WHEN reproduction configuration fields are added, THE example_config.toml SHALL include the new fields with comments explaining purpose and valid ranges.
2. WHEN reproduction configuration fields are added, THE README.md SHALL document the new fields in the configuration reference.
3. WHEN reproduction configuration fields are added, THE config info panel in viz_bevy/setup.rs SHALL display the new field values.
4. WHEN reproduction configuration fields are added, THE config-documentation.md steering file SHALL be updated with the new fields in the configuration reference table.

