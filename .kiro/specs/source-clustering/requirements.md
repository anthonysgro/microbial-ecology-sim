# Requirements Document

## Introduction

Add a spatial clustering parameter (`source_clustering`) to `SourceFieldConfig` that controls how chemical and heat sources are distributed on the grid during world initialization. Currently sources are placed uniformly at random, which leads to homogeneous resource landscapes and convergent evolutionary strategies. This feature introduces a "clumpiness" knob that ranges from evenly-spread placement (0.0) to tightly-clustered hotspot placement (1.0), creating spatial heterogeneity that drives evolutionary diversity.

## Glossary

- **Source_Clustering_Parameter**: A floating-point field `source_clustering` on `SourceFieldConfig`, in the range `[0.0, 1.0]`, controlling the spatial distribution of sources during world initialization.
- **SourceFieldConfig**: The shared configuration struct used by both `heat_source_config` and `chemical_source_config` in `WorldInitConfig`.
- **Placement_Algorithm**: The function within `generate_sources` responsible for selecting cell indices for new sources.
- **Cluster_Center**: A randomly-chosen grid cell around which sources are placed when `source_clustering > 0.0`.
- **Grid**: The 2D cell grid (`width × height`) representing the simulation environment.
- **Config_Validator**: The validation logic in `validate_source_field_config` and `validate_world_config` that enforces field constraints.
- **Info_Panel**: The Bevy visualization panel (toggled by pressing `I`) that displays active configuration values via `format_config_info`.

## Requirements

### Requirement 1: Source Clustering Configuration Field

**User Story:** As a simulation designer, I want a `source_clustering` parameter on `SourceFieldConfig`, so that I can control the spatial distribution of heat and chemical sources independently.

#### Acceptance Criteria

1. THE SourceFieldConfig SHALL include a `source_clustering` field of type `f32` with a default value of `0.0`.
2. WHEN `source_clustering` is `0.0`, THE Placement_Algorithm SHALL distribute sources uniformly at random across all grid cells, preserving the current behavior.
3. WHEN `source_clustering` is `1.0`, THE Placement_Algorithm SHALL place sources in tight clusters around one or more randomly-chosen Cluster_Centers.
4. WHEN `source_clustering` is between `0.0` and `1.0` exclusive, THE Placement_Algorithm SHALL interpolate between uniform and clustered placement, producing progressively tighter groupings as the value increases.

### Requirement 2: Clustering Placement Algorithm

**User Story:** As a simulation designer, I want the clustering algorithm to produce spatially coherent hotspots, so that resource oases and barren gaps emerge on the grid.

#### Acceptance Criteria

1. WHEN placing a source with `source_clustering > 0.0`, THE Placement_Algorithm SHALL select a Cluster_Center and then offset each source position from that center using a distance distribution controlled by `source_clustering`.
2. THE Placement_Algorithm SHALL wrap source positions using toroidal (modular) arithmetic so that clusters near grid edges wrap around correctly.
3. THE Placement_Algorithm SHALL use seeded, deterministic RNG for all random decisions, so that identical seeds and configurations produce identical source layouts.
4. WHEN multiple sources are placed for the same field type, THE Placement_Algorithm SHALL reuse the same Cluster_Center for all sources in that batch, producing a single cluster per field type per invocation.

### Requirement 3: Configuration Validation

**User Story:** As a simulation designer, I want invalid `source_clustering` values to be rejected at config load time, so that I receive clear error messages instead of undefined behavior.

#### Acceptance Criteria

1. IF `source_clustering` is less than `0.0`, THEN THE Config_Validator SHALL return a validation error indicating the value is below the minimum.
2. IF `source_clustering` is greater than `1.0`, THEN THE Config_Validator SHALL return a validation error indicating the value exceeds the maximum.
3. IF `source_clustering` is NaN or infinite, THEN THE Config_Validator SHALL return a validation error indicating the value is not finite.

### Requirement 4: TOML Configuration Support

**User Story:** As a simulation designer, I want to set `source_clustering` in my TOML config file, so that I can tune clustering without recompiling.

#### Acceptance Criteria

1. THE SourceFieldConfig SHALL deserialize `source_clustering` from the TOML `[world_init.heat_source_config]` and `[world_init.chemical_source_config]` sections.
2. WHEN `source_clustering` is omitted from the TOML file, THE SourceFieldConfig SHALL default to `0.0`, preserving backward compatibility.
3. WHEN an unknown key is present in the TOML file, THE config parser SHALL reject it at parse time (existing `deny_unknown_fields` behavior preserved).

### Requirement 5: Documentation Updates

**User Story:** As a simulation designer, I want the example config, info panel, and config-documentation steering file to reflect the new parameter, so that documentation stays in sync with the code.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include `source_clustering` entries in both `[world_init.heat_source_config]` and `[world_init.chemical_source_config]` sections with explanatory comments.
2. THE Info_Panel SHALL display the `source_clustering` value for both heat and chemical source configs via `format_config_info`.
3. THE `config-documentation.md` steering file SHALL include `source_clustering` in the `SourceFieldConfig` configuration reference table.

### Requirement 6: Determinism

**User Story:** As a simulation developer, I want source placement to remain fully deterministic, so that identical seed + config combinations always produce identical grids for replay and debugging.

#### Acceptance Criteria

1. FOR ALL values of `source_clustering` in `[0.0, 1.0]`, THE Placement_Algorithm SHALL produce identical source layouts given the same seed, grid dimensions, and `SourceFieldConfig`.
2. THE Placement_Algorithm SHALL not introduce new RNG stream dependencies that alter the output of existing RNG consumers (field population, actor generation) for a given seed.
