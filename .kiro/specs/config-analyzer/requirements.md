# Requirements Document

## Introduction

A standalone CLI binary (`config-analyzer`) that statically analyzes a simulation configuration TOML file and produces a human-readable characterization of the expected simulation dynamics — without running the simulation. The tool reuses the existing `WorldConfig` parsing and validation infrastructure from `src/io/config_file.rs` and outputs a formatted text report to stdout. This is a COLD path tool: it runs once, allocations and dynamic dispatch are permitted.

## Glossary

- **Analyzer**: The `config-analyzer` binary that reads a TOML configuration file and produces a static analysis report.
- **WorldConfig**: The top-level configuration struct (`WorldConfig` in `src/io/config_file.rs`) aggregating seed, grid, world_init, and actor sub-configs.
- **GridConfig**: Configuration for the environment grid (dimensions, diffusion, decay, tick duration).
- **ActorConfig**: Configuration for actor metabolism, movement, reproduction, and heritable traits.
- **SourceFieldConfig**: Configuration for procedural source generation (count, emission rate, reservoir, respawn).
- **Report**: The formatted text output produced by the Analyzer, printed to stdout.
- **Chemical_Budget**: The per-tick balance of chemical entering the system (from sources) versus chemical leaving (via decay and actor consumption).
- **Energy_Budget**: The per-tick net energy gain or loss for a single actor given average chemical availability.
- **Carrying_Capacity**: An estimate of the maximum number of actors the grid can sustain at steady state given total chemical input versus per-actor consumption.
- **Break_Even_Concentration**: The minimum per-cell chemical concentration at which an actor's energy gain from consumption equals its energy loss from basal decay and movement cost.
- **Diffusion_Number**: The dimensionless stability parameter `diffusion_rate * tick_duration * 8`, which must be less than 1.0 for numerical stability of the discrete Laplacian.

## Requirements

### Requirement 1: Configuration Loading and Validation

**User Story:** As a simulation developer, I want the Analyzer to load and validate a TOML configuration file using the existing parsing infrastructure, so that I can trust the analysis is based on a valid configuration.

#### Acceptance Criteria

1. WHEN the Analyzer is invoked with a `--config <path>` argument, THE Analyzer SHALL load the TOML file at the specified path using the existing `load_world_config` function.
2. WHEN the TOML file is missing or unreadable, THE Analyzer SHALL print a descriptive error message to stderr and exit with a non-zero exit code.
3. WHEN the TOML file contains invalid syntax or unknown fields, THE Analyzer SHALL print the parse error to stderr and exit with a non-zero exit code.
4. WHEN the loaded configuration fails cross-field validation via `validate_world_config`, THE Analyzer SHALL print the validation error to stderr and exit with a non-zero exit code.
5. WHEN no `--config` argument is provided, THE Analyzer SHALL print a usage message to stderr and exit with a non-zero exit code.

### Requirement 2: Numerical Stability Analysis

**User Story:** As a simulation developer, I want the Analyzer to check numerical stability constraints, so that I can detect unstable configurations before running the simulation.

#### Acceptance Criteria

1. THE Analyzer SHALL compute the Diffusion_Number as `diffusion_rate * tick_duration * 8` and include the value in the Report.
2. THE Analyzer SHALL compute the thermal stability number as `thermal_conductivity * tick_duration * 8` and include the value in the Report.
3. WHEN the Diffusion_Number is greater than or equal to 1.0, THE Analyzer SHALL emit a warning in the Report indicating the diffusion is numerically unstable.
4. WHEN the thermal stability number is greater than or equal to 1.0, THE Analyzer SHALL emit a warning in the Report indicating the thermal diffusion is numerically unstable.
5. WHEN all stability numbers are below 1.0, THE Analyzer SHALL emit a confirmation that the configuration is numerically stable.

### Requirement 3: Chemical Budget Analysis

**User Story:** As a simulation developer, I want the Analyzer to estimate the chemical budget, so that I can understand whether sources produce enough chemical to sustain actors.

#### Acceptance Criteria

1. THE Analyzer SHALL compute the expected total chemical input per tick as the midpoint source count multiplied by the midpoint emission rate, using the chemical SourceFieldConfig ranges.
2. THE Analyzer SHALL compute the expected total chemical decay per tick as the total grid cell count multiplied by an estimated average concentration multiplied by the chemical decay rate.
3. THE Analyzer SHALL compute the expected total actor consumption per tick as the midpoint initial actor count multiplied by the configured consumption_rate.
4. THE Analyzer SHALL compute the net chemical balance per tick as total input minus total decay minus total actor consumption, and include the value in the Report.
5. WHEN the net chemical balance is negative, THE Analyzer SHALL note in the Report that the system is in chemical deficit and sources are being out-consumed.
6. WHEN no ActorConfig is present, THE Analyzer SHALL compute the chemical budget without actor consumption and note that actors are disabled.

### Requirement 4: Energy Budget Per Actor

**User Story:** As a simulation developer, I want the Analyzer to estimate per-actor energy dynamics, so that I can understand whether actors can survive and reproduce under the configured parameters.

#### Acceptance Criteria

1. WHEN an ActorConfig is present, THE Analyzer SHALL compute the net energy per tick for an actor at the average chemical concentration as `consumption_rate * min(available_concentration, consumption_rate) * (energy_conversion_factor - extraction_cost) - base_energy_decay - base_movement_cost`.
2. THE Analyzer SHALL compute the Break_Even_Concentration as the minimum per-cell concentration at which net energy per tick equals zero, and include the value in the Report.
3. THE Analyzer SHALL compute the idle survival time as `initial_energy / base_energy_decay` ticks and include the value in the Report.
4. THE Analyzer SHALL compute the estimated ticks to reach reproduction_threshold from initial_energy given the net energy per tick, and include the value in the Report.
5. WHEN the net energy per tick is negative at the estimated average concentration, THE Analyzer SHALL note in the Report that actors are expected to lose energy under average conditions.
6. WHEN no ActorConfig is present, THE Analyzer SHALL skip the energy budget section and note that actors are disabled.

### Requirement 5: Population Carrying Capacity Estimate

**User Story:** As a simulation developer, I want the Analyzer to estimate carrying capacity, so that I can understand the population ceiling the grid can sustain.

#### Acceptance Criteria

1. WHEN an ActorConfig is present, THE Analyzer SHALL compute the Carrying_Capacity as the total chemical input per tick divided by the per-actor consumption per tick, and include the value in the Report.
2. THE Analyzer SHALL compare the Carrying_Capacity to the grid cell count and note whether the grid is space-limited or resource-limited.
3. WHEN no ActorConfig is present, THE Analyzer SHALL skip the carrying capacity section and note that actors are disabled.

### Requirement 6: Source Density and Coverage Analysis

**User Story:** As a simulation developer, I want the Analyzer to characterize source density and spatial coverage, so that I can understand how resources are distributed across the grid.

#### Acceptance Criteria

1. THE Analyzer SHALL compute the expected chemical source density as the midpoint source count divided by the total grid cell count, and include the value in the Report.
2. THE Analyzer SHALL compute the expected heat source density as the midpoint heat source count divided by the total grid cell count, and include the value in the Report.
3. THE Analyzer SHALL report the fraction of sources expected to be renewable versus non-renewable for both chemical and heat sources.
4. WHEN non-renewable sources have respawn enabled, THE Analyzer SHALL report the expected respawn cooldown range.
5. WHEN non-renewable sources do not have respawn enabled, THE Analyzer SHALL note that depleted sources are permanent.

### Requirement 7: Diffusion Characterization

**User Story:** As a simulation developer, I want the Analyzer to characterize how quickly chemicals and heat spread, so that I can understand the effective range of sources.

#### Acceptance Criteria

1. THE Analyzer SHALL compute the effective diffusion length scale as `sqrt(diffusion_rate * tick_duration)` and include the value in the Report as an approximate per-tick spread distance in cells.
2. THE Analyzer SHALL compute the effective thermal diffusion length scale as `sqrt(thermal_conductivity * tick_duration)` and include the value in the Report.
3. THE Analyzer SHALL estimate the number of ticks for a chemical source to influence a cell N cells away, using the diffusion length scale, for N = 5 and N = 10.
4. THE Analyzer SHALL report the chemical decay half-life in ticks as `ln(2) / decay_rate` for each chemical species.

### Requirement 8: Report Formatting

**User Story:** As a simulation developer, I want the Report to be clearly formatted and easy to read, so that I can quickly understand the simulation dynamics.

#### Acceptance Criteria

1. THE Analyzer SHALL organize the Report into labeled sections: Numerical Stability, Chemical Budget, Energy Budget, Carrying Capacity, Source Density, and Diffusion Characterization.
2. THE Analyzer SHALL print the Report to stdout as plain text with section headers, aligned values, and units where applicable.
3. THE Analyzer SHALL include a summary header showing the grid dimensions, seed, tick duration, and whether actors are enabled.
4. WHEN a section contains warnings, THE Analyzer SHALL prefix warning lines with `[WARN]` for visual distinction.
5. WHEN a section contains confirmations of healthy parameters, THE Analyzer SHALL prefix those lines with `[OK]`.

### Requirement 9: Binary Entry Point

**User Story:** As a simulation developer, I want the Analyzer to be a separate Cargo binary target, so that I can run it independently without pulling in Bevy or visualization dependencies.

#### Acceptance Criteria

1. THE Analyzer SHALL be defined as a separate `[[bin]]` target in `Cargo.toml` with the name `config-analyzer`.
2. THE Analyzer SHALL depend only on the library crate's `io` and `grid` modules, and SHALL NOT depend on the `viz_bevy` module or the `bevy` crate.
3. THE Analyzer SHALL use `anyhow` for top-level error handling in its `main` function, consistent with the project's application boundary convention.
4. THE Analyzer SHALL accept `--config <path>` as its sole required argument.
