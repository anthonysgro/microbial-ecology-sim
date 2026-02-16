# Requirements Document

## Introduction

The simulation currently treats chemical species inconsistently: source configuration is shared across all species (single `chemical_source_config`), decay rates are a flat `Vec<f32>` in `GridConfig`, and diffusion uses a single scalar `diffusion_rate` shared by all species. This feature introduces a unified `ChemicalSpeciesConfig` bundle that gives each chemical species (indexed `0..num_chemicals`) its own independent source configuration, decay rate, and diffusion rate. The default `num_chemicals` changes from 1 to 2 to encourage multi-species environments out of the box. Actors continue to interact exclusively with chemical species 0 — no actor behavior changes. No backwards compatibility with the old config format is required.

## Glossary

- **GridConfig**: The immutable configuration struct for the environment grid, including dimensions, thread count, tick duration, and thermal conductivity.
- **WorldInitConfig**: The configuration struct governing procedural world generation, including source placement, initial field values, and actor seeding.
- **ChemicalSpeciesConfig**: A new configuration struct bundling source generation parameters, decay rate, and diffusion rate for a single chemical species.
- **SourceFieldConfig**: A reusable configuration struct defining source generation parameters (count, emission rate, reservoir, clustering, respawn) for a single field type.
- **Chemical_Species**: A distinct chemical tracked per grid cell, indexed `0..num_chemicals`.
- **Config_Parser**: The TOML deserialization and validation layer in `src/io/config_file.rs`.
- **Source_Generator**: The `generate_sources` function in `src/grid/world_init.rs` that places initial sources on the grid.
- **Emission_System**: The `run_emission` and `run_emission_phase` functions that inject source values into field write buffers each tick.
- **Respawn_System**: The `run_respawn_phase` function that spawns replacement sources for depleted non-renewable sources.
- **Diffusion_System**: The `run_diffusion` function in `src/grid/diffusion.rs` that computes discrete Laplacian chemical spread each tick (HOT path).
- **Decay_System**: The `run_decay` function in `src/grid/decay.rs` that applies exponential decay to chemical concentrations each tick (HOT path).
- **Tick_Orchestrator**: The `TickOrchestrator::step` function that drives per-tick execution.
- **Info_Panel**: The Bevy visualization overlay displaying active configuration values, toggled by pressing `I`.

## Requirements

### Requirement 1: ChemicalSpeciesConfig Data Model

**User Story:** As a simulation designer, I want each chemical species to have its own unified configuration bundle (source config, decay rate, diffusion rate), so that I can independently tune the spatial distribution, persistence, and spread behavior of each chemical.

#### Acceptance Criteria

1. THE WorldInitConfig SHALL contain a `chemical_species_configs` field holding one ChemicalSpeciesConfig per Chemical_Species, replacing the single `chemical_source_config` field.
2. EACH ChemicalSpeciesConfig SHALL contain a `source_config` field of type SourceFieldConfig, a `decay_rate` field of type f32, and a `diffusion_rate` field of type f32.
3. THE GridConfig SHALL remove the `diffusion_rate` field and the `chemical_decay_rates` field, as these are now per-species in ChemicalSpeciesConfig.
4. WHEN WorldInitConfig uses its compiled default, THE WorldInitConfig SHALL produce a `chemical_species_configs` vector with exactly two entries (matching `num_chemicals = 2` default), where species 0 uses `max_sources = 3`, `decay_rate = 0.05`, `diffusion_rate = 0.05`, and species 1 uses the same defaults.
5. THE GridConfig SHALL default `num_chemicals` to 2.

### Requirement 2: TOML Configuration Format

**User Story:** As a simulation operator, I want to specify per-species chemical configurations in TOML using an array of tables, so that the config file clearly maps each entry to a chemical species by position.

#### Acceptance Criteria

1. THE Config_Parser SHALL deserialize `[[world_init.chemical_species_configs]]` as an ordered array of ChemicalSpeciesConfig entries, where the i-th entry configures Chemical_Species i.
2. WHEN the TOML file omits the `[[world_init.chemical_species_configs]]` section entirely, THE Config_Parser SHALL fall back to the compiled default (two entries with default ChemicalSpeciesConfig values).
3. THE Config_Parser SHALL reject TOML files containing the old `[world_init.chemical_source_config]` key, the old `chemical_decay_rates` key in `[grid]`, or the old `diffusion_rate` key in `[grid]` as unknown fields.

### Requirement 3: Configuration Validation

**User Story:** As a simulation operator, I want the system to validate that the number of chemical species config entries matches `num_chemicals` and that each entry's fields are valid, so that misconfigured files are caught at startup.

#### Acceptance Criteria

1. WHEN `chemical_species_configs.len()` does not equal `grid.num_chemicals`, THEN THE Config_Parser SHALL return a validation error stating the mismatch with both values.
2. WHEN any individual ChemicalSpeciesConfig entry fails field-level validation (source config ranges, decay rate bounds, diffusion rate bounds), THEN THE Config_Parser SHALL return a validation error identifying the species index and the specific field violation.
3. THE Config_Parser SHALL validate each entry's `source_config` using the same field-level rules applied to `heat_source_config`.
4. THE Config_Parser SHALL validate that each entry's `decay_rate` is in the range [0.0, 1.0].
5. THE Config_Parser SHALL validate that each entry's `diffusion_rate` is non-negative and finite.

### Requirement 4: Source Generation

**User Story:** As a simulation designer, I want `generate_sources` to use each species' own SourceFieldConfig when placing initial chemical sources, so that species-specific spatial distributions and emission parameters take effect at world initialization.

#### Acceptance Criteria

1. WHEN generating chemical sources for Chemical_Species i, THE Source_Generator SHALL use `chemical_species_configs[i].source_config` for source count, emission rate, reservoir, clustering, and all other SourceFieldConfig parameters.
2. WHEN `chemical_species_configs[i].source_config.source_clustering > 0.0`, THE Source_Generator SHALL compute a per-species cluster center independently for Chemical_Species i.

### Requirement 5: Emission and Respawn Integration

**User Story:** As a simulation designer, I want the emission and respawn systems to use per-species source configurations, so that depletion cooldowns and replacement source parameters are species-specific.

#### Acceptance Criteria

1. WHEN a Chemical_Species i source depletes and `chemical_species_configs[i].source_config.respawn_enabled` is true, THE Emission_System SHALL sample the respawn cooldown from `chemical_species_configs[i].source_config`'s cooldown range.
2. WHEN spawning a replacement source for Chemical_Species i, THE Respawn_System SHALL sample emission rate, reservoir capacity, and deceleration threshold from `chemical_species_configs[i].source_config`.
3. WHEN spawning a replacement source for Chemical_Species i with `chemical_species_configs[i].source_config.source_clustering > 0.0`, THE Respawn_System SHALL use the stored cluster center for that species.

### Requirement 6: Per-Species Diffusion

**User Story:** As a simulation designer, I want each chemical species to diffuse at its own rate, so that I can model chemicals with different physical properties (e.g., a fast-spreading signal vs. a slow-spreading nutrient).

#### Acceptance Criteria

1. THE Diffusion_System SHALL use `chemical_species_configs[i].diffusion_rate` when computing the discrete Laplacian for Chemical_Species i, instead of a single shared diffusion rate.
2. THE Diffusion_System SHALL accept a slice of per-species diffusion rates (`&[f32]`) to avoid indexing into config structs in the HOT path.
3. WHEN `chemical_species_configs[i].diffusion_rate` is 0.0, THE Diffusion_System SHALL skip diffusion for Chemical_Species i entirely (no read, no write, no cost).

### Requirement 7: Per-Species Decay

**User Story:** As a simulation designer, I want each chemical species to decay at its own rate sourced from the per-species config bundle, so that decay configuration is co-located with the other species parameters.

#### Acceptance Criteria

1. THE Decay_System SHALL use `chemical_species_configs[i].decay_rate` when computing exponential decay for Chemical_Species i.
2. THE Decay_System SHALL accept a slice of per-species decay rates (`&[f32]`) to avoid indexing into config structs in the HOT path.
3. WHEN `chemical_species_configs[i].decay_rate` is 0.0, THE Decay_System SHALL skip decay for Chemical_Species i entirely (existing behavior preserved).

### Requirement 8: Tick Orchestrator Signature Update

**User Story:** As a developer, I want the tick orchestrator to accept per-species chemical configs, so that the emission, respawn, diffusion, and decay phases can look up the correct config for each chemical species.

#### Acceptance Criteria

1. THE Tick_Orchestrator SHALL accept a slice of ChemicalSpeciesConfig (`&[ChemicalSpeciesConfig]`) for chemical configurations instead of a single `&SourceFieldConfig`.
2. WHEN processing a depletion event for `SourceField::Chemical(i)`, THE Tick_Orchestrator SHALL index into the chemical config slice at position i to determine respawn eligibility and cooldown range.
3. THE Tick_Orchestrator SHALL extract per-species diffusion rates into a contiguous `&[f32]` slice and pass it to the Diffusion_System.
4. THE Tick_Orchestrator SHALL extract per-species decay rates into a contiguous `&[f32]` slice and pass it to the Decay_System.

### Requirement 9: No Actor Behavior Changes

**User Story:** As a simulation designer, I want actors to continue interacting exclusively with chemical species 0, so that this environmental configuration change does not alter actor behavior.

#### Acceptance Criteria

1. THE actor sensing system (`run_actor_sensing`) SHALL continue reading from chemical species 0 only.
2. THE actor metabolism system (`run_actor_metabolism`) SHALL continue consuming from chemical species 0 only.
3. WHEN `num_chemicals` changes, THE actor systems SHALL remain unaffected.

### Requirement 10: Documentation Updates

**User Story:** As a simulation operator, I want the example config file, info panel, and steering file config reference to reflect the new per-species format, so that documentation stays in sync with the code.

#### Acceptance Criteria

1. THE example_config.toml SHALL use `[[world_init.chemical_species_configs]]` with entries for each species, replacing the old `[world_init.chemical_source_config]` section, and SHALL remove `diffusion_rate` and `chemical_decay_rates` from the `[grid]` section.
2. THE Info_Panel SHALL display each chemical species' full config (source config, decay rate, diffusion rate) separately, labeled by species index, and SHALL remove the old shared `diffusion_rate` and `chemical_decay_rates` display from the grid section.
3. THE config-documentation steering file SHALL update the configuration reference to document the new `[[world_init.chemical_species_configs]]` array-of-tables format and remove the old `diffusion_rate` and `chemical_decay_rates` entries from the `[grid]` section.
