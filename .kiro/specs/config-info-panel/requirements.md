# Requirements Document

## Introduction

A toggle-able information panel in the Bevy visualization layer that displays the simulation seed and key world configuration parameters. The panel is hidden by default and toggled via a key press, avoiding visual clutter while keeping configuration data accessible on demand. This is a COLD path feature with zero hot-path impact.

## Glossary

- **Info_Panel**: A Bevy UI text entity that displays formatted simulation configuration data. Hidden by default, toggled visible/hidden by a key press.
- **BevyVizConfig**: The existing Bevy resource holding the seed, grid configuration, world initialization configuration, actor configuration, and visualization parameters.
- **GridConfig**: Configuration struct containing grid dimensions, chemical species count, diffusion rate, thermal conductivity, ambient heat, tick duration, thread count, and per-species chemical decay rates.
- **WorldInitConfig**: Configuration struct containing heat/chemical source generation parameters, initial field value ranges, and actor count ranges.
- **ActorConfig**: Optional configuration struct containing actor metabolism parameters (consumption rate, energy conversion, base decay, initial energy, registry capacity).
- **Panel_Visibility**: A Bevy resource tracking whether the Info_Panel is currently shown or hidden.

## Requirements

### Requirement 1: Toggle Panel Visibility

**User Story:** As a user, I want to press a key to show or hide the configuration info panel, so that I can inspect simulation parameters without permanent screen clutter.

#### Acceptance Criteria

1. WHEN the user presses the `I` key, THE Info_Panel SHALL toggle between visible and hidden states.
2. WHEN the application starts, THE Info_Panel SHALL be hidden by default.
3. WHEN the Info_Panel is toggled, THE Panel_Visibility resource SHALL update in the same frame as the key press.

### Requirement 2: Display Seed and Grid Configuration

**User Story:** As a user, I want to see the simulation seed and grid configuration values in the panel, so that I can verify and reproduce simulation runs.

#### Acceptance Criteria

1. WHILE the Info_Panel is visible, THE Info_Panel SHALL display the simulation seed value.
2. WHILE the Info_Panel is visible, THE Info_Panel SHALL display the GridConfig parameters: width, height, num_chemicals, diffusion_rate, thermal_conductivity, ambient_heat, tick_duration, num_threads, and chemical_decay_rates.
3. WHILE the Info_Panel is visible, THE Info_Panel SHALL display the WorldInitConfig parameters: heat source config ranges, chemical source config ranges, initial heat range, initial concentration range, and actor count range.
4. WHILE the Info_Panel is visible AND an ActorConfig is present, THE Info_Panel SHALL display the ActorConfig parameters: consumption_rate, energy_conversion_factor, base_energy_decay, initial_energy, and initial_actor_capacity.
5. WHILE the Info_Panel is visible AND no ActorConfig is present, THE Info_Panel SHALL display "Actors: disabled" in place of actor configuration parameters.

### Requirement 3: Panel Formatting

**User Story:** As a user, I want the panel text to be clearly formatted with section headers and labeled values, so that I can quickly find specific parameters.

#### Acceptance Criteria

1. THE Info_Panel SHALL organize displayed values under section headers: "Seed", "Grid", "World Init", and "Actors".
2. THE Info_Panel SHALL label each parameter value with its field name.
3. THE Info_Panel SHALL format floating-point values to a consistent decimal precision.

### Requirement 4: Panel Positioning

**User Story:** As a user, I want the info panel positioned so it does not overlap existing UI elements, so that I can read all information without obstruction.

#### Acceptance Criteria

1. THE Info_Panel SHALL be positioned using absolute positioning that does not overlap the overlay label (top-left), rate label (top-right), hover tooltip (bottom-left), or color scale bar (right edge).
2. THE Info_Panel SHALL use a semi-transparent background to maintain readability over the grid visualization.

### Requirement 5: ECS Architecture Compliance

**User Story:** As a developer, I want the info panel to follow existing ECS patterns, so that the codebase remains consistent and maintainable.

#### Acceptance Criteria

1. THE Info_Panel entity SHALL use a marker component for query identification, following the existing pattern of OverlayLabel, RateLabel, and HoverTooltip markers.
2. THE Panel_Visibility resource SHALL be a plain data struct with no business logic methods beyond a toggle operation.
3. THE toggle input system SHALL be a stateless function operating on Bevy resource and component queries.
4. THE panel text formatting SHALL be implemented as a pure function that accepts configuration data and returns a formatted string, testable in isolation without Bevy dependencies.
