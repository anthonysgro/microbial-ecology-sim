# Requirements Document

## Introduction

This spec covers two phases: (1) removing moisture as a fundamental grid field, and (2) introducing persistent energy sources for heat and chemicals.

Moisture is a second-level emergent property — it does not belong in the grid substrate. The first phase strips all moisture infrastructure from the codebase: the `FieldBuffer`, evaporation system, config fields, viz overlay, and all accessors. The second phase introduces a `SourceRegistry` that stores persistent emitters (heat, chemical) and an emission phase that injects values into field write buffers each tick, creating sustained gradients that drive actor behavior.

## Glossary

- **Grid**: The 2D environment composed of cells, each holding heat and N chemical species values. Owned by the `Grid` struct.
- **FieldBuffer**: A double-buffered contiguous array for a single physical field. Supports read/write separation for deterministic tick updates.
- **Source**: A persistent emitter that injects a value into a specific grid field at a specific cell each tick.
- **Source_Registry**: A collection of all active sources, stored on the Grid. Iterated once per tick during the emission phase.
- **Emission_Rate**: The quantity of a field value (heat units, chemical concentration) injected per tick by a single source.
- **Emission_Phase**: A WARM precomputation step that runs before the HOT parallel update systems (diffusion, heat radiation), injecting source values into write buffers.
- **TickOrchestrator**: The struct that drives per-tick execution sequence, running environmental systems in order with validation and buffer swaps.
- **Chemical_Species**: An index identifying one of the N tracked chemical types in the grid.
- **Cell_Index**: A flat index into the grid's field buffers, derived from (x, y) coordinates.

## Requirements

### Requirement 1: Remove Moisture Field from Grid

**User Story:** As a simulation engineer, I want to remove moisture as a fundamental grid field, so that the grid substrate only contains physically fundamental quantities (heat and chemicals).

#### Acceptance Criteria

1. THE Grid struct SHALL NOT contain a moisture FieldBuffer
2. THE Grid struct SHALL NOT expose `read_moisture`, `write_moisture`, `swap_moisture`, or `read_write_moisture` methods
3. THE Grid struct SHALL NOT expose the `heat_read_moisture_rw` combined accessor
4. THE GridConfig SHALL NOT contain an `evaporation_coefficient` field
5. THE CellDefaults SHALL NOT contain a `moisture` field

### Requirement 2: Remove Evaporation System

**User Story:** As a simulation engineer, I want to remove the evaporation system entirely, so that no code depends on the removed moisture field.

#### Acceptance Criteria

1. THE codebase SHALL NOT contain the `evaporation.rs` module
2. THE `src/grid/mod.rs` module declaration SHALL NOT include `pub mod evaporation`
3. THE TickOrchestrator SHALL NOT call `run_evaporation` or validate/swap moisture buffers
4. THE TickOrchestrator tick sequence SHALL consist of: diffusion → validate → swap chemicals, then heat radiation → validate → swap heat

### Requirement 3: Remove Moisture from Visualization

**User Story:** As a simulation engineer, I want to remove moisture visualization, so that the viz layer compiles without referencing the removed moisture field.

#### Acceptance Criteria

1. THE OverlayMode enum SHALL NOT contain a `Moisture` variant
2. THE renderer SHALL NOT reference `moisture_bg_color` or `grid.read_moisture()`
3. THE input handler SHALL NOT map any key to a moisture overlay
4. THE color module SHALL NOT contain a `moisture_bg_color` function
5. THE stats module tests SHALL NOT reference moisture overlays

### Requirement 4: Remove Moisture from Application Entry Point

**User Story:** As a simulation engineer, I want to remove moisture references from main.rs, so that the application compiles cleanly after moisture removal.

#### Acceptance Criteria

1. THE CellDefaults construction in main.rs SHALL NOT include a `moisture` field
2. THE GridConfig construction in main.rs SHALL NOT include an `evaporation_coefficient` field

### Requirement 5: Source Data Model

**User Story:** As a simulation designer, I want to define energy sources as structured data, so that the system can store and iterate them efficiently without heap allocation in the emission loop.

#### Acceptance Criteria

1. THE Source_Registry SHALL store sources in a contiguous `Vec` pre-allocated at initialization time
2. WHEN a source is created, THE Source_Registry SHALL validate that the source's cell index is within grid bounds
3. WHEN a source is created with a chemical field target, THE Source_Registry SHALL validate that the chemical species index is within the configured range
4. THE Source data model SHALL represent each source as a cell index, a target field discriminant (heat or chemical species index), and an emission rate
5. IF a source is created with a negative emission rate, THEN THE Source_Registry SHALL accept it as a valid drain (negative emission)

### Requirement 6: Heat Sources

**User Story:** As a simulation designer, I want to place persistent heat emitters on the grid, so that localized thermal gradients are sustained across ticks and drive actor behavior.

#### Acceptance Criteria

1. WHEN the emission phase runs, THE Emission_Phase SHALL add each heat source's emission rate to the heat write buffer at the source's cell index
2. WHEN multiple heat sources target the same cell, THE Emission_Phase SHALL apply all of them additively
3. WHEN a heat source has a negative emission rate, THE Emission_Phase SHALL subtract from the heat write buffer (heat sink)

### Requirement 7: Chemical Sources

**User Story:** As a simulation designer, I want to place persistent chemical emitters on the grid, so that nutrient gradients are sustained and actors can forage along concentration gradients.

#### Acceptance Criteria

1. WHEN the emission phase runs, THE Emission_Phase SHALL add each chemical source's emission rate to the corresponding chemical species write buffer at the source's cell index
2. WHEN multiple chemical sources target the same cell and same species, THE Emission_Phase SHALL apply all of them additively
3. WHEN a chemical source has a negative emission rate, THE Emission_Phase SHALL subtract from the chemical write buffer (nutrient drain)

### Requirement 8: Emission Phase Integration

**User Story:** As a simulation engineer, I want the emission phase to execute at the correct point in the tick sequence, so that injected values are processed by downstream systems (diffusion, heat radiation) within the same tick.

#### Acceptance Criteria

1. THE TickOrchestrator SHALL execute the emission phase before all other per-tick systems (diffusion, heat radiation)
2. WHEN the emission phase writes to a field's write buffer, THE TickOrchestrator SHALL copy the read buffer into the write buffer before emission, so that emission adds to the current state rather than to stale or zeroed data
3. WHEN the emission phase completes for a field, THE TickOrchestrator SHALL swap that field's buffers so downstream systems read the post-emission state
4. THE Emission_Phase SHALL iterate the source list sequentially (WARM path classification — not parallelized, runs over a small source list)

### Requirement 9: Numerical Safety

**User Story:** As a simulation engineer, I want emission values to be validated, so that sources cannot introduce NaN or infinity into the simulation.

#### Acceptance Criteria

1. IF the emission phase produces a NaN or infinite value in any write buffer cell, THEN THE TickOrchestrator SHALL return a TickError before swapping buffers
2. THE Emission_Phase SHALL clamp post-emission chemical concentration values to a minimum of zero (concentrations cannot be negative)

### Requirement 10: Source Management API

**User Story:** As a simulation designer, I want to add and remove sources at runtime, so that the simulation environment can evolve over time.

#### Acceptance Criteria

1. WHEN a source is added to the Source_Registry, THE Source_Registry SHALL append it to the source list and return a stable identifier for later removal
2. WHEN a source is removed by identifier, THE Source_Registry SHALL remove it from the source list
3. IF a removal is requested for a nonexistent identifier, THEN THE Source_Registry SHALL return an error
4. THE Source_Registry SHALL provide a method to query the current number of active sources

### Requirement 11: Determinism

**User Story:** As a simulation engineer, I want emission to be fully deterministic, so that replaying a simulation from the same seed and source configuration produces identical results.

#### Acceptance Criteria

1. THE Emission_Phase SHALL process sources in a deterministic order (iteration order of the source list)
2. WHEN sources are added or removed between ticks, THE Source_Registry SHALL maintain a deterministic iteration order for the remaining sources
