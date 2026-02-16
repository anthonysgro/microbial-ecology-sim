# Requirements Document

## Introduction

The simulation currently applies a flat additive thermal cost (`thermal_sensitivity * (cell_heat - optimal_temp)²`) during metabolism. This cost is negligible relative to other energy flows, so actors freely traverse hostile thermal zones with no meaningful penalty. Movement cost, consumption efficiency, and sensing are all thermally agnostic.

This feature introduces a **thermal fitness factor** — a multiplicative scalar in `[0.0, 1.0]` derived from the actor's thermal mismatch — that degrades multiple actor capabilities when the actor operates outside its thermal comfort zone. The biological analogy is enzyme kinetics: membrane fluidity, ATP production, and protein folding are all temperature-dependent. An organism doesn't "decide" to avoid hostile temperatures — it physically cannot function well there.

The existing `thermal_sensitivity` config parameter and `optimal_temp` heritable trait are reused. The current additive thermal drain remains as a baseline energy cost. The new multiplicative fitness factor is layered on top, scaling consumption efficiency, movement cost, and optionally sensing acuity.

Expected emergent behavior: actors naturally cluster in thermally comfortable zones, `optimal_temp` becomes a meaningful heritable trait under selective pressure, and thermal niche partitioning emerges across the grid.

## Glossary

- **Thermal_Fitness_Factor**: A multiplicative scalar in `[0.0, 1.0]` computed from the squared thermal mismatch between cell heat and the actor's heritable `optimal_temp`, scaled by a configurable width parameter. Value of `1.0` at zero mismatch, decaying toward `0.0` as mismatch increases.
- **Thermal_Mismatch**: The absolute difference between the cell's heat value and the actor's heritable `optimal_temp` trait: `|cell_heat - optimal_temp|`.
- **Thermal_Fitness_Width**: A configurable parameter controlling how quickly the Thermal_Fitness_Factor decays with increasing Thermal_Mismatch. Larger values produce a wider comfort zone (more tolerant actors); smaller values produce a narrower comfort zone (more sensitive actors).
- **Actor**: A mobile biological agent occupying one grid cell, with internal energy reserves and heritable traits.
- **Metabolism_System**: The HOT-path system (`run_actor_metabolism`) that computes per-tick energy balance for all actors.
- **Movement_System**: The HOT-path system (`run_actor_movement`) that executes actor movement and deducts movement energy costs.
- **Sensing_System**: The WARM-path system (`run_actor_sensing`) that computes movement targets from chemical gradients and Lévy flight tumble state.
- **ActorConfig**: The configuration struct holding all actor-related parameters, loaded from TOML.
- **HeritableTraits**: The per-actor struct of heritable trait values inherited during fission with proportional mutation.

## Requirements

### Requirement 1: Thermal Fitness Factor Computation

**User Story:** As a simulation engine, I want to compute a thermal fitness factor for each actor based on its thermal mismatch, so that actor capabilities degrade smoothly in hostile thermal zones.

#### Acceptance Criteria

1. THE Thermal_Fitness_Factor computation SHALL produce a value of `1.0` WHEN the cell heat equals the actor's `optimal_temp`
2. WHEN the Thermal_Mismatch increases, THE Thermal_Fitness_Factor SHALL monotonically decrease toward `0.0`
3. THE Thermal_Fitness_Factor SHALL always produce a value in the closed interval `[0.0, 1.0]` for all finite inputs
4. THE Thermal_Fitness_Factor computation SHALL use the formula `exp(-thermal_mismatch² / (2 * thermal_fitness_width²))` (Gaussian decay)
5. THE Thermal_Fitness_Factor computation SHALL be a pure function with no heap allocation, suitable for HOT-path execution on every actor every tick

### Requirement 2: Thermal Fitness Degrades Consumption Efficiency

**User Story:** As a simulation designer, I want consumption efficiency to scale with thermal fitness, so that actors in hostile thermal zones extract less energy from the same chemical concentration.

#### Acceptance Criteria

1. WHEN an actor consumes chemical during metabolism, THE Metabolism_System SHALL multiply the effective energy conversion by the actor's Thermal_Fitness_Factor
2. WHEN the Thermal_Fitness_Factor is `1.0`, THE Metabolism_System SHALL produce identical energy gain to the current behavior (no regression)
3. WHEN the Thermal_Fitness_Factor is close to `0.0`, THE Metabolism_System SHALL produce near-zero energy gain from consumption regardless of chemical availability
4. THE Metabolism_System SHALL continue to apply the existing additive thermal cost (`thermal_sensitivity * mismatch²`) in addition to the multiplicative fitness scaling

### Requirement 3: Thermal Fitness Increases Movement Cost

**User Story:** As a simulation designer, I want movement cost to increase under thermal stress, so that locomotion becomes more expensive for actors outside their thermal comfort zone.

#### Acceptance Criteria

1. WHEN an actor moves to a new cell, THE Movement_System SHALL divide the base movement cost by the actor's Thermal_Fitness_Factor, increasing the effective cost under thermal stress
2. WHEN the Thermal_Fitness_Factor is `1.0`, THE Movement_System SHALL produce identical movement cost to the current behavior (no regression)
3. WHEN the Thermal_Fitness_Factor approaches `0.0`, THE Movement_System SHALL cap the movement cost multiplier at a configurable maximum (`thermal_movement_cap`) to prevent infinite cost
4. IF the Thermal_Fitness_Factor is exactly `0.0`, THEN THE Movement_System SHALL apply the capped maximum movement cost instead of dividing by zero

### Requirement 4: Configuration Parameters

**User Story:** As a simulation operator, I want to configure the thermal fitness parameters via TOML, so that I can tune the strength and shape of thermal fitness effects.

#### Acceptance Criteria

1. THE ActorConfig SHALL include a `thermal_fitness_width` field (f32, default `0.5`) controlling the Gaussian decay width of the Thermal_Fitness_Factor
2. THE ActorConfig SHALL include a `thermal_movement_cap` field (f32, default `5.0`) specifying the maximum movement cost multiplier when Thermal_Fitness_Factor approaches zero
3. WHEN `thermal_fitness_width` is set to `0.0`, THE Thermal_Fitness_Factor SHALL be `1.0` for all actors (disabling the mechanic)
4. THE ActorConfig SHALL validate that `thermal_fitness_width` is `>= 0.0` and finite
5. THE ActorConfig SHALL validate that `thermal_movement_cap` is `> 1.0` and finite
6. THE `example_config.toml` file SHALL include the new configuration fields with explanatory comments
7. THE Bevy config info panel (`format_config_info`) SHALL display the new configuration fields

### Requirement 5: Documentation Updates

**User Story:** As a project maintainer, I want all documentation artifacts to reflect the new thermal fitness parameters, so that the configuration reference stays in sync with the code.

#### Acceptance Criteria

1. THE `config-documentation.md` steering file SHALL be updated with the new `thermal_fitness_width` and `thermal_movement_cap` fields in the ActorConfig reference table
2. THE `example_config.toml` SHALL document the new fields under the existing "Thermal Metabolism" comment section

### Requirement 6: Determinism and HOT-Path Compliance

**User Story:** As a systems engineer, I want the thermal fitness computation to comply with HOT-path constraints, so that simulation performance and determinism are preserved.

#### Acceptance Criteria

1. THE Thermal_Fitness_Factor computation SHALL perform zero heap allocations
2. THE Thermal_Fitness_Factor computation SHALL produce identical results for identical inputs across all platforms (deterministic)
3. THE Metabolism_System and Movement_System modifications SHALL preserve the existing deterministic iteration order (ascending slot index)
4. IF the Thermal_Fitness_Factor computation produces NaN or infinity for any input, THEN THE Metabolism_System SHALL return a `TickError::NumericalError`
