# Implementation Plan: TOML World Configuration

## Overview

Add TOML-based configuration file support to the simulation engine. The implementation proceeds bottom-up: serde derives on existing structs, then the new `io` module (error type, config loader, CLI parser), then binary integration, then tests. Each step builds on the previous and ends with wired-in, compilable code.

## Tasks

- [ ] 1. Add dependencies and serde derives to existing config structs
  - [ ] 1.1 Add `serde`, `toml` dependencies to `Cargo.toml`
    - Add `serde = { version = "1", features = ["derive"] }` and `toml = "0.8"` to `[dependencies]`
    - _Requirements: 1.1, 1.4_
  - [ ] 1.2 Add `Serialize`, `Deserialize`, `Default` derives to `GridConfig`
    - Add `#[derive(Serialize, Deserialize)]` with `#[serde(default)]` to `GridConfig` in `src/grid/config.rs`
    - Implement `Default` for `GridConfig` matching the current hardcoded values in `main.rs` (width=30, height=30, etc.)
    - Add `#[derive(Serialize, Deserialize)]` with `#[serde(default)]` to `CellDefaults`
    - _Requirements: 1.1, 1.2, 4.1_
  - [ ] 1.3 Add `Serialize`, `Deserialize` derives to `WorldInitConfig` and `SourceFieldConfig`
    - Add `#[derive(Serialize, Deserialize)]` with `#[serde(default)]` to both structs in `src/grid/world_init.rs`
    - Add `Default` impl for `SourceFieldConfig` (extract from existing `WorldInitConfig::default()`)
    - _Requirements: 1.1, 1.2_
  - [ ] 1.4 Add `Serialize`, `Deserialize`, `Default` derives to `ActorConfig`
    - Add `#[derive(Serialize, Deserialize)]` with `#[serde(default)]` to `ActorConfig` in `src/grid/actor_config.rs`
    - Implement `Default` for `ActorConfig` matching the current hardcoded values in `main.rs`
    - _Requirements: 1.1, 1.2, 4.1_

- [ ] 2. Create the `src/io/` module with config error type
  - [ ] 2.1 Create `src/io/mod.rs` and `src/io/config_error.rs`
    - Define `ConfigError` enum with `Io`, `Parse`, `Serialize`, `Validation`, `CliError` variants using `thiserror`
    - Add `pub mod io;` to `src/lib.rs`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.3_

- [ ] 3. Implement config file loader and serializer
  - [ ] 3.1 Create `src/io/config_file.rs` with `WorldConfig`, `BevyExtras`, `BevyWorldConfig` structs
    - Define `WorldConfig` with `seed`, `grid: GridConfig`, `world_init: WorldInitConfig`, `actor: Option<ActorConfig>`, all with `#[serde(default)]` and `#[serde(deny_unknown_fields)]`
    - Define `BevyExtras` with Bevy-specific fields and defaults
    - Define `BevyWorldConfig` with `#[serde(flatten)]` for `WorldConfig` and `bevy: BevyExtras`
    - Implement `Default` for `WorldConfig` and `BevyExtras`
    - _Requirements: 1.1, 1.2, 1.3, 6.3_
  - [ ] 3.2 Implement `load_world_config`, `load_bevy_config`, `to_toml_string`, and `validate_world_config`
    - `load_world_config`: read file to string, deserialize with `toml::from_str`, return `Result<WorldConfig, ConfigError>`
    - `load_bevy_config`: same but deserializes into `BevyWorldConfig`
    - `to_toml_string`: serialize `WorldConfig` to pretty TOML string
    - `validate_world_config`: call `world_init::validate_config`, check `chemical_decay_rates.len() == num_chemicals`, check `removal_threshold <= 0.0`
    - _Requirements: 1.1, 1.3, 1.4, 2.1, 2.2, 2.3, 2.4_
  - [ ]* 3.3 Write property test: TOML round-trip consistency
    - **Property 1: TOML round-trip consistency**
    - Implement `Arbitrary` generators for all config structs via `proptest`
    - For any valid `WorldConfig`, `toml::from_str(to_toml_string(config))` equals the original
    - **Validates: Requirements 1.1, 1.4, 1.5, 5.1**
  - [ ]* 3.4 Write property test: default fallback for omitted fields
    - **Property 2: Default fallback for omitted fields**
    - Generate a `WorldConfig`, serialize a random subset of fields, deserialize, verify omitted fields equal `Default::default()`
    - **Validates: Requirements 1.2**
  - [ ]* 3.5 Write property test: unknown key rejection
    - **Property 3: Unknown key rejection**
    - Generate valid TOML, inject a random unknown key, verify deserialization fails
    - **Validates: Requirements 1.3**
  - [ ]* 3.6 Write property test: invalid range detection
    - **Property 4: Invalid range detection**
    - Generate `WorldInitConfig` with at least one inverted min/max pair, verify validation returns error
    - **Validates: Requirements 2.1**
  - [ ]* 3.7 Write property test: type mismatch rejection
    - **Property 5: Type mismatch rejection**
    - Generate valid TOML, replace a field value with an incompatible type, verify deserialization error
    - **Validates: Requirements 2.4**
  - [ ]* 3.8 Write unit tests for validation edge cases
    - Test decay rates length mismatch (Requirement 2.2)
    - Test positive removal threshold (Requirement 2.3)
    - Test missing file returns `ConfigError::Io` (Requirement 3.3)
    - Test `WorldConfig::default()` matches current hardcoded values (Requirements 4.1, 4.2)
    - Test `BevyExtras::default()` matches current hardcoded Bevy values (Requirement 6.3)

- [ ] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Implement CLI argument parser
  - [ ] 5.1 Create `src/io/cli.rs` with `CliArgs` struct and `parse_cli_args` function
    - Parse `--config <path>` flag and optional positional `<seed>` argument
    - Return `CliArgs { config_path: Option<PathBuf>, seed_override: Option<u64> }`
    - Handle error cases: `--config` without path, non-numeric seed
    - _Requirements: 3.1, 3.2, 3.4_
  - [ ]* 5.2 Write property test: CLI --config path extraction
    - **Property 6: CLI --config path extraction**
    - For any valid path string, parsing `["bin", "--config", path]` produces `config_path == Some(path)`
    - **Validates: Requirements 3.1**
  - [ ]* 5.3 Write property test: CLI seed override precedence
    - **Property 7: CLI seed override precedence**
    - For any seed S, parsing `["bin", "S"]` produces `seed_override == Some(S)`
    - **Validates: Requirements 3.4**
  - [ ]* 5.4 Write unit tests for CLI edge cases
    - Test empty args → `config_path: None, seed_override: None` (Requirement 3.2)
    - Test `--config` without path → `ConfigError::CliError`
    - Test both `--config path` and positional seed together

- [ ] 6. Integrate config loading into both binaries
  - [ ] 6.1 Update `src/main.rs` to use CLI parser and config loader
    - Replace hardcoded config construction with: parse CLI args → load config file (if provided) or use defaults → apply seed override → validate → pass to `world_init::initialize`
    - Preserve backward compatibility: no args = same behavior as before
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.3, 6.1_
  - [ ] 6.2 Update `src/bin/bevy_viz.rs` to use CLI parser and config loader
    - Same flow as terminal binary but use `load_bevy_config` and construct `BevyVizConfig` from the loaded config + `BevyExtras`
    - Preserve backward compatibility: no args = same behavior as before
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.2, 4.4, 6.2, 6.3_

- [ ] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties via `proptest`
- Unit tests validate specific examples and edge cases
- This is entirely COLD path code — allocations and dynamic dispatch are permitted throughout
