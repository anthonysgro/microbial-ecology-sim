# Implementation Plan: Seeded World Initialization

## Overview

Implement a COLD-path procedural initialization system in `src/grid/world_init.rs` that generates deterministic initial grid state from a `u64` seed. Replace the hardcoded setup in `main.rs`. Uses `ChaCha8Rng` with RNG forking for phase isolation. All validation via `thiserror` error types, no panics.

## Tasks

- [x] 1. Add `rand` and `rand_chacha` dependencies
  - Add `rand = "0.8"` and `rand_chacha = "0.8"` to `[dependencies]` in `Cargo.toml`
  - These provide `SeedableRng`, `Rng` trait, and `ChaCha8Rng` for deterministic seeded generation
  - _Requirements: 1.1, 1.2_

- [x] 2. Implement `WorldInitConfig`, `WorldInitError`, and validation
  - [x] 2.1 Create `src/grid/world_init.rs` with `WorldInitConfig` struct and `WorldInitError` enum
    - Define `WorldInitConfig` with all range fields (heat sources, chemical sources, emission rate, initial heat, initial concentration)
    - Implement `Default` for `WorldInitConfig` with reasonable ranges
    - Define `WorldInitError` using `thiserror` with `InvalidRange`, `GridError`, and `SourceError` variants
    - Register the module in `src/grid/mod.rs`
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

  - [x] 2.2 Implement `validate_config` function
    - Check all five range fields: return `WorldInitError::InvalidRange` on first min > max
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

  - [ ]* 2.3 Write property test for invalid config rejection
    - **Property 7: Invalid config rejection**
    - Generate `WorldInitConfig` where at least one range has min > max, verify `initialize` returns `WorldInitError::InvalidRange`
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**

- [x] 3. Implement source generation
  - [x] 3.1 Implement `generate_sources` function
    - Accept `&mut Grid`, `&mut impl Rng`, `&WorldInitConfig`, `num_chemicals: usize`
    - Sample heat source count from `[min_heat_sources, max_heat_sources]` using `rng.gen_range()`
    - For each heat source: sample `cell_index` from `[0, cell_count)`, sample `emission_rate` from `[min_emission_rate, max_emission_rate]`, call `grid.add_source()`
    - Repeat for chemical sources: for each species `0..num_chemicals`, sample count from `[min_chemical_sources, max_chemical_sources]`, generate sources with `SourceField::Chemical(species)`
    - Propagate `SourceError` via `?` into `WorldInitError`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

  - [ ]* 3.2 Write property test for source counts within configured ranges
    - **Property 3: Source counts within configured ranges**
    - Generate random seed, GridConfig, valid WorldInitConfig; call `initialize`; count heat and per-species chemical sources; verify counts in `[min, max]`
    - **Validates: Requirements 2.1, 2.2**

  - [ ]* 3.3 Write property test for source parameters within bounds
    - **Property 4: Source parameters within bounds**
    - For all sources in initialized grid, verify `cell_index < cell_count` and `emission_rate` in `[min_emission_rate, max_emission_rate]`
    - **Validates: Requirements 2.3, 2.4**

- [ ] 4. Implement field population
  - [ ] 4.1 Implement `populate_fields` function
    - Accept `&mut Grid`, `&mut impl Rng`, `&WorldInitConfig`, `num_chemicals: usize`
    - For each cell: sample heat from `[min_initial_heat, max_initial_heat]`, write to heat write buffer
    - For each species, for each cell: sample concentration from `[min_initial_concentration, max_initial_concentration]`, write to chemical write buffer
    - After writing all values, swap heat and chemical buffers so seeded values land in the read buffers
    - _Requirements: 3.1, 3.2, 3.3_

  - [ ]* 4.2 Write property test for field values within configured ranges
    - **Property 5: Field values within configured ranges**
    - After `initialize`, read all cells from `grid.read_heat()` and `grid.read_chemical(i)`; verify all values in configured ranges
    - **Validates: Requirements 3.1, 3.2, 3.3**

- [ ] 5. Implement top-level `initialize` function and RNG forking
  - [ ] 5.1 Implement `pub fn initialize(seed: u64, grid_config: GridConfig, init_config: &WorldInitConfig) -> Result<Grid, WorldInitError>`
    - Call `validate_config`
    - Construct `CellDefaults` with zeros (field population overwrites them)
    - Call `Grid::new(grid_config, defaults)?`
    - Create master `ChaCha8Rng::seed_from_u64(seed)`
    - Fork into `source_rng` and `field_rng` via `ChaCha8Rng::from_rng(&mut master_rng)`
    - Call `generate_sources(&mut grid, &mut source_rng, init_config, num_chemicals)?`
    - Call `populate_fields(&mut grid, &mut field_rng, init_config, num_chemicals)`
    - Return `Ok(grid)`
    - _Requirements: 1.1, 1.2, 4.1, 4.2, 4.3_

  - [ ]* 5.2 Write property test for deterministic initialization
    - **Property 1: Deterministic initialization**
    - For any seed, GridConfig, valid WorldInitConfig: call `initialize` twice, compare all field buffers cell-by-cell and source registries
    - **Validates: Requirements 1.2**

  - [ ]* 5.3 Write property test for seed sensitivity
    - **Property 2: Seed sensitivity**
    - For any two distinct seeds with same config: call `initialize` on each, verify at least one field value or source parameter differs
    - **Validates: Requirements 1.3**

- [ ] 6. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement grid-valid-for-tick property test
  - [ ]* 7.1 Write property test for grid valid for tick execution
    - **Property 6: Grid valid for tick execution**
    - For any seed and valid WorldInitConfig: call `initialize`, then call `TickOrchestrator::step` on the result; verify no error
    - **Validates: Requirements 4.3**

- [ ] 8. Integrate into `main.rs`
  - [ ] 8.1 Replace hardcoded initialization in `main.rs` with `world_init::initialize`
    - Accept seed as an optional CLI argument (e.g., `std::env::args().nth(1)` parsed as `u64`, defaulting to a fixed seed like `42`)
    - Construct a `WorldInitConfig::default()`
    - Call `world_init::initialize(seed, config, &init_config)` instead of `Grid::new` + manual `add_source` calls
    - Pass the returned `Grid` to `run_visualization`
    - _Requirements: 6.1, 6.2, 6.3_

- [ ] 9. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- `proptest` is already in `[dev-dependencies]`
- This is entirely COLD path code — allocations and dynamic dispatch are permitted
- `ChaCha8Rng` chosen for portable determinism (no platform-dependent output)
- RNG forking isolates source generation from field population, so adding new generation phases won't break determinism of existing phases
- Field population writes to write buffers then swaps, avoiding the need for a `read_mut()` accessor on `FieldBuffer`
