# Requirements Document

## Introduction

Non-renewable sources in the simulation deplete and become inert, eventually leaving the grid barren except for the surviving renewable sources. This creates a degenerate endgame where actors either camp on the last renewable source or die. Source respawn cycling addresses this by introducing a cooldown-then-respawn mechanic: when a non-renewable source fully depletes, the system tracks it, waits a configurable number of ticks, then spawns a new source at a random unoccupied location on the grid. This keeps the resource landscape dynamic, rewards exploration, and prevents the simulation from collapsing into a static equilibrium.

## Glossary

- **Source**: A persistent emitter registered in the `SourceRegistry` that injects a value into a grid field each tick. Has a reservoir that may be finite or infinite.
- **Depleted_Source**: A non-renewable source whose reservoir has reached zero. Emits nothing and is skipped during the emission phase.
- **Respawn_Cooldown**: The number of ticks a depleted source must wait before a replacement source is spawned elsewhere on the grid.
- **Respawn_Queue**: A collection of pending respawn entries, each tracking a depleted source's field type, the tick at which it depleted, and the tick at which the replacement should spawn.
- **Occupied_Cell**: A grid cell that currently has at least one active (non-depleted) source of the same field type. Respawned sources avoid occupied cells.
- **SourceRegistry**: The slot-based storage that holds all active sources with generational indexing.
- **SourceFieldConfig**: Per-field-type configuration struct controlling source generation parameters (emission rate, reservoir capacity, renewable fraction, deceleration threshold).
- **RespawnConfig**: Configuration struct controlling respawn behavior (cooldown range, whether respawning is enabled).
- **Grid**: The spatial grid of cells holding persistent field state (heat, chemical concentrations).
- **Tick**: A single discrete simulation step.

## Requirements

### Requirement 1: Detect Source Depletion Events

**User Story:** As a simulation designer, I want the system to detect when a non-renewable source fully depletes, so that the respawn cycle can begin.

#### Acceptance Criteria

1. WHEN a non-renewable source's reservoir reaches zero during the emission phase, THE emission system SHALL record a depletion event containing the source's field type and the current tick number.
2. WHILE a source is renewable, THE emission system SHALL never generate a depletion event for that source.
3. THE emission system SHALL generate at most one depletion event per source (the first tick the reservoir reaches zero).

### Requirement 2: Queue Respawns with Configurable Cooldown

**User Story:** As a simulation designer, I want depleted sources to enter a cooldown queue before respawning, so that there is a period of scarcity that pressures actors to migrate.

#### Acceptance Criteria

1. WHEN a depletion event is recorded, THE Respawn_Queue SHALL create a pending entry with a respawn tick computed as `depletion_tick + cooldown_ticks`.
2. THE cooldown_ticks value SHALL be sampled from a configurable range `[min_cooldown_ticks, max_cooldown_ticks]` using the seeded RNG.
3. THE Respawn_Queue SHALL store entries in a deterministic order based on their scheduled respawn tick.
4. WHILE respawning is disabled in configuration, THE Respawn_Queue SHALL not accept new entries and no respawns SHALL occur.

### Requirement 3: Spawn Replacement Sources

**User Story:** As a simulation designer, I want a new source to appear at a random grid location after the cooldown expires, so that the resource landscape stays dynamic.

#### Acceptance Criteria

1. WHEN the current tick reaches or exceeds a pending entry's scheduled respawn tick, THE respawn system SHALL spawn a new source on the grid.
2. THE respawn system SHALL select a cell that does not already contain an active source of the same field type, using the seeded RNG.
3. IF all cells of the relevant field type are occupied by active sources, THEN THE respawn system SHALL defer the respawn to the next tick and retry.
4. THE respawn system SHALL sample the new source's emission rate from the corresponding SourceFieldConfig range using the seeded RNG.
5. THE respawn system SHALL always create the replacement source as non-renewable, with reservoir capacity sampled from the corresponding SourceFieldConfig range using the seeded RNG.
6. THE respawn system SHALL sample the new source's deceleration threshold from the corresponding SourceFieldConfig range using the seeded RNG.
7. WHEN a replacement source is spawned, THE respawn system SHALL register it in the SourceRegistry and remove the pending entry from the Respawn_Queue.

### Requirement 4: Respawn Configuration

**User Story:** As a simulation designer, I want to configure respawn behavior per field type, so that heat and chemical sources can have independent respawn dynamics.

#### Acceptance Criteria

1. THE SourceFieldConfig SHALL include a boolean field `respawn_enabled` that controls whether depleted sources of that field type trigger respawns.
2. THE SourceFieldConfig SHALL include `min_respawn_cooldown_ticks` and `max_respawn_cooldown_ticks` fields specifying the range for cooldown duration in ticks.
3. WHEN `respawn_enabled` is false, THE respawn system SHALL skip depletion event recording and respawn processing for that field type.
4. THE default value for `respawn_enabled` SHALL be false, preserving backward-compatible behavior.
5. THE default value for `min_respawn_cooldown_ticks` SHALL be 50 and `max_respawn_cooldown_ticks` SHALL be 150.

### Requirement 5: Respawn Configuration Validation

**User Story:** As a simulation designer, I want respawn configuration validated at startup, so that invalid cooldown ranges are caught before the simulation runs.

#### Acceptance Criteria

1. IF `min_respawn_cooldown_ticks > max_respawn_cooldown_ticks`, THEN THE validation system SHALL return an error identifying the field type and range name.
2. IF `respawn_enabled` is true and `max_respawn_cooldown_ticks` is zero, THEN THE validation system SHALL return an error (zero cooldown with respawn enabled is degenerate).
3. WHEN `respawn_enabled` is false, THE validation system SHALL accept any cooldown range values without error (they are unused).

### Requirement 6: Determinism and Hot-Path Compliance

**User Story:** As a simulation engineer, I want source respawning to be fully deterministic and allocation-free in the per-tick path, so that replay fidelity and performance are preserved.

#### Acceptance Criteria

1. THE respawn system SHALL produce identical results for identical initial state, configuration, and tick sequence (deterministic execution).
2. THE Respawn_Queue SHALL be pre-allocated at initialization time and perform zero heap allocations during per-tick processing.
3. THE respawn system SHALL use the simulation's seeded RNG for all random decisions (cooldown sampling, cell selection, source parameter sampling).
4. THE respawn system SHALL process pending entries in deterministic order (by scheduled respawn tick, then by insertion order for ties).

### Requirement 7: Tick Phase Integration

**User Story:** As a simulation engineer, I want respawn processing to execute at the correct point in the tick sequence, so that newly spawned sources participate in emission starting the following tick.

#### Acceptance Criteria

1. THE respawn system SHALL execute after the emission phase within the same tick (so that depletion events from the current emission are captured).
2. WHEN a source is spawned by the respawn system, THE source SHALL begin emitting on the next tick (not the current tick).
3. THE respawn system SHALL be classified as WARM path (runs infrequently over a small queue).

### Requirement 8: Cleanup of Depleted Source Slots

**User Story:** As a simulation engineer, I want depleted source slots to be reclaimed after their respawn entry is queued, so that the SourceRegistry does not accumulate inert slots indefinitely.

#### Acceptance Criteria

1. WHEN a depletion event is recorded and a respawn entry is queued, THE respawn system SHALL remove the depleted source from the SourceRegistry.
2. THE SourceRegistry slot freed by removal SHALL be available for reuse by future source additions.

### Requirement 9: Documentation Update

**User Story:** As a developer, I want configuration documentation updated to reflect the new respawn fields, so that the example config and README stay in sync with the code.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include the new respawn fields (`respawn_enabled`, `min_respawn_cooldown_ticks`, `max_respawn_cooldown_ticks`) with explanatory comments.
2. THE config-documentation steering rule SHALL be updated to include the new fields in the SourceFieldConfig reference table.
3. THE `format_config_info()` in `src/viz_bevy/setup.rs` SHALL display the new respawn configuration fields when the info panel is toggled.
