# Requirements Document

## Introduction

When `source_clustering > 0.0`, the world initialization phase clusters sources around randomly-chosen centers per field type (Heat, Chemical(0), Chemical(1), etc.). However, the respawn phase (`run_respawn_phase`) places replacement sources at uniform-random positions, ignoring the original cluster geometry. Over many depletion–respawn cycles, sources scatter across the grid, destroying the spatial structure that `source_clustering` was designed to create.

This feature persists the cluster center coordinates computed during `generate_sources` and makes `run_respawn_phase` reuse them when placing replacement sources, so that respawned sources land back in their original cluster region.

## Glossary

- **Grid**: The top-level environment struct owning all field buffers, source registry, respawn queue, and spatial metadata.
- **SourceField**: An enum identifying which grid field a source emits into (`Heat` or `Chemical(species_index)`).
- **ClusterCenter**: A `(col, row)` coordinate pair identifying the spatial center of a source cluster for a given `SourceField` variant.
- **ClusterCenterMap**: A collection stored on `Grid` that maps each `SourceField` variant to its `ClusterCenter`. Only populated for field types where `source_clustering > 0.0` at initialization time.
- **RespawnPhase**: The WARM-path system (`run_respawn_phase`) that spawns replacement sources for depleted non-renewable sources after their cooldown expires.
- **GenerateSources**: The COLD-path function (`generate_sources`) that creates initial sources during world initialization, computing ephemeral cluster centers.
- **SampleClusteredPosition**: The existing helper function that offsets a position from a cluster center using a 2D normal distribution with toroidal wrapping.

## Requirements

### Requirement 1: Persist Cluster Centers During Initialization

**User Story:** As a simulation operator, I want cluster centers computed during world initialization to be stored on the Grid, so that respawn logic can reuse them.

#### Acceptance Criteria

1. WHEN `generate_sources` computes a cluster center for a `SourceField` variant and `source_clustering > 0.0`, THE GenerateSources function SHALL store that center as a `ClusterCenter` entry in the Grid's ClusterCenterMap.
2. WHEN `source_clustering == 0.0` for a given `SourceFieldConfig`, THE GenerateSources function SHALL store no ClusterCenter entry for that field type in the ClusterCenterMap.
3. THE ClusterCenterMap SHALL support one independent ClusterCenter per distinct `SourceField` variant, including separate entries for each chemical species index.
4. WHEN the Grid is constructed, THE Grid SHALL initialize the ClusterCenterMap as empty.

### Requirement 2: Respawn Sources at Cluster Centers

**User Story:** As a simulation operator, I want respawned sources to land near their field type's original cluster center, so that spatial clustering is preserved across depletion–respawn cycles.

#### Acceptance Criteria

1. WHEN a respawn entry matures and a ClusterCenter exists for that entry's `SourceField`, THE RespawnPhase SHALL use `sample_clustered_position` with the stored ClusterCenter and the field's `source_clustering` value to select the replacement source's cell index.
2. WHEN a respawn entry matures and no ClusterCenter exists for that entry's `SourceField`, THE RespawnPhase SHALL select the replacement source's cell index using uniform-random placement (current behavior).
3. THE RespawnPhase SHALL continue to reject occupied cells and retry placement, consistent with existing collision-avoidance logic.
4. THE RespawnPhase SHALL receive the `source_clustering` value for the relevant field type from the `SourceFieldConfig` passed to it.

### Requirement 3: Determinism

**User Story:** As a simulation operator, I want identical seed and configuration to produce identical cluster center storage and respawn placement, so that simulation replay is exact.

#### Acceptance Criteria

1. FOR ALL identical seed and configuration inputs, THE GenerateSources function SHALL produce identical ClusterCenterMap contents.
2. FOR ALL identical seed, configuration, and simulation history, THE RespawnPhase SHALL produce identical replacement source positions.
3. THE ClusterCenterMap SHALL use a collection type whose lookup behavior does not depend on iteration order or hash randomization.

### Requirement 4: No Configuration Changes

**User Story:** As a simulation operator, I want this feature to work with the existing `source_clustering` parameter without requiring new TOML fields, so that my configuration files remain unchanged.

#### Acceptance Criteria

1. THE system SHALL reuse the existing `source_clustering` field on `SourceFieldConfig` to control both initial placement and respawn placement.
2. THE system SHALL require no new TOML configuration fields.

### Requirement 5: Data Model Constraints

**User Story:** As a systems engineer, I want the cluster center storage to be minimal and cache-friendly, so that it does not degrade WARM-path performance.

#### Acceptance Criteria

1. THE ClusterCenter struct SHALL contain exactly two fields: a column coordinate and a row coordinate, both stored as `u32`.
2. THE ClusterCenterMap SHALL be a small, fixed-overhead collection suitable for the expected cardinality (one entry per active `SourceField` variant, typically 2–5 entries).
3. THE ClusterCenterMap SHALL support O(n) or better lookup by `SourceField` key, where n is the number of entries.

### Requirement 6: Signature Changes to Respawn Phase

**User Story:** As a systems engineer, I want `run_respawn_phase` to accept the additional context it needs for clustered respawn without breaking the existing call site.

#### Acceptance Criteria

1. WHEN `run_respawn_phase` is called, THE caller SHALL pass a reference to the Grid's ClusterCenterMap and the `source_clustering` values from both heat and chemical `SourceFieldConfig` structs.
2. THE `run_respawn_phase` function signature SHALL accept the ClusterCenterMap and grid dimensions needed to call `sample_clustered_position`.
