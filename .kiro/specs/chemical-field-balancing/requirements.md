# Requirements Document

## Introduction

The simulation's chemical field grows without bound because sources emit 0.1–5.0 units/tick while actors consume only 0.1 units/tick, and diffusion redistributes mass without removing it. This feature introduces two mechanisms to restore equilibrium: per-species chemical decay (natural degradation each tick) and rebalanced actor consumption rates so that actors function as meaningful chemical sinks.

## Glossary

- **Grid**: The top-level environment structure owning all field buffers, sources, and actors.
- **GridConfig**: Immutable configuration struct provided at Grid construction time, holding grid dimensions, diffusion rate, thermal conductivity, and tick parameters.
- **ActorConfig**: Immutable configuration struct for actor metabolism parameters (consumption rate, energy conversion, basal decay, initial energy).
- **Chemical_Field**: A double-buffered contiguous array (`FieldBuffer<f32>`) storing per-cell concentration values for a single chemical species.
- **Decay_Rate**: A per-species fractional rate in the range [0.0, 1.0] representing the proportion of chemical concentration removed each tick due to natural degradation.
- **Decay_System**: A stateless system function that applies exponential decay (`concentration *= (1.0 - decay_rate)`) to every cell in every chemical species buffer each tick.
- **TickOrchestrator**: The per-tick execution driver that runs all environmental systems in deterministic order with validation and buffer swaps.
- **Consumption_Rate**: The `ActorConfig.consumption_rate` field specifying chemical units consumed per tick per actor from the actor's current cell.

## Requirements

### Requirement 1: Per-Species Chemical Decay Configuration

**User Story:** As a simulation operator, I want to configure a decay rate for each chemical species, so that chemical concentrations naturally degrade over time and do not grow without bound.

#### Acceptance Criteria

1. THE GridConfig SHALL include a `chemical_decay_rates` field storing one decay rate per chemical species as a contiguous `Vec<f32>`.
2. WHEN a GridConfig is constructed, THE GridConfig SHALL validate that the length of `chemical_decay_rates` equals `num_chemicals`.
3. WHEN a GridConfig is constructed, THE GridConfig SHALL validate that each decay rate is in the range [0.0, 1.0].
4. IF a decay rate is outside the range [0.0, 1.0], THEN THE Grid SHALL return a configuration error and refuse construction.
5. WHEN `chemical_decay_rates` contains all zeros, THE Decay_System SHALL leave all chemical concentrations unchanged (no-op behavior).

### Requirement 2: Chemical Decay System Execution

**User Story:** As a simulation operator, I want chemical concentrations to decay each tick after diffusion, so that the field reaches a natural equilibrium between emission, consumption, diffusion, and decay.

#### Acceptance Criteria

1. THE Decay_System SHALL multiply each cell's chemical concentration by `(1.0 - decay_rate)` for the corresponding species, where `decay_rate` is read from `GridConfig.chemical_decay_rates`.
2. THE TickOrchestrator SHALL execute the Decay_System after the diffusion phase and before the heat phase in the per-tick system sequence.
3. THE Decay_System SHALL operate on the write buffer, validate for NaN and infinity, and swap chemical buffers after completion, following the same double-buffer discipline as diffusion and heat systems.
4. THE Decay_System SHALL process all chemical species in species-index order for deterministic execution.
5. WHEN the decay rate for a species is 0.0, THE Decay_System SHALL skip that species without modifying the buffer.
6. THE Decay_System SHALL clamp resulting concentrations to a minimum of 0.0 to prevent negative values from floating-point rounding.

### Requirement 3: Rebalanced Actor Consumption Defaults

**User Story:** As a simulation operator, I want the default actor consumption rate to be high enough that a cluster of actors near a source can deplete it, so that actors function as meaningful chemical sinks.

#### Acceptance Criteria

1. THE ActorConfig SHALL use a default `consumption_rate` of 1.5 units per tick, replacing the previous default of 0.1.
2. WHEN an actor consumes chemicals, THE actor metabolism system SHALL consume `min(consumption_rate, available_concentration)` from the actor's current cell, preserving the existing partial-consumption behavior.
3. THE main binary and bevy_viz binary SHALL construct ActorConfig with the updated default `consumption_rate` of 1.5.

### Requirement 4: Equilibrium Behavior

**User Story:** As a simulation operator, I want the combined effect of decay and increased consumption to produce a bounded steady-state chemical field, so that the simulation remains dynamic and actors continue to exhibit gradient-following behavior.

#### Acceptance Criteria

1. WHEN sources emit at a constant rate and decay is active, THE Chemical_Field SHALL converge toward a bounded steady-state concentration where emission equals decay plus consumption losses.
2. WHEN decay is active, THE Chemical_Field SHALL maintain non-zero spatial gradients around sources, preserving meaningful actor chemotaxis behavior.
3. THE Decay_System SHALL preserve total mass conservation within floating-point tolerance: the total mass removed per tick per species SHALL equal the sum of `concentration * decay_rate` across all cells for that species.
