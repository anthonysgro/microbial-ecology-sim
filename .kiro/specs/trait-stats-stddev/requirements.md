# Requirements Document

## Introduction

Add standard deviation (std dev) to the population trait statistics displayed in the Bevy visualization stats panel. Currently, the stats panel shows min, p25, p50, p75, max, and mean for each heritable trait and energy. This feature extends the `SingleTraitStats` struct with a `std_dev` field, computes it during the existing stats pass, and displays it alongside the existing statistics.

## Glossary

- **Stats_Panel**: The Bevy GUI text panel toggled by the `S` key that displays population-level statistics for heritable traits and energy.
- **SingleTraitStats**: The plain data struct in `src/viz_bevy/resources.rs` that holds per-trait aggregate statistics (min, max, mean, percentiles).
- **Compute_Stats_System**: The `compute_trait_stats_from_actors` function in `src/viz_bevy/systems.rs` that collects trait values from living actors and computes aggregate statistics.
- **Format_Stats**: The `format_trait_stats` function in `src/viz_bevy/setup.rs` that renders `TraitStats` into the displayable stats panel string.
- **Standard_Deviation**: The population standard deviation, computed as `sqrt(mean((x - mean)^2))` over all living actors for a given trait.

## Requirements

### Requirement 1: Extend SingleTraitStats with Standard Deviation

**User Story:** As a simulation observer, I want to see the standard deviation of each trait in the stats panel, so that I can gauge population diversity and convergence at a glance.

#### Acceptance Criteria

1. THE SingleTraitStats SHALL include a `std_dev` field of type `f32`
2. WHEN the Compute_Stats_System computes statistics for a trait, THE Compute_Stats_System SHALL calculate the population standard deviation using the formula `sqrt(sum((x_i - mean)^2) / n)` where `n` is the number of living actors
3. WHEN only one living actor exists, THE Compute_Stats_System SHALL set `std_dev` to `0.0`

### Requirement 2: Display Standard Deviation in the Stats Panel

**User Story:** As a simulation observer, I want the standard deviation displayed in the stats panel row for each trait and for energy, so that I can read it alongside the existing statistics.

#### Acceptance Criteria

1. WHEN the Stats_Panel renders a trait row, THE Format_Stats SHALL include the `std_dev` value in the formatted output line
2. WHEN the Stats_Panel renders the energy row, THE Format_Stats SHALL include the energy `std_dev` value in the formatted output line
3. THE Format_Stats SHALL display `std_dev` with the same decimal precision (two decimal places) as the existing statistics

### Requirement 3: Documentation Update

**User Story:** As a project maintainer, I want the configuration documentation steering file updated to reflect the new `std_dev` field, so that the documentation stays in sync with the code.

#### Acceptance Criteria

1. WHEN the `SingleTraitStats` struct is modified, THE config-documentation steering file SHALL be updated to document the new `std_dev` field in the `SingleTraitStats` description
