# Requirements Document

## Introduction

This feature extends the Bevy visualization HUD with two additions: a per-tick and cumulative predation event counter displayed in the stats header line, and actor energy population statistics (min, p25, p50, p75, max, mean) displayed alongside the existing heritable trait stats panel. Both additions are COLD-path visualization concerns and do not affect simulation determinism.

## Glossary

- **HUD**: The heads-up display overlay rendered by the Bevy visualization layer.
- **Stats_Panel**: The toggleable text panel (activated by `S` key) that displays population-level heritable trait statistics.
- **Stats_Header**: The first line of the Stats_Panel showing tick number and actor count.
- **Predation_Event**: A single successful contact predation where one actor consumes an adjacent actor, transferring energy and marking the prey inert. Corresponds to one entry in the `events` SmallVec inside `run_contact_predation`.
- **Predation_Counter**: A Bevy resource tracking per-tick predation count and cumulative total across all ticks.
- **Energy_Stats**: Population-level descriptive statistics (min, p25, p50, p75, max, mean) computed from the `energy` field of all non-inert actors.
- **SingleTraitStats**: The existing struct holding min, max, mean, p25, p50, p75 for a single trait dimension.
- **TraitStats**: The existing Bevy resource holding per-tick population statistics for heritable traits.
- **TickOrchestrator**: The struct in `src/grid/tick.rs` that drives the simulation tick phases.
- **ActorRegistry**: The registry holding all actor data, iterated for stats computation.

## Requirements

### Requirement 1: Predation Count Propagation

**User Story:** As a simulation observer, I want the predation system to report how many predation events occurred each tick, so that the visualization layer can display this information.

#### Acceptance Criteria

1. WHEN `run_contact_predation` completes successfully, THE Predation_Counter system SHALL return the number of Predation_Events that occurred during that invocation as part of its `Ok` result.
2. WHEN `run_actor_phases` completes successfully, THE TickOrchestrator SHALL propagate the predation count from `run_contact_predation` through its return value.
3. WHEN `TickOrchestrator::step` completes successfully, THE TickOrchestrator SHALL propagate the predation count from `run_actor_phases` through its return value.
4. WHEN no actors exist in the simulation, THE TickOrchestrator SHALL report zero predation events for that tick.

### Requirement 2: Predation Counter Resource

**User Story:** As a simulation observer, I want a persistent counter tracking predation events, so that I can see both per-tick and cumulative predation totals.

#### Acceptance Criteria

1. THE Predation_Counter SHALL store the number of Predation_Events from the most recent tick.
2. THE Predation_Counter SHALL store the cumulative total of all Predation_Events across all ticks since simulation start.
3. WHEN a new tick completes, THE Predation_Counter SHALL update the per-tick count to the value returned by the tick orchestrator and add that value to the cumulative total.
4. WHEN the simulation is paused, THE Predation_Counter SHALL retain its current per-tick and cumulative values without modification.

### Requirement 3: Predation Counter HUD Display

**User Story:** As a simulation observer, I want to see predation counts in the stats panel header, so that I can monitor predation activity at a glance.

#### Acceptance Criteria

1. WHEN the Stats_Panel is visible and actors are present, THE Stats_Header SHALL display the per-tick predation count and cumulative total alongside the existing tick and actor count (format: `Tick: N  |  Actors: N  |  Predations: N (total: N)`).
2. WHEN the Stats_Panel is visible and no actors are present, THE Stats_Header SHALL display zero for both predation values.

### Requirement 4: Actor Energy Population Statistics

**User Story:** As a simulation observer, I want to see population-level energy statistics in the trait stats panel, so that I can monitor the energy distribution of the actor population.

#### Acceptance Criteria

1. WHEN computing trait statistics, THE Stats_Panel system SHALL collect the `energy` value from every non-inert actor in the ActorRegistry.
2. WHEN at least one non-inert actor exists, THE Stats_Panel system SHALL compute Energy_Stats (min, p25, p50, p75, max, mean) using the same algorithm as heritable trait statistics.
3. WHEN no non-inert actors exist, THE Stats_Panel system SHALL omit Energy_Stats from the display.
4. WHEN the Stats_Panel is visible and Energy_Stats are available, THE Stats_Panel system SHALL display the energy row in the same tabular format as heritable trait rows, labeled "energy".

### Requirement 5: Documentation Update

**User Story:** As a developer, I want the configuration documentation to remain accurate after this change, so that the steering files reflect the current state of the codebase.

#### Acceptance Criteria

1. WHEN the Predation_Counter resource is added, THE config-documentation steering file SHALL be updated to document any new Bevy resource or config field introduced by this feature.
2. WHEN Energy_Stats are added to TraitStats, THE config-documentation steering file SHALL be updated to reflect the new array size or struct changes.
