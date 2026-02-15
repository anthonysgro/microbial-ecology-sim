# Requirements Document

## Introduction

This specification defines the introduction of mobile Actors into the existing deterministic grid simulation. Actors are reactive biological agents that occupy grid cells, sense local environmental gradients, consume chemical resources, expend energy, move across the substrate, and die when energy is depleted. Actors live entirely within the simulation crate and have no dependency on any rendering framework. All behavior is deterministic and allocation-free in hot paths.

## Glossary

- **Actor**: A mobile biological agent occupying exactly one grid cell, possessing an internal energy reserve and capable of sensing, consuming, and moving.
- **ActorRegistry**: A generational slot-based registry that stores all active Actors in contiguous memory with deterministic iteration order, modeled after the existing SourceRegistry.
- **ActorId**: An opaque generational index identifying a registered Actor, preventing the ABA problem on slot reuse.
- **Grid**: The top-level environment structure owning all field buffers, partitions, the SourceRegistry, and the ActorRegistry.
- **FieldBuffer**: A double-buffered contiguous array providing separate read and write slices for a single physical field.
- **Read_Buffer**: The current-state buffer from which all sensing and gradient reads are performed during a tick.
- **Write_Buffer**: The next-state buffer into which all mutations (chemical consumption, energy changes) are written during a tick.
- **Von_Neumann_Neighborhood**: The four orthogonally adjacent cells (north, south, east, west) of a given cell.
- **Gradient**: The difference in a field value between a neighboring cell and the current cell, used for movement decisions.
- **Occupancy_Map**: A flat Vec<Option<usize>> of length cell_count, mapping each cell index to the slot index of the Actor occupying it, or None if unoccupied. Pre-allocated at grid construction time.
- **Tick**: One discrete simulation time step comprising ordered phases executed sequentially.
- **TickOrchestrator**: The struct responsible for executing all tick phases in deterministic order.
- **Deferred_Removal**: A pattern where Actors marked for death during metabolism are collected into a pre-allocated buffer and removed from the ActorRegistry after iteration completes, avoiding structural mutation during iteration.

## Requirements

### Requirement 1: Actor Data Model

**User Story:** As a simulation engineer, I want a minimal Actor struct with physical state, so that each agent carries only the data needed for v1 behavior.

#### Acceptance Criteria

1. THE Actor struct SHALL contain a cell_index field of type usize identifying the occupied grid cell.
2. THE Actor struct SHALL contain an energy field of type f32 representing the internal energy reserve.
3. THE Actor struct SHALL be a plain data struct with no methods beyond trivial constructors, deriving Debug, Clone, Copy, and PartialEq.
4. THE ActorId struct SHALL contain an index field and a generation field, matching the generational index pattern used by SourceId.

### Requirement 2: Actor Registry

**User Story:** As a simulation engineer, I want a slot-based ActorRegistry with generational indices, so that Actors can be added and removed without allocation during tick execution.

#### Acceptance Criteria

1. THE ActorRegistry SHALL store Actors in a contiguous Vec of slots, each slot holding an Option<Actor> and a generation counter.
2. THE ActorRegistry SHALL maintain a free list of reusable slot indices to avoid linear scans on insertion.
3. THE ActorRegistry SHALL provide an add method that validates cell_index against cell_count and returns an ActorId.
4. THE ActorRegistry SHALL provide a remove method that validates the ActorId generation before clearing the slot and incrementing the generation counter.
5. WHEN iterating active Actors, THE ActorRegistry SHALL yield Actors in deterministic slot-index order.
6. THE ActorRegistry SHALL pre-allocate slot capacity at grid construction time so that no heap allocation occurs during tick execution.
7. THE ActorRegistry SHALL provide len and is_empty methods reporting the count of active Actors.

### Requirement 3: Occupancy Map

**User Story:** As a simulation engineer, I want an occupancy map enforcing one Actor per cell, so that movement conflict resolution is O(1) per lookup.

#### Acceptance Criteria

1. THE Grid SHALL own an Occupancy_Map of length cell_count, pre-allocated at construction time.
2. WHEN an Actor is added to the ActorRegistry, THE Occupancy_Map SHALL mark the Actor's cell_index as occupied with the Actor's slot index.
3. WHEN an Actor is removed from the ActorRegistry, THE Occupancy_Map SHALL mark the Actor's former cell_index as unoccupied.
4. IF an Actor is added to a cell that is already occupied, THEN THE ActorRegistry SHALL return an error and reject the addition.
5. THE Occupancy_Map SHALL be updated atomically with ActorRegistry mutations so that the two structures remain consistent.

### Requirement 4: Tick Phase Integration

**User Story:** As a simulation engineer, I want Actor phases integrated into the tick orchestrator in a defined order, so that determinism and buffer discipline are preserved.

#### Acceptance Criteria

1. THE TickOrchestrator SHALL execute phases in the following order: (1) Source Emission, (2) Actor Sensing, (3) Actor Metabolism, (4) Actor Movement, (5) Chemical Diffusion, (6) Heat Radiation, (7) Buffer Swaps.
2. WHILE Actor phases execute, THE simulation SHALL read only from Read_Buffers and write only to Write_Buffers.
3. WHEN Actor phases complete and before Diffusion begins, THE TickOrchestrator SHALL swap chemical buffers affected by Actor consumption so that Diffusion reads post-consumption state.
4. THE TickOrchestrator SHALL execute all Actor phases sequentially over Actors in deterministic slot-index order.
5. WHEN no Actors are registered, THE TickOrchestrator SHALL skip all Actor phases with zero overhead.

### Requirement 5: Sensing Model

**User Story:** As a simulation engineer, I want Actors to sense local chemical gradients, so that they can make movement decisions based on their immediate environment.

#### Acceptance Criteria

1. WHEN sensing, THE Actor SHALL read chemical concentration values from the Von_Neumann_Neighborhood (four orthogonal neighbors) of its current cell using the Read_Buffer.
2. WHEN a neighbor cell lies outside grid boundaries, THE sensing system SHALL treat the out-of-bounds neighbor as having zero chemical concentration.
3. THE sensing system SHALL compute a Gradient for each valid neighbor as the difference between the neighbor's chemical value and the current cell's chemical value.
4. THE sensing system SHALL select the neighbor with the maximum positive Gradient as the preferred movement target.
5. IF no neighbor has a positive Gradient, THEN THE sensing system SHALL indicate no preferred movement direction (the Actor stays in place).
6. THE sensing system SHALL operate on chemical species index 0 for v1.

### Requirement 6: Movement Rules

**User Story:** As a simulation engineer, I want deterministic Actor movement rules, so that Actors relocate toward chemical gradients without violating simulation invariants.

#### Acceptance Criteria

1. WHEN the sensing system selects a preferred target cell, THE movement system SHALL attempt to move the Actor to that cell.
2. THE movement system SHALL move an Actor at most one cell per tick.
3. IF the target cell is already occupied by another Actor, THEN THE movement system SHALL cancel the move and the Actor SHALL remain in its current cell.
4. WHEN an Actor moves, THE movement system SHALL update the Occupancy_Map to reflect the new cell_index and clear the old cell_index.
5. WHEN an Actor moves, THE movement system SHALL update the Actor's cell_index field in the ActorRegistry.
6. THE movement system SHALL process Actors in deterministic slot-index order, granting movement priority to lower slot indices.
7. WHEN the grid uses bounded (non-wrapping) boundaries, THE movement system SHALL clamp movement targets to valid grid coordinates.

### Requirement 7: Metabolism Model

**User Story:** As a simulation engineer, I want Actors to consume chemical resources and expend energy each tick, so that Actor survival depends on environmental conditions.

#### Acceptance Criteria

1. WHEN the metabolism phase executes, THE metabolism system SHALL subtract a configurable consumption_rate amount of chemical species 0 from the Actor's current cell, writing the result to the Write_Buffer.
2. WHEN the metabolism phase executes, THE metabolism system SHALL add energy to the Actor equal to the consumed chemical amount multiplied by a configurable energy_conversion_factor.
3. WHEN the metabolism phase executes, THE metabolism system SHALL subtract a configurable base_energy_decay from the Actor's energy each tick.
4. IF chemical concentration at the Actor's current cell is less than consumption_rate, THEN THE metabolism system SHALL consume only the available concentration and convert proportionally.
5. THE metabolism system SHALL clamp chemical concentration in the Write_Buffer to a minimum of 0.0 after consumption.
6. WHEN an Actor's energy reaches zero or below after metabolism, THE metabolism system SHALL mark the Actor for deferred removal.

### Requirement 8: Actor Death and Deferred Removal

**User Story:** As a simulation engineer, I want dead Actors removed safely after iteration, so that structural mutation does not occur during the metabolism loop.

#### Acceptance Criteria

1. WHEN the metabolism system marks an Actor for death, THE system SHALL record the ActorId in a pre-allocated removal buffer.
2. WHEN all Actor metabolism iterations complete, THE system SHALL remove all marked Actors from the ActorRegistry and clear their Occupancy_Map entries.
3. THE removal buffer SHALL be pre-allocated at grid construction time with capacity equal to the ActorRegistry slot capacity, so that no heap allocation occurs during tick execution.
4. THE deferred removal process SHALL execute in deterministic order (ascending slot index).

### Requirement 9: Actor Configuration

**User Story:** As a simulation engineer, I want Actor behavior parameters in a configuration struct, so that metabolism and sensing can be tuned without code changes.

#### Acceptance Criteria

1. THE ActorConfig struct SHALL contain a consumption_rate field of type f32.
2. THE ActorConfig struct SHALL contain an energy_conversion_factor field of type f32.
3. THE ActorConfig struct SHALL contain a base_energy_decay field of type f32.
4. THE ActorConfig struct SHALL contain an initial_energy field of type f32 used when spawning new Actors.
5. THE ActorConfig struct SHALL contain an initial_actor_capacity field of type usize specifying pre-allocated registry slot count.

### Requirement 10: Simulation Invariants

**User Story:** As a simulation engineer, I want formally stated invariants, so that correctness can be verified through automated testing.

#### Acceptance Criteria

1. THE simulation SHALL guarantee that no two Actors occupy the same cell at any point during a tick.
2. THE simulation SHALL guarantee that all Actor phase reads come from Read_Buffers and all writes go to Write_Buffers.
3. THE simulation SHALL guarantee that no heap allocation occurs inside any HOT or WARM per-tick Actor phase.
4. THE simulation SHALL guarantee deterministic output given identical initial state and configuration.
5. THE simulation SHALL guarantee that chemical concentrations in Write_Buffers remain non-negative after Actor consumption.
6. IF the ActorRegistry contains zero active Actors, THEN THE TickOrchestrator SHALL produce identical output to the pre-Actor tick sequence.

### Requirement 11: Error Handling

**User Story:** As a simulation engineer, I want Actor operations to return typed errors, so that invalid operations are caught at the API boundary rather than causing panics.

#### Acceptance Criteria

1. THE ActorRegistry SHALL define an ActorError enum using thiserror.
2. WHEN an Actor is added with a cell_index exceeding cell_count, THE ActorRegistry SHALL return an ActorError::CellOutOfBounds error.
3. WHEN an Actor is added to an already-occupied cell, THE ActorRegistry SHALL return an ActorError::CellOccupied error.
4. WHEN a remove operation receives a stale or invalid ActorId, THE ActorRegistry SHALL return an ActorError::InvalidActorId error.
5. IF a numerical error (NaN or infinity) is detected in Actor energy after metabolism, THEN THE TickOrchestrator SHALL return a TickError.

### Requirement 12: Performance Constraints

**User Story:** As a simulation engineer, I want Actor systems to meet the same performance standards as existing grid systems, so that adding Actors does not degrade tick throughput.

#### Acceptance Criteria

1. THE Actor sensing phase SHALL access memory in a cache-friendly pattern by iterating Actors in contiguous slot order and reading contiguous field buffer regions.
2. THE Actor metabolism phase SHALL perform no dynamic dispatch.
3. THE Actor movement phase SHALL resolve occupancy conflicts in O(1) per Actor via the Occupancy_Map.
4. THE ActorRegistry SHALL store Actor data in a contiguous Vec for cache-line-friendly sequential iteration.
