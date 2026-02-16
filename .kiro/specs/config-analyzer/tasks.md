# Implementation Plan: Config Analyzer

## Overview

Implement a standalone `config-analyzer` CLI binary that statically analyzes simulation TOML configs and prints a formatted report. The analysis module lives in `src/io/analysis.rs` as pure functions over config structs. The binary entry point lives in `src/bin/config_analyzer.rs`.

## Tasks

- [x] 1. Set up binary target and analysis module skeleton
  - [x] 1.1 Add `[[bin]]` target to `Cargo.toml` for `config-analyzer` pointing to `src/bin/config_analyzer.rs`
    - _Requirements: 9.1, 9.2_
  - [x] 1.2 Create `src/bin/config_analyzer.rs` with CLI parsing (`--config <path>`), config loading via `load_world_config`, validation via `validate_world_config`, and `anyhow`-based error handling in `main`
    - Parse `--config <path>` manually (same pattern as existing `cli.rs`)
    - Print usage to stderr and exit 1 when `--config` is missing
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 9.3, 9.4_
  - [x] 1.3 Create `src/io/analysis.rs` with the module declaration and stub `analyze` function returning a `FullReport`
    - Define all report structs: `FullReport`, `StabilityReport`, `ChemicalBudgetReport`, `EnergyBudgetReport`, `CarryingCapacityReport`, `SourceDensityReport`, `DiffusionReport`
    - Add `pub mod analysis;` to `src/io/mod.rs`
    - _Requirements: 8.1_

- [-] 2. Implement stability and diffusion analysis
  - [x] 2.1 Implement `analyze_stability(grid: &GridConfig) -> StabilityReport`
    - Compute `diffusion_number = diffusion_rate * tick_duration * 8.0`
    - Compute `thermal_stability_number = thermal_conductivity * tick_duration * 8.0`
    - Set `diffusion_stable` and `thermal_stable` flags
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_
  - [x] 2.2 Implement `analyze_diffusion(grid: &GridConfig) -> DiffusionReport`
    - Compute `chemical_length_scale = sqrt(diffusion_rate * tick_duration)`
    - Compute `thermal_length_scale = sqrt(thermal_conductivity * tick_duration)`
    - Compute `ticks_to_reach_5_cells` and `ticks_to_reach_10_cells`
    - Compute `chemical_half_lives` as `ln(2) / decay_rate` per species (guard against zero decay)
    - _Requirements: 7.1, 7.2, 7.3, 7.4_
  - [ ]* 2.3 Write property test for stability analysis
    - **Property 1: Stability analysis correctness**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**
  - [ ]* 2.4 Write property test for diffusion characterization
    - **Property 7: Diffusion characterization correctness**
    - **Validates: Requirements 7.1, 7.2, 7.3, 7.4**

- [x] 3. Implement chemical budget and source density analysis
  - [x] 3.1 Implement `analyze_chemical_budget(grid: &GridConfig, world_init: &WorldInitConfig, actor: Option<&ActorConfig>) -> ChemicalBudgetReport`
    - Compute midpoint source count and midpoint emission rate
    - Compute expected decay using midpoint initial concentration and decay rate
    - Compute actor consumption (0 if no ActorConfig)
    - Compute net balance and deficit flag
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_
  - [x] 3.2 Implement `analyze_source_density(grid: &GridConfig, world_init: &WorldInitConfig) -> SourceDensityReport`
    - Compute chemical and heat source densities
    - Report renewable fractions directly from config
    - Report respawn cooldown ranges when enabled
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
  - [ ]* 3.3 Write property test for chemical budget
    - **Property 2: Chemical budget correctness**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**
  - [ ]* 3.4 Write property test for source density
    - **Property 6: Source density correctness**
    - **Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5**

- [x] 4. Implement energy budget and carrying capacity analysis
  - [x] 4.1 Implement `analyze_energy_budget(grid: &GridConfig, world_init: &WorldInitConfig, actor: &ActorConfig) -> EnergyBudgetReport`
    - Compute net energy per tick at average concentration
    - Compute break-even concentration
    - Compute idle survival ticks
    - Compute ticks to reproduction (None if net <= 0)
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - [x] 4.2 Implement `analyze_carrying_capacity(grid: &GridConfig, world_init: &WorldInitConfig, actor: &ActorConfig) -> CarryingCapacityReport`
    - Compute carrying capacity as total chemical input / per-actor consumption
    - Compare to cell count for space-limited flag
    - _Requirements: 5.1, 5.2_
  - [ ]* 4.3 Write property test for energy budget
    - **Property 3: Energy budget correctness**
    - **Validates: Requirements 4.1, 4.3, 4.4, 4.5**
  - [ ]* 4.4 Write property test for break-even concentration round-trip
    - **Property 4: Break-even concentration round-trip**
    - **Validates: Requirements 4.2**
  - [ ]* 4.5 Write property test for carrying capacity
    - **Property 5: Carrying capacity correctness**
    - **Validates: Requirements 5.1, 5.2**

- [x] 5. Checkpoint - Ensure all analysis functions work
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Implement report formatting and wire everything together
  - [x] 6.1 Implement `analyze(config: &WorldConfig) -> FullReport` orchestrator function
    - Call each analysis function, passing appropriate config sections
    - Set `energy_budget` and `carrying_capacity` to `None` when no ActorConfig
    - _Requirements: 4.6, 5.3_
  - [x] 6.2 Implement `format_report(report: &FullReport) -> String`
    - Summary header with grid dimensions, seed, tick duration, actors enabled
    - Sections: Numerical Stability, Chemical Budget, Energy Budget, Carrying Capacity, Source Density, Diffusion Characterization
    - Prefix warnings with `[WARN]`, healthy confirmations with `[OK]`
    - Skip Energy Budget and Carrying Capacity sections when actors are disabled
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
  - [x] 6.3 Wire `format_report` into the binary entry point: call `analyze`, then `format_report`, then print to stdout
    - _Requirements: 8.2, 9.3_
  - [ ]* 6.4 Write property test for report formatting
    - **Property 8: Report formatting correctness**
    - **Validates: Requirements 8.3, 8.4, 8.5**
  - [ ]* 6.5 Write unit tests for report formatting edge cases
    - Test with actors disabled (no energy/carrying capacity sections)
    - Test with unstable diffusion (verify [WARN] prefix)
    - Test with the example_config.toml values
    - _Requirements: 8.1, 8.3, 8.4, 8.5_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties using `proptest`
- The analysis module contains only pure functions — no I/O, no side effects
- The binary is COLD path: standard Rust allocation practices apply
