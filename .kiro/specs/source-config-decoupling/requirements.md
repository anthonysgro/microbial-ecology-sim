# Requirements Document

## Introduction

The current `WorldInitConfig` uses a single set of parameters (source count range, emission rate range, renewable fraction, reservoir capacity range, deceleration threshold range) shared by both heat sources and chemical sources. This coupling prevents independent tuning of heat vs. chemical source characteristics during world initialization. Heat and chemical are distinct physical fundamentals — they should be independently configurable. This feature extracts all source-generation parameters into a reusable per-field-type config struct, so that each fundamental has its own complete configuration. The design should make it straightforward to add new fundamentals in the future by adding another instance of the same config struct.

## Glossary

- **WorldInitConfig**: Top-level configuration struct controlling procedural world generation ranges and parameters.
- **SourceFieldConfig**: New sub-configuration struct holding the complete set of source-generation parameters for a single field type (e.g., heat or chemical). Reusable for any future fundamental.
- **Source**: A persistent emitter that injects a value into a grid field each tick. Defined in `src/grid/source.rs`.
- **SourceField**: Enum discriminating between `Heat` and `Chemical(usize)` source types.
- **Fundamental**: A distinct physical field type in the simulation (currently heat and chemical). Each fundamental has its own independent source configuration.
- **Renewable Source**: A source with infinite reservoir (`f32::INFINITY`). Emits indefinitely at full rate.
- **Finite Source**: A source with a bounded reservoir that depletes over time.
- **Emission Rate**: The base rate (units per tick) at which a source injects into its target field.
- **Renewable Fraction**: The probability that a generated source is renewable vs. finite, in `[0.0, 1.0]`.
- **Reservoir Capacity**: The total emittable quantity assigned to a finite source at creation.
- **Deceleration Threshold**: The fraction of initial capacity below which a finite source's emission rate begins to taper.

## Requirements

### Requirement 1: Introduce a reusable per-field-type source configuration struct

**User Story:** As a simulation designer, I want all source-generation parameters grouped into a self-contained struct per field type, so that each fundamental is independently configurable and new fundamentals can be added by reusing the same struct.

#### Acceptance Criteria

1. THE SourceFieldConfig SHALL include fields for: source count range (`min_sources`, `max_sources`), emission rate range (`min_emission_rate`, `max_emission_rate`), `renewable_fraction`, reservoir capacity range (`min_reservoir_capacity`, `max_reservoir_capacity`), and deceleration threshold range (`min_deceleration_threshold`, `max_deceleration_threshold`).
2. THE WorldInitConfig SHALL contain a `heat_source_config` field of type `SourceFieldConfig` that controls all heat source generation parameters.
3. THE WorldInitConfig SHALL contain a `chemical_source_config` field of type `SourceFieldConfig` that controls all chemical source generation parameters.
4. THE WorldInitConfig SHALL no longer contain the shared fields `min_heat_sources`, `max_heat_sources`, `min_chemical_sources`, `max_chemical_sources`, `min_emission_rate`, `max_emission_rate`, `renewable_fraction`, `min_reservoir_capacity`, `max_reservoir_capacity`, `min_deceleration_threshold`, `max_deceleration_threshold`.
5. THE SourceFieldConfig SHALL be a plain data struct with `Debug`, `Clone`, and `PartialEq` derives, suitable for reuse by any future fundamental.

### Requirement 2: Validate each SourceFieldConfig independently

**User Story:** As a simulation designer, I want each field type's configuration validated independently, so that an invalid heat config does not mask a valid chemical config and error messages identify which field type failed.

#### Acceptance Criteria

1. WHEN `validate_config` is called, THE validation system SHALL validate the heat `SourceFieldConfig` and the chemical `SourceFieldConfig` independently.
2. IF a `SourceFieldConfig` has `min_sources > max_sources`, THEN THE validation system SHALL return an `InvalidRange` error identifying the field type and range name.
3. IF a `SourceFieldConfig` has `min_emission_rate > max_emission_rate`, THEN THE validation system SHALL return an `InvalidRange` error identifying the field type and range name.
4. IF a `SourceFieldConfig` has `renewable_fraction` outside `[0.0, 1.0]`, THEN THE validation system SHALL return an `InvalidConfig` error identifying the field type.
5. IF a `SourceFieldConfig` has `min_reservoir_capacity <= 0.0`, THEN THE validation system SHALL return an `InvalidConfig` error identifying the field type.
6. IF a `SourceFieldConfig` has `max_reservoir_capacity < min_reservoir_capacity`, THEN THE validation system SHALL return an `InvalidRange` error identifying the field type.
7. IF a `SourceFieldConfig` has `min_deceleration_threshold` or `max_deceleration_threshold` outside `[0.0, 1.0]`, THEN THE validation system SHALL return an `InvalidConfig` error identifying the field type.
8. IF a `SourceFieldConfig` has `max_deceleration_threshold < min_deceleration_threshold`, THEN THE validation system SHALL return an `InvalidRange` error identifying the field type.

### Requirement 3: Generate sources using per-field-type configuration

**User Story:** As a simulation designer, I want heat sources generated exclusively from the heat config and chemical sources generated exclusively from the chemical config, so that each fundamental reflects its own independent parameters.

#### Acceptance Criteria

1. WHEN generating heat sources, THE `generate_sources` function SHALL sample source count, emission rate, renewable fraction, reservoir capacity, and deceleration threshold exclusively from `heat_source_config`.
2. WHEN generating chemical sources, THE `generate_sources` function SHALL sample source count, emission rate, renewable fraction, reservoir capacity, and deceleration threshold exclusively from `chemical_source_config`.
3. WHEN heat and chemical `SourceFieldConfig` values differ, THE generated heat sources and chemical sources SHALL reflect their respective independent configurations.

### Requirement 4: Preserve backward-compatible defaults

**User Story:** As a developer, I want `WorldInitConfig::default()` to produce the same initialization behavior as before the refactor, so that existing code using defaults is unaffected.

#### Acceptance Criteria

1. THE `Default` implementation for `SourceFieldConfig` SHALL provide values matching the previous shared defaults (emission rate `[0.1, 5.0]`, renewable fraction `0.3`, reservoir capacity `[50.0, 200.0]`, deceleration threshold `[0.1, 0.5]`).
2. THE `Default` implementation for `WorldInitConfig` SHALL set `heat_source_config` with source count `[1, 5]` and all other fields from `SourceFieldConfig::default()`.
3. THE `Default` implementation for `WorldInitConfig` SHALL set `chemical_source_config` with source count `[1, 3]` and all other fields from `SourceFieldConfig::default()`.
4. WHEN using `WorldInitConfig::default()` with the same seed, THE initialization system SHALL produce identical source registries and field buffers as the previous implementation.

### Requirement 5: Update all call sites

**User Story:** As a developer, I want all existing consumers of `WorldInitConfig` updated to use the new struct layout, so that the codebase compiles and behaves correctly after the refactor.

#### Acceptance Criteria

1. WHEN `main.rs` constructs a `WorldInitConfig`, THE construction SHALL use the new `heat_source_config` and `chemical_source_config` fields.
2. WHEN `sample_reservoir_params` is called, THE function SHALL accept a `SourceFieldConfig` reference instead of a `WorldInitConfig` reference.
3. THE codebase SHALL compile without errors or warnings after the refactor.
