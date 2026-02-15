# Requirements Document

## Introduction

Add TOML-based configuration file support to the simulation engine. Currently all world parameters are hardcoded in the two binary entry points (`src/main.rs` and `src/bin/bevy_viz.rs`), requiring recompilation to change any value. This feature introduces a `--config <path>` CLI flag that loads a TOML file mapping directly to the existing config structs (`GridConfig`, `WorldInitConfig`, `ActorConfig`). Fields omitted from the TOML fall back to compiled defaults, preserving backward compatibility. The seed is part of the TOML file so that a single file fully specifies a reproducible world.

## Glossary

- **Config_Loader**: The module responsible for reading a TOML file from disk, deserializing it into the application's configuration structs, and merging with defaults.
- **CLI_Parser**: The command-line argument parser that extracts the optional `--config <path>` flag and optional positional seed from process arguments.
- **GridConfig**: Struct controlling environment physics parameters (grid dimensions, diffusion, thermal conductivity, decay rates).
- **WorldInitConfig**: Struct controlling procedural generation ranges (source placement, initial field values, actor counts). Contains nested `SourceFieldConfig` sub-structs for heat and chemical sources.
- **ActorConfig**: Struct controlling actor metabolism and lifecycle parameters (consumption, energy, movement cost, removal threshold).
- **TopLevelConfig**: A top-level deserialization struct that aggregates seed, GridConfig, WorldInitConfig, and ActorConfig into a single TOML document.
- **Config_Validator**: The existing `world_init::validate_config` function that checks range invariants on `WorldInitConfig`.
- **Terminal_Binary**: The `src/main.rs` entry point for terminal-mode simulation.
- **Bevy_Binary**: The `src/bin/bevy_viz.rs` entry point for graphical Bevy-mode simulation.

## Requirements

### Requirement 1: TOML Deserialization

**User Story:** As a simulation operator, I want to define world parameters in a TOML file, so that I can change configuration without recompiling.

#### Acceptance Criteria

1. WHEN a valid TOML file is provided, THE Config_Loader SHALL deserialize the file into a TopLevelConfig containing seed, GridConfig, WorldInitConfig, and ActorConfig sections
2. WHEN a TOML field is omitted, THE Config_Loader SHALL use the compiled default value for that field
3. WHEN a TOML file contains an unrecognized key, THE Config_Loader SHALL reject the file and return a descriptive error identifying the unknown key
4. THE Config_Loader SHALL serialize a TopLevelConfig back into a valid TOML string (pretty-printer)
5. FOR ALL valid TopLevelConfig values, deserializing the serialized TOML representation SHALL produce an equivalent TopLevelConfig (round-trip property)

### Requirement 2: Configuration Validation

**User Story:** As a simulation operator, I want invalid configurations to be rejected at load time with clear error messages, so that I do not run a simulation with nonsensical parameters.

#### Acceptance Criteria

1. WHEN a deserialized WorldInitConfig contains a range where min exceeds max, THE Config_Validator SHALL return an error identifying the invalid range
2. WHEN a deserialized GridConfig specifies `chemical_decay_rates` with a length that does not equal `num_chemicals`, THE Config_Loader SHALL return a validation error
3. WHEN a deserialized ActorConfig specifies a `removal_threshold` greater than zero, THE Config_Loader SHALL return a validation error
4. IF the TOML file contains a value of the wrong type for a field, THEN THE Config_Loader SHALL return a descriptive deserialization error

### Requirement 3: CLI Integration

**User Story:** As a simulation operator, I want to pass a `--config <path>` flag on the command line, so that I can select which configuration file to load.

#### Acceptance Criteria

1. WHEN the `--config <path>` flag is provided, THE CLI_Parser SHALL extract the file path and pass it to the Config_Loader
2. WHEN no `--config` flag is provided, THE CLI_Parser SHALL signal that no config file was requested
3. WHEN the `--config` flag is provided but the file does not exist or is unreadable, THE CLI_Parser SHALL report a filesystem error and exit with a non-zero status code
4. THE CLI_Parser SHALL accept an optional positional seed argument that overrides the seed in the TOML file when both are provided

### Requirement 4: Default Fallback Behavior

**User Story:** As a simulation operator, I want the simulation to run with sensible defaults when no config file is provided, so that existing workflows remain unchanged.

#### Acceptance Criteria

1. WHEN no `--config` flag is provided and no positional seed is provided, THE Terminal_Binary SHALL initialize the world using the current hardcoded defaults and seed 42
2. WHEN no `--config` flag is provided and no positional seed is provided, THE Bevy_Binary SHALL initialize the world using the current hardcoded defaults and seed 42
3. WHEN only a positional seed is provided without a config file, THE Terminal_Binary SHALL use the hardcoded defaults with the provided seed
4. WHEN only a positional seed is provided without a config file, THE Bevy_Binary SHALL use the hardcoded defaults with the provided seed

### Requirement 5: Reproducibility

**User Story:** As a simulation operator, I want the same config file to always produce the same world, so that I can share and replay exact simulation setups.

#### Acceptance Criteria

1. WHEN the same TOML file is loaded twice, THE Config_Loader SHALL produce identical TopLevelConfig values both times
2. WHEN two simulations are initialized with identical TopLevelConfig values, THE world_init::initialize function SHALL produce identical Grid states

### Requirement 6: Binary Integration

**User Story:** As a simulation operator, I want both the terminal and Bevy binaries to support the `--config` flag, so that I have a consistent interface regardless of visualization mode.

#### Acceptance Criteria

1. WHEN the Terminal_Binary receives a `--config` flag, THE Terminal_Binary SHALL load the TOML file and use the resulting configuration for world initialization
2. WHEN the Bevy_Binary receives a `--config` flag, THE Bevy_Binary SHALL load the TOML file and use the resulting configuration for world initialization
3. WHEN the Bevy_Binary loads a TOML file, THE Bevy_Binary SHALL apply Bevy-specific defaults (tick_hz, zoom, pan, color_scale_max) for fields not present in the TOML
