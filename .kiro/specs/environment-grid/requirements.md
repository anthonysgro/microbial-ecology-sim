# Requirements Document

## Introduction

This document specifies the requirements for the Environment Grid — the foundational Layer 0 substrate of the Emergent Sovereignty simulation. The Environment Grid is a headless, high-performance, multi-threaded grid of cells where each cell holds persistent physical state (chemical gradients, heat, and moisture). Environmental processes — chemical diffusion, heat radiation, and moisture evaporation — run as independent systems over this grid. The grid is the active physical medium through which all actors interact with their surroundings.

## Glossary

- **Grid**: A two-dimensional rectangular array of Cells representing the simulated world. The Grid owns all Cell data in contiguous memory.
- **Cell**: The atomic spatial unit of the environment. Each Cell holds persistent physical state: ChemicalGradients, Heat, and Moisture.
- **ChemicalGradients**: A fixed-size collection of chemical concentration values (floating-point) stored per Cell. Each entry represents the concentration of a distinct chemical species (e.g., Nitrogen, Glucose, Oxygen).
- **Heat**: A scalar floating-point value representing the thermal energy stored in a Cell.
- **Moisture**: A scalar floating-point value representing the water content stored in a Cell.
- **Diffusion_System**: The independent system responsible for spreading chemical concentrations between neighboring Cells over time.
- **Heat_System**: The independent system responsible for radiating thermal energy between neighboring Cells over time.
- **Evaporation_System**: The independent system responsible for reducing Moisture in Cells over time based on local Heat.
- **Spatial_Partition**: A subdivision of the Grid into non-overlapping rectangular regions used to distribute work across threads.
- **Double_Buffer**: A pair of Grid-sized data buffers where one is read from while the other is written to, then swapped, to avoid data races during concurrent updates.
- **Tick**: A single discrete time step of the simulation.
- **Neighbor**: A Cell that is orthogonally or diagonally adjacent to a given Cell (8-connectivity).

## Requirements

### Requirement 1: Grid Initialization

**User Story:** As a simulation runner, I want to create an environment grid of configurable dimensions with physically valid initial state, so that the simulation has a well-defined starting substrate.

#### Acceptance Criteria

1. WHEN the simulation is initialized with width and height parameters, THE Grid SHALL allocate a contiguous block of Cell data with exactly width × height Cells.
2. WHEN a Cell is created, THE Cell SHALL contain a ChemicalGradients field, a Heat field, and a Moisture field, all initialized to caller-supplied default values.
3. WHEN the Grid is initialized, THE Grid SHALL store Cell data in a Structure-of-Arrays layout where all ChemicalGradients values are contiguous, all Heat values are contiguous, and all Moisture values are contiguous.
4. IF width or height is zero, THEN THE Grid SHALL return an error indicating invalid dimensions.
5. WHEN the Grid is initialized, THE Grid SHALL allocate a Double_Buffer for each physical field (ChemicalGradients, Heat, Moisture) so that one buffer is readable while the other is writable.

### Requirement 2: Cell State Access

**User Story:** As a simulation system, I want to read and write individual Cell state by coordinate, so that actors and environmental systems can interact with specific locations.

#### Acceptance Criteria

1. WHEN a valid (x, y) coordinate is provided, THE Grid SHALL return a read reference to the Cell state at that position in constant time.
2. WHEN a valid (x, y) coordinate is provided for writing, THE Grid SHALL allow mutation of the Cell state at that position in the write buffer in constant time.
3. IF an (x, y) coordinate is outside the Grid bounds, THEN THE Grid SHALL return an error indicating an out-of-bounds access.
4. WHEN a Cell is accessed by coordinate, THE Grid SHALL compute the storage index as y × width + x using the Structure-of-Arrays layout.

### Requirement 3: Chemical Diffusion

**User Story:** As a simulation runner, I want chemicals to spread between neighboring cells over time, so that chemical gradients form naturally and actors experience a physically realistic chemical environment.

#### Acceptance Criteria

1. WHEN the Diffusion_System executes a Tick, THE Diffusion_System SHALL read chemical concentrations from the read buffer and write updated concentrations to the write buffer for every Cell.
2. WHEN computing diffusion for a Cell, THE Diffusion_System SHALL calculate the net chemical flow from each Neighbor based on the concentration difference between the Cell and that Neighbor, scaled by a configurable diffusion rate.
3. WHEN a Cell is on the Grid boundary, THE Diffusion_System SHALL treat missing Neighbors as having zero concentration (open boundary condition).
4. WHEN the Diffusion_System completes a Tick, THE Diffusion_System SHALL preserve the total sum of each chemical species across all Cells within a floating-point tolerance (conservation of mass).
5. WHEN the Diffusion_System executes, THE Diffusion_System SHALL process Cells using data parallelism by dividing the Grid into Spatial_Partitions and processing each partition on a separate thread.

### Requirement 4: Heat Radiation

**User Story:** As a simulation runner, I want heat to radiate between neighboring cells over time, so that thermal gradients form naturally and drive moisture and metabolic dynamics.

#### Acceptance Criteria

1. WHEN the Heat_System executes a Tick, THE Heat_System SHALL read Heat values from the read buffer and write updated Heat values to the write buffer for every Cell.
2. WHEN computing heat radiation for a Cell, THE Heat_System SHALL calculate the net thermal flow from each Neighbor based on the Heat difference between the Cell and that Neighbor, scaled by a configurable thermal conductivity rate.
3. WHEN a Cell is on the Grid boundary, THE Heat_System SHALL treat missing Neighbors as having a configurable ambient Heat value.
4. WHEN the Heat_System completes a Tick, THE Heat_System SHALL preserve the total Heat across all Cells plus any net heat exchanged with the ambient boundary, within a floating-point tolerance (conservation of energy).
5. WHEN the Heat_System executes, THE Heat_System SHALL process Cells using data parallelism by dividing the Grid into Spatial_Partitions and processing each partition on a separate thread.

### Requirement 5: Moisture Evaporation

**User Story:** As a simulation runner, I want moisture to evaporate from cells based on local heat, so that wet and dry regions emerge naturally from thermal dynamics.

#### Acceptance Criteria

1. WHEN the Evaporation_System executes a Tick, THE Evaporation_System SHALL reduce the Moisture value of each Cell based on that Cell's current Heat value and a configurable evaporation coefficient.
2. WHEN computing evaporation for a Cell, THE Evaporation_System SHALL calculate moisture loss as evaporation_coefficient × Heat × current_Moisture × tick_duration.
3. WHEN evaporation would reduce Moisture below zero, THE Evaporation_System SHALL clamp the Moisture value to zero.
4. WHEN the Evaporation_System executes, THE Evaporation_System SHALL process Cells using data parallelism by dividing the Grid into Spatial_Partitions and processing each partition on a separate thread.

### Requirement 6: Double-Buffer Synchronization

**User Story:** As a simulation runner, I want concurrent read and write access to the grid without data races, so that multiple systems can safely execute in parallel.

#### Acceptance Criteria

1. WHEN a system begins a Tick, THE Grid SHALL provide read-only access to the current read buffer and write access to the current write buffer.
2. WHEN all systems have completed their writes for a Tick, THE Grid SHALL swap the read and write buffers so that the newly written data becomes the read buffer for the next Tick.
3. WHILE a Tick is in progress, THE Grid SHALL prevent any system from writing to the read buffer.
4. WHEN the buffers are swapped, THE Grid SHALL complete the swap without copying Cell data (pointer or index swap only).

### Requirement 7: Spatial Partitioning for Parallelism

**User Story:** As a simulation runner, I want the grid to be spatially partitioned for parallel processing, so that the simulation scales across multiple CPU cores.

#### Acceptance Criteria

1. WHEN the Grid is initialized, THE Grid SHALL divide the Cell space into non-overlapping rectangular Spatial_Partitions that together cover all Cells.
2. WHEN Spatial_Partitions are created, THE Grid SHALL size each partition to balance work evenly across the available number of threads.
3. WHEN a system processes a Spatial_Partition, THE system SHALL read from any Cell in the read buffer (including Neighbors outside the partition boundary) but write only to Cells within its assigned partition in the write buffer.
4. WHEN Spatial_Partitions are assigned to threads, THE Grid SHALL ensure that no two threads write to overlapping Cell ranges.

### Requirement 8: Memory Layout and Performance

**User Story:** As a simulation runner, I want the grid to use cache-friendly memory layouts, so that hot-path iteration over cells is efficient on modern hardware.

#### Acceptance Criteria

1. THE Grid SHALL store each physical field (ChemicalGradients, Heat, Moisture) as a separate contiguous array (Structure-of-Arrays layout).
2. THE Grid SHALL use indexed Vec-based storage for all Cell data, avoiding HashMap or pointer-heavy collections in hot-path iteration.
3. WHEN iterating over Cells for a system Tick, THE Grid SHALL access memory in row-major order to maximize cache line utilization.
4. THE Grid SHALL avoid heap allocation during Tick processing; all per-Tick scratch data SHALL be pre-allocated at Grid initialization.

### Requirement 9: Tick Orchestration

**User Story:** As a simulation runner, I want a single Tick to execute all environmental systems in a defined order, so that the simulation advances deterministically.

#### Acceptance Criteria

1. WHEN a Tick is executed, THE Grid SHALL run the Diffusion_System, Heat_System, and Evaporation_System in a defined sequential order within that Tick.
2. WHEN each system completes within a Tick, THE Grid SHALL swap the Double_Buffer before the next system begins, so that each system reads the output of the previous system.
3. WHEN the same initial state and configuration are provided, THE Grid SHALL produce identical results across repeated runs (deterministic execution).
4. IF a system encounters a numerical error (NaN or infinity in any Cell field), THEN THE Grid SHALL halt the Tick and return an error indicating which system and Cell produced the invalid value.
