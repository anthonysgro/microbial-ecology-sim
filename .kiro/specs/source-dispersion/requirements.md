# Requirements Document

## Introduction

Add a `source_dispersion` parameter to `SourceFieldConfig` that controls how many distinct cluster centers sources are distributed across for a given field type (heat or chemical species). Currently all sources for a field type share a single cluster center (controlled by `source_clustering` which sets intra-cluster tightness). The new `source_dispersion` parameter controls inter-cluster count: `0.0` means one shared center (current behavior), `1.0` means each source gets its own independent center, and intermediate values interpolate linearly. The formula is `num_clusters = max(1, round(source_dispersion * num_sources))`.

## Glossary

- **Source_Dispersion_Parameter**: A `[0.0, 1.0]` float on `SourceFieldConfig` controlling how many distinct cluster centers sources are distributed across. `0.0` = one center (backward compatible), `1.0` = one center per source.
- **Cluster_Center**: A `(col, row)` grid coordinate around which sources are spatially placed using the existing `source_clustering` sigma.
- **Source_Clustering_Parameter**: The existing `[0.0, 1.0]` float on `SourceFieldConfig` controlling intra-cluster tightness (sigma of the 2D normal offset).
- **Cluster_Center_Map**: The `SmallVec<[(SourceField, ClusterCenter); 4]>` stored on `Grid` that maps field types to their cluster centers for respawn.
- **Source_Field_Config**: The shared configuration struct (`SourceFieldConfig`) used by both heat and per-species chemical source generation.
- **Cluster_Index**: An integer field on `Source` identifying which cluster center the source belongs to, used during respawn to sample near the correct center.
- **Generate_Sources**: The COLD-path function in `world_init.rs` that procedurally places sources during initialization.
- **Run_Respawn_Phase**: The WARM-path function in `source.rs` that spawns replacement sources when depleted non-renewable sources complete their cooldown.
- **Config_Info_Panel**: The Bevy visualization panel (toggled by `I` key) displaying all active configuration values.

## Requirements

### Requirement 1: Source Dispersion Configuration Field

**User Story:** As a simulation designer, I want to configure how many cluster centers sources are distributed across, so that I can create spatially diverse resource landscapes with multiple hotspots per field type.

#### Acceptance Criteria

1. THE Source_Field_Config SHALL include a `source_dispersion` field of type `f32` with a default value of `0.0`.
2. WHEN `source_dispersion` is `0.0`, THE Generate_Sources function SHALL produce one cluster center per field batch, preserving current behavior.
3. WHEN `source_dispersion` is `1.0`, THE Generate_Sources function SHALL produce one cluster center per source in the batch.
4. WHEN `source_dispersion` is between `0.0` and `1.0` exclusive, THE Generate_Sources function SHALL compute the number of cluster centers as `max(1, round(source_dispersion * num_sources))`.

### Requirement 2: Source Dispersion Validation

**User Story:** As a simulation designer, I want invalid `source_dispersion` values to be rejected at config load time, so that I receive clear error messages instead of undefined behavior.

#### Acceptance Criteria

1. WHEN `source_dispersion` is less than `0.0` or greater than `1.0`, THEN THE Validator SHALL return an error indicating the value is out of range.
2. WHEN `source_dispersion` is not finite (NaN or infinity), THEN THE Validator SHALL return an error indicating the value is not finite.

### Requirement 3: Multi-Center Source Generation

**User Story:** As a simulation designer, I want sources distributed across multiple cluster centers, so that resource hotspots form at distinct spatial locations rather than a single point.

#### Acceptance Criteria

1. WHEN `source_dispersion` produces K cluster centers (K > 1), THE Generate_Sources function SHALL sample K independent random grid positions as cluster centers.
2. WHEN K cluster centers exist, THE Generate_Sources function SHALL assign each source to a cluster center using round-robin assignment (source index modulo K).
3. WHEN a source is assigned to a cluster center, THE Generate_Sources function SHALL sample the source position using `sample_clustered_position` with that cluster center's coordinates and the field's `source_clustering` sigma.
4. THE Generate_Sources function SHALL store all K cluster centers in the Cluster_Center_Map so that the Run_Respawn_Phase can look up the correct center for each source.

### Requirement 4: Source Cluster Index for Respawn

**User Story:** As a simulation engineer, I want each source to know which cluster center it belongs to, so that respawned sources appear near the correct spatial cluster.

#### Acceptance Criteria

1. THE Source struct SHALL include a `cluster_index` field of type `u8` identifying which cluster center the source belongs to.
2. WHEN a source is created during generation, THE Generate_Sources function SHALL set the source's `cluster_index` to the index of its assigned cluster center.
3. WHEN a depleted source triggers a respawn entry, THE Run_Respawn_Phase SHALL record the depleted source's `cluster_index` and `field` in the respawn entry.
4. WHEN a respawn entry matures, THE Run_Respawn_Phase SHALL look up the cluster center corresponding to the entry's `field` and `cluster_index`, and sample the replacement source position near that center.

### Requirement 5: Cluster Center Map Multi-Center Storage

**User Story:** As a simulation engineer, I want the cluster center storage to support multiple centers per field type, so that respawn can resolve the correct center for any source regardless of dispersion level.

#### Acceptance Criteria

1. THE Cluster_Center_Map SHALL support storing multiple cluster centers per Source_Field variant, indexed by cluster index.
2. WHEN `source_dispersion` is `0.0` and `source_clustering` is `0.0`, THE Generate_Sources function SHALL store zero cluster centers for that field (preserving the uniform-random respawn fallback).
3. WHEN `source_dispersion` is `0.0` and `source_clustering` is greater than `0.0`, THE Generate_Sources function SHALL store exactly one cluster center for that field (preserving current single-center behavior).
4. WHEN `source_dispersion` is greater than `0.0`, THE Generate_Sources function SHALL store all K cluster centers for that field.

### Requirement 6: Backward Compatibility

**User Story:** As an existing user, I want the simulation to behave identically when `source_dispersion` is omitted or set to `0.0`, so that existing configurations produce the same results.

#### Acceptance Criteria

1. WHEN `source_dispersion` is omitted from the TOML configuration, THE Parser SHALL default the value to `0.0`.
2. WHEN `source_dispersion` is `0.0`, THE Generate_Sources function SHALL produce identical source placement to the pre-feature implementation for the same seed.
3. WHEN `source_dispersion` is `0.0`, THE Run_Respawn_Phase SHALL behave identically to the pre-feature implementation.

### Requirement 7: Documentation Updates

**User Story:** As a simulation designer, I want the new parameter documented in all relevant locations, so that I can discover and understand the configuration option.

#### Acceptance Criteria

1. THE example_config.toml SHALL include the `source_dispersion` field with a comment explaining its purpose and valid range `[0.0, 1.0]`.
2. THE Config_Info_Panel SHALL display the `source_dispersion` value for heat and each chemical species.
3. THE config-documentation.md steering file SHALL include the `source_dispersion` field in the `SourceFieldConfig` table.
