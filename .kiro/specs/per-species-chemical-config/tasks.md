# Implementation Plan: Per-Species Chemical Configuration

## Overview

Replace the fragmented per-species chemical config (single `chemical_source_config`, flat `chemical_decay_rates`, scalar `diffusion_rate`) with a unified `ChemicalSpeciesConfig` bundle per species. Update all consumers: config parsing/validation, source generation, emission/respawn, diffusion (HOT), decay (HOT), tick orchestrator, analysis, visualization, and documentation. Default `num_chemicals` changes from 1 to 2.

## Tasks

- [x] 1. Create ChemicalSpeciesConfig and update data model
  - [x] 1.1 Add `ChemicalSpeciesConfig` struct to `src/grid/world_init.rs`
    - Define struct with `source_config: SourceFieldConfig`, `decay_rate: f32`, `diffusion_rate: f32`
    - Implement `Default` with `max_sources = 3`, `decay_rate = 0.05`, `diffusion_rate = 0.05`
    - Derive `Debug, Clone, PartialEq, Serialize, Deserialize` with `#[serde(default)]`
    - _Requirements: 1.1, 1.2_

  - [x] 1.2 Update `WorldInitConfig` to use `chemical_species_configs: Vec<ChemicalSpeciesConfig>`
    - Replace `chemical_source_config: SourceFieldConfig` with `chemical_species_configs: Vec<ChemicalSpeciesConfig>`
    - Update `Default` impl to produce two entries (matching `num_chemicals = 2`)
    - _Requirements: 1.1, 1.4_

  - [x] 1.3 Update `GridConfig` to remove `diffusion_rate` and `chemical_decay_rates`
    - Remove `diffusion_rate: f32` field
    - Remove `chemical_decay_rates: Vec<f32>` field
    - Change `num_chemicals` default from 1 to 2
    - _Requirements: 1.3, 1.5_

  - [x] 1.4 Update `Grid::new()` in `src/grid/mod.rs`
    - Remove the `chemical_decay_rates` length and range validation from `Grid::new()`
    - These checks move to `validate_config()` for `ChemicalSpeciesConfig` entries
    - _Requirements: 1.3_

- [x] 2. Update validation logic
  - [x] 2.1 Update `validate_config()` in `src/grid/world_init.rs`
    - Add `WorldInitError` variants: `ChemicalSpeciesConfigError { species, source }`, `InvalidDecayRate { species, value }`, `InvalidDiffusionRate { species, value }`
    - Replace single `validate_source_field_config(&config.chemical_source_config, &CHEMICAL_LABELS)` with a loop over `chemical_species_configs`
    - For each entry: validate `source_config` (reuse `validate_source_field_config`), validate `decay_rate` in [0.0, 1.0], validate `diffusion_rate` >= 0.0 and finite
    - Error messages must include species index
    - _Requirements: 3.2, 3.3, 3.4, 3.5_

  - [x] 2.2 Update `validate_world_config()` in `src/io/config_file.rs`
    - Replace `chemical_decay_rates.len() != num_chemicals` check with `chemical_species_configs.len() != num_chemicals`
    - _Requirements: 3.1_

  - [ ]* 2.3 Write property test: TOML round-trip (Property 1)
    - **Property 1: TOML serialization round-trip**
    - Generate arbitrary valid `WorldInitConfig` with 1–4 species, serialize to TOML, deserialize, assert equality
    - **Validates: Requirements 2.1**

  - [ ]* 2.4 Write property test: length mismatch validation (Property 2)
    - **Property 2: Length mismatch validation rejects mismatched configs**
    - Generate arbitrary `num_chemicals` (1–8) and `chemical_species_configs` of different length, assert validation error
    - **Validates: Requirements 3.1**

  - [ ]* 2.5 Write property test: per-entry validation identifies species index (Property 3)
    - **Property 3: Per-entry validation identifies species index**
    - Generate valid multi-species config, corrupt one random entry, assert error identifies species index
    - **Validates: Requirements 3.2, 3.3, 3.4, 3.5**

- [x] 3. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Update source generation
  - [x] 4.1 Update `generate_sources()` in `src/grid/world_init.rs`
    - Change chemical source loop to index `config.chemical_species_configs[species].source_config` per species
    - Each species uses its own source count range, emission rate range, reservoir params, clustering
    - _Requirements: 4.1, 4.2_

  - [ ]* 4.2 Write property test: source generation respects per-species config (Property 4)
    - **Property 4: Source generation respects per-species config**
    - Generate configs with 2–4 species having distinct source count ranges, run `generate_sources`, count sources per species, assert within range
    - **Validates: Requirements 4.1, 4.2**

- [x] 5. Update HOT path systems (diffusion and decay)
  - [x] 5.1 Update `run_diffusion()` in `src/grid/diffusion.rs`
    - Add `diffusion_rates: &[f32]` parameter
    - Remove `config.diffusion_rate` usage, use `diffusion_rates[species]` per species
    - Skip species with `diffusion_rates[species] == 0.0` (no read, no write)
    - HOT path: no allocation, contiguous slice access only
    - _Requirements: 6.1, 6.2, 6.3_

  - [x] 5.2 Update `run_decay()` in `src/grid/decay.rs`
    - Add `decay_rates: &[f32]` parameter
    - Replace `config.chemical_decay_rates[species]` with `decay_rates[species]`
    - Preserve existing skip-zero behavior
    - HOT path: no allocation, contiguous slice access only
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ]* 5.3 Write property test: per-species diffusion rates (Property 6)
    - **Property 6: Per-species diffusion rates applied correctly**
    - Create grid with 2 species, species 0 diffusion_rate=0.0, species 1 diffusion_rate>0.0, non-uniform concentrations, assert species 0 unchanged and species 1 changed
    - **Validates: Requirements 6.1, 6.3**

  - [ ]* 5.4 Write property test: per-species decay rates (Property 7)
    - **Property 7: Per-species decay rates applied correctly**
    - Create grid with 2 species, species 0 decay_rate=0.0, species 1 decay_rate>0.0, positive concentrations, assert species 0 unchanged and species 1 reduced
    - **Validates: Requirements 7.1, 7.3**

- [x] 6. Update tick orchestrator and emission/respawn
  - [x] 6.1 Update `TickOrchestrator::step()` in `src/grid/tick.rs`
    - Change signature: `chemical_source_config: &SourceFieldConfig` → `chemical_species_configs: &[ChemicalSpeciesConfig]`
    - Extract `diffusion_rates: SmallVec<[f32; 8]>` and `decay_rates: SmallVec<[f32; 8]>` from configs
    - Pass `&diffusion_rates` to `run_diffusion()` and `&decay_rates` to `run_decay()`
    - _Requirements: 8.1, 8.3, 8.4_

  - [x] 6.2 Update `run_emission_phase()` in `src/grid/tick.rs`
    - Change signature: `chemical_config: &SourceFieldConfig` → `chemical_species_configs: &[ChemicalSpeciesConfig]`
    - Index `chemical_species_configs[i].source_config` for depletion event processing
    - _Requirements: 5.1, 8.2_

  - [x] 6.3 Update `run_respawn_phase()` in `src/grid/source.rs`
    - Change signature: `chemical_config: &SourceFieldConfig` → `chemical_species_configs: &[ChemicalSpeciesConfig]`
    - Index `chemical_species_configs[i].source_config` when `entry.field == SourceField::Chemical(i)`
    - _Requirements: 5.2, 5.3_

  - [ ]* 6.4 Write property test: respawn uses per-species config (Property 5)
    - **Property 5: Respawn uses per-species config parameters**
    - Set up grid with mature respawn entry for a random species, run `run_respawn_phase`, verify new source parameters within species config ranges
    - **Validates: Requirements 5.1, 5.2, 5.3**

- [x] 7. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Update call sites and analysis
  - [x] 8.1 Update `tick_simulation()` in `src/viz_bevy/systems.rs`
    - Pass `&viz_config.init_config.chemical_species_configs` instead of `&viz_config.init_config.chemical_source_config`
    - _Requirements: 8.1_

  - [x] 8.2 Update `src/io/analysis.rs`
    - Update `analyze_stability()`: compute per-species diffusion stability numbers from `WorldInitConfig.chemical_species_configs`
    - Update `analyze_chemical_budget()`: use `chemical_species_configs[0].source_config` and `chemical_species_configs[0].decay_rate`
    - Update `analyze_carrying_capacity()`: use `chemical_species_configs[0].source_config`
    - Update `analyze_source_density()`: use `chemical_species_configs[0].source_config`
    - Update `analyze_diffusion()`: compute per-species diffusion length scales and half-lives from `chemical_species_configs`
    - Update function signatures as needed to accept `&WorldInitConfig` or `&[ChemicalSpeciesConfig]`
    - _Requirements: 1.3, 6.1, 7.1_

- [x] 9. Update documentation and visualization
  - [x] 9.1 Update `format_config_info()` in `src/viz_bevy/setup.rs`
    - Remove `diffusion_rate` and `chemical_decay_rates` from grid section
    - Replace single chemical source config block with loop over `chemical_species_configs`
    - Display source_config, decay_rate, and diffusion_rate per species, labeled by species index
    - _Requirements: 10.2_

  - [ ]* 9.2 Write property test: info panel displays all species configs (Property 8)
    - **Property 8: Info panel displays all species configs**
    - Generate configs with 1–4 species, call `format_config_info`, assert output contains species-indexed labels with decay_rate and diffusion_rate for each
    - **Validates: Requirements 10.2**

  - [x] 9.3 Update `example_config.toml`
    - Remove `diffusion_rate` and `chemical_decay_rates` from `[grid]`
    - Replace `[world_init.chemical_source_config]` with `[[world_init.chemical_species_configs]]` entries
    - Add one entry per species with `source_config`, `decay_rate`, `diffusion_rate` fields
    - _Requirements: 10.1_

  - [x] 9.4 Update `config-documentation.md` steering file
    - Remove `diffusion_rate` and `chemical_decay_rates` from `[grid]` section
    - Remove `[world_init.chemical_source_config]` section
    - Add `[[world_init.chemical_species_configs]]` section documenting `ChemicalSpeciesConfig` fields
    - _Requirements: 10.3_

- [x] 10. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- HOT path changes (diffusion, decay) receive pre-extracted `&[f32]` slices — no struct indexing in inner loops
- Actor systems (`run_actor_sensing`, `run_actor_metabolism`) are NOT modified — actors still interact with species 0 only
- `SmallVec<[f32; 8]>` for rate extraction is stack-allocated for up to 8 species
- Property tests use `proptest` crate with minimum 100 iterations
- The `src/io/analysis.rs` functions need signature updates since they reference removed `GridConfig` fields
