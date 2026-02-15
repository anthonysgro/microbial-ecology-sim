# Requirements Document

## Introduction

This feature adds finite reservoirs and renewability semantics to chemical and heat emission sources in the simulation grid. Currently, sources emit at a fixed rate indefinitely, creating a degenerate equilibrium where actors can "park" on a source forever without resource pressure. By introducing depletable reservoirs and emission deceleration, sources become a contested, time-limited resource that drives emergent actor migration and competition.

## Glossary

- **Source**: A persistent emitter registered in the `SourceRegistry` that injects a value into a grid field (heat or chemical) each tick.
- **Reservoir**: The finite quantity of emittable material remaining in a source. Measured in the same units as the field it emits into.
- **Emission_Rate**: The base rate (units per tick) at which a source injects material into its target field.
- **Effective_Emission_Rate**: The actual emission rate for a given tick, after applying deceleration based on reservoir level. Always ≤ `Emission_Rate`.
- **Depletion**: The process by which a source's reservoir decreases each tick by the effective emission amount.
- **Renewability**: A classification of whether a source has an infinite reservoir (renewable) or a finite reservoir (non-renewable).
- **Deceleration_Threshold**: The fraction of initial reservoir capacity below which emission rate begins to decrease. Expressed as a value in [0.0, 1.0].
- **Depleted_Source**: A non-renewable source whose reservoir has reached zero. A depleted source emits nothing.
- **Grid**: The spatial grid of cells holding persistent field state (heat, chemical concentrations).
- **SourceRegistry**: The slot-based storage that holds all active sources with generational indexing.
- **WorldInitConfig**: Configuration struct controlling source placement and parameterization during world initialization.

## Requirements

### Requirement 1: Source Reservoir Model

**User Story:** As a simulation designer, I want sources to have a finite reservoir of emittable material, so that sources can deplete over time and create resource scarcity dynamics.

#### Acceptance Criteria

1. THE Source SHALL store a reservoir value representing the remaining emittable quantity.
2. THE Source SHALL store an initial reservoir capacity representing the total quantity at creation time.
3. WHEN a source is created with a finite reservoir, THE SourceRegistry SHALL validate that the reservoir value is greater than zero.
4. WHEN a source is created with a renewable designation, THE Source SHALL treat its reservoir as infinite and never deplete.

### Requirement 2: Reservoir Depletion During Emission

**User Story:** As a simulation designer, I want source reservoirs to drain as they emit, so that non-renewable sources eventually run out and actors must adapt.

#### Acceptance Criteria

1. WHEN a non-renewable source emits during a tick, THE emission system SHALL subtract the effective emission amount from the source's reservoir.
2. WHEN a non-renewable source's reservoir reaches zero, THE emission system SHALL cease emission from that source.
3. WHEN a non-renewable source's reservoir is less than the effective emission rate for a tick, THE emission system SHALL emit only the remaining reservoir amount and set the reservoir to zero.
4. WHILE a source is renewable, THE emission system SHALL emit at the full emission rate without modifying the reservoir.

### Requirement 3: Emission Deceleration

**User Story:** As a simulation designer, I want emission rate to decrease as a source's reservoir runs low, so that depletion is gradual rather than abrupt.

#### Acceptance Criteria

1. THE Source SHALL store a deceleration threshold as a fraction in [0.0, 1.0] of initial capacity.
2. WHILE a non-renewable source's reservoir is above the deceleration threshold fraction of its initial capacity, THE emission system SHALL emit at the full base emission rate.
3. WHILE a non-renewable source's reservoir is at or below the deceleration threshold fraction of its initial capacity, THE emission system SHALL compute the effective emission rate as `base_rate * (current_reservoir / (threshold * initial_capacity))`.
4. WHEN the deceleration threshold is set to zero, THE emission system SHALL emit at the full base rate until the reservoir is exhausted (no deceleration).

### Requirement 4: Depleted Source Handling

**User Story:** As a simulation designer, I want depleted sources to be clearly identifiable and skipped during emission, so that the simulation does not waste cycles on inert sources.

#### Acceptance Criteria

1. WHEN a non-renewable source's reservoir reaches zero, THE Source SHALL be marked as depleted.
2. WHILE a source is depleted, THE emission system SHALL skip that source during the emission phase.
3. THE SourceRegistry SHALL provide a method to query whether a source is depleted.
4. THE SourceRegistry SHALL provide a count of active (non-depleted) sources.

### Requirement 5: World Initialization Configuration

**User Story:** As a simulation designer, I want to configure reservoir sizes and renewability during world initialization, so that I can control the resource dynamics of each simulation run.

#### Acceptance Criteria

1. THE WorldInitConfig SHALL include a range for initial reservoir capacity of non-renewable sources.
2. THE WorldInitConfig SHALL include a parameter for the fraction of sources that are renewable (value in [0.0, 1.0]).
3. THE WorldInitConfig SHALL include a range for the deceleration threshold of non-renewable sources.
4. WHEN generating sources during initialization, THE initialization system SHALL assign each source as renewable or non-renewable based on the configured renewable fraction, using the seeded RNG.
5. WHEN generating a non-renewable source, THE initialization system SHALL sample the reservoir capacity from the configured range using the seeded RNG.
6. WHEN generating a non-renewable source, THE initialization system SHALL sample the deceleration threshold from the configured range using the seeded RNG.

### Requirement 6: Determinism and Hot-Path Compliance

**User Story:** As a simulation engineer, I want source depletion to be fully deterministic and allocation-free in the emission hot path, so that replay fidelity and performance are preserved.

#### Acceptance Criteria

1. THE emission system SHALL produce identical results for identical initial state and tick sequence (deterministic execution).
2. THE emission system SHALL perform zero heap allocations during the emission phase.
3. THE emission system SHALL iterate sources in deterministic slot order during depletion updates.
4. THE Source depletion state SHALL be fully reconstructable from the initial configuration, seed, and tick number.

### Requirement 7: Serialization Round-Trip for Source State

**User Story:** As a simulation engineer, I want source reservoir state to serialize and deserialize correctly, so that simulation snapshots preserve depletion progress.

#### Acceptance Criteria

1. THE Source SHALL serialize its reservoir, initial capacity, renewability, and deceleration threshold fields.
2. FOR ALL valid Source instances, serializing then deserializing SHALL produce an equivalent Source (round-trip property).
