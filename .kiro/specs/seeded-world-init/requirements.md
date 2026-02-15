# Requirements Document

## Introduction

The simulation currently hardcodes initial conditions (source positions, emission rates, grid field values) in `main.rs`. This feature introduces a seeded procedural initialization system that generates deterministic initial world states from a seed value. The seed controls only initial conditions — source placement, emission rates, and per-cell field values — without affecting physics parameters, grid dimensions, or tick orchestration. This enables reproducible experimentation and replay of interesting configurations.

## Glossary

- **World_Initializer**: The system responsible for procedurally generating initial grid state and source placement from a seed and configuration parameters.
- **Seed**: A `u64` value used to initialize a deterministic pseudo-random number generator.
- **WorldInitConfig**: A configuration struct specifying the ranges and constraints for procedural generation (source count bounds, emission rate bounds, initial field value ranges).
- **Grid**: The top-level environment grid owning all field buffers and the source registry.
- **Source**: A persistent emitter that injects heat or chemical values into a grid field each tick.
- **SourceRegistry**: The slot-based storage for active sources within the grid.
- **GridConfig**: Immutable configuration for grid dimensions, physics parameters, and threading.
- **FieldBuffer**: Double-buffered contiguous array for a single physical field (heat or chemical species).

## Requirements

### Requirement 1: Deterministic World Generation from Seed

**User Story:** As a simulation operator, I want to generate a complete initial world state from a seed value, so that I can reproduce and share interesting configurations.

#### Acceptance Criteria

1. WHEN the World_Initializer receives a Seed and WorldInitConfig, THE World_Initializer SHALL produce a fully initialized Grid with sources registered and field buffers populated.
2. WHEN the World_Initializer is invoked twice with the same Seed, GridConfig, and WorldInitConfig, THE World_Initializer SHALL produce identical Grid states (same source positions, same emission rates, same per-cell field values).
3. WHEN the World_Initializer is invoked with two different Seed values but identical GridConfig and WorldInitConfig, THE World_Initializer SHALL produce different Grid states with high probability.

### Requirement 2: Seeded Source Placement

**User Story:** As a simulation operator, I want source positions and emission rates to be procedurally determined by the seed, so that each seed produces a unique spatial arrangement of energy emitters.

#### Acceptance Criteria

1. WHEN generating sources, THE World_Initializer SHALL place a number of heat sources within the range specified by WorldInitConfig (min_heat_sources to max_heat_sources inclusive).
2. WHEN generating sources, THE World_Initializer SHALL place a number of chemical sources per species within the range specified by WorldInitConfig (min_chemical_sources to max_chemical_sources inclusive).
3. WHEN placing a source, THE World_Initializer SHALL assign a cell position uniformly sampled from valid grid cell indices (0 to width*height - 1).
4. WHEN placing a source, THE World_Initializer SHALL assign an emission rate uniformly sampled from the range specified by WorldInitConfig (min_emission_rate to max_emission_rate).
5. WHEN placing sources, THE World_Initializer SHALL register each source via the Grid add_source API, propagating any SourceError as a WorldInitError.

### Requirement 3: Seeded Initial Field Values

**User Story:** As a simulation operator, I want initial per-cell heat and chemical concentrations to be procedurally varied by the seed, so that each world starts with a spatially heterogeneous environment.

#### Acceptance Criteria

1. WHEN initializing field values, THE World_Initializer SHALL write an initial heat value to each cell, sampled from the range specified by WorldInitConfig (min_initial_heat to max_initial_heat).
2. WHEN initializing field values, THE World_Initializer SHALL write an initial chemical concentration per species to each cell, sampled from the range specified by WorldInitConfig (min_initial_concentration to max_initial_concentration).
3. WHEN writing initial field values, THE World_Initializer SHALL write directly to the Grid read buffers so that the first tick reads the seeded state.

### Requirement 4: Separation of Seed Scope from Physics

**User Story:** As a simulation operator, I want the seed to control only initial conditions, so that physics behavior remains unchanged and comparable across seeds.

#### Acceptance Criteria

1. THE World_Initializer SHALL accept GridConfig as a read-only input and SHALL NOT modify any physics parameters (diffusion_rate, thermal_conductivity, ambient_heat, tick_duration).
2. THE World_Initializer SHALL accept grid dimensions (width, height) and num_chemicals from GridConfig as read-only constraints for placement bounds and field buffer sizing.
3. WHEN the World_Initializer completes, THE Grid SHALL be in a valid state for immediate tick execution by TickOrchestrator without additional setup.

### Requirement 5: Configuration Validation

**User Story:** As a simulation operator, I want invalid initialization configurations to be rejected with clear errors, so that I can correct misconfiguration before running the simulation.

#### Acceptance Criteria

1. IF WorldInitConfig specifies min_heat_sources greater than max_heat_sources, THEN THE World_Initializer SHALL return a WorldInitError describing the invalid range.
2. IF WorldInitConfig specifies min_chemical_sources greater than max_chemical_sources, THEN THE World_Initializer SHALL return a WorldInitError describing the invalid range.
3. IF WorldInitConfig specifies min_emission_rate greater than max_emission_rate, THEN THE World_Initializer SHALL return a WorldInitError describing the invalid range.
4. IF WorldInitConfig specifies min_initial_heat greater than max_initial_heat, THEN THE World_Initializer SHALL return a WorldInitError describing the invalid range.
5. IF WorldInitConfig specifies min_initial_concentration greater than max_initial_concentration, THEN THE World_Initializer SHALL return a WorldInitError describing the invalid range.

### Requirement 6: Integration with Existing Entry Point

**User Story:** As a simulation operator, I want to provide a seed via the application entry point, so that I can control world generation without modifying source code.

#### Acceptance Criteria

1. WHEN the application starts, THE main function SHALL accept an optional seed parameter (defaulting to a fixed seed if not provided).
2. WHEN the application starts, THE main function SHALL construct a WorldInitConfig with reasonable default ranges and pass it along with the seed to the World_Initializer.
3. WHEN the World_Initializer returns a Grid, THE main function SHALL use that Grid for the simulation tick loop, replacing the current hardcoded initialization.
