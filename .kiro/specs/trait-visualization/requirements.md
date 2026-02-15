# Requirements Document

## Introduction

This feature adds two visualization capabilities to the Bevy-based 2D grid simulation viewer: a toggleable population statistics panel showing live aggregate trait distributions across all living actors, and a click-to-select actor inspection mechanism that displays individual actor details and highlights the selected actor on the grid. Both features are purely visualization-side — no changes to the simulation core (`src/grid/`).

## Glossary

- **Stats_Panel**: A toggleable UI text panel displaying population-level aggregate statistics for heritable traits, toggled by the `T` key.
- **Trait_Stats**: A pre-computed Bevy resource holding min, max, mean, and percentile (p25, p50, p75) values for each of the four heritable traits across all living actors.
- **Actor_Inspector**: A UI text panel displaying the full state of a single selected actor (traits, energy, position, active/inert status).
- **Selected_Actor**: A Bevy resource tracking which actor (by slot index) is currently selected for inspection, or `None` if no actor is selected.
- **Highlight**: A distinct pixel color (e.g., cyan) rendered in place of the default white for the currently selected actor's cell in the grid texture.
- **Living_Actor**: An actor in the `ActorRegistry` that is not inert (`actor.inert == false`). Inert actors are excluded from population statistics but visible in the inspector if selected.
- **Heritable_Traits**: The four `f32` fields on each actor: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`.

## Requirements

### Requirement 1: Population Statistics Computation

**User Story:** As a simulation observer, I want population-level trait statistics computed each tick, so that I can monitor evolutionary drift in real time.

#### Acceptance Criteria

1. WHEN a simulation tick completes, THE Trait_Stats resource SHALL be recomputed from all Living_Actors in the ActorRegistry.
2. THE Trait_Stats resource SHALL contain min, max, mean, p25, p50, and p75 values for each of the four Heritable_Traits fields.
3. THE Trait_Stats resource SHALL contain the total count of Living_Actors and the current tick number.
4. WHEN zero Living_Actors exist, THE Trait_Stats resource SHALL report a count of zero and omit statistical values.
5. WHEN exactly one Living_Actor exists, THE Trait_Stats resource SHALL report min, max, mean, p25, p50, and p75 all equal to that actor's trait values.

### Requirement 2: Population Statistics Panel Display

**User Story:** As a simulation observer, I want a toggleable panel showing live trait statistics, so that I can watch population dynamics without cluttering the view.

#### Acceptance Criteria

1. WHEN the user presses the `T` key, THE Stats_Panel SHALL toggle between visible and hidden states.
2. WHILE the Stats_Panel is visible, THE Stats_Panel SHALL display the current Trait_Stats values formatted with trait names, stat labels, and numeric values to two decimal places.
3. WHILE the Stats_Panel is visible, THE Stats_Panel SHALL display the total Living_Actor count and current tick number.
4. WHEN the Trait_Stats resource changes, THE Stats_Panel text SHALL update to reflect the new values in the same frame.
5. THE Stats_Panel SHALL be positioned so it does not overlap with the existing overlay label, rate label, or hover tooltip.
6. THE Stats_Panel SHALL render with a semi-transparent dark background for readability against the grid.

### Requirement 3: Actor Selection

**User Story:** As a simulation observer, I want to click on an actor to select it, so that I can inspect its individual state.

#### Acceptance Criteria

1. WHEN the user left-clicks on a grid cell occupied by an actor, THE Selected_Actor resource SHALL store that actor's slot index.
2. WHEN the user left-clicks on an empty grid cell, THE Selected_Actor resource SHALL be cleared to `None`.
3. WHEN the user presses the `Escape` key while an actor is selected, THE Selected_Actor resource SHALL be cleared to `None`.
4. WHEN the selected actor is removed from the simulation (death/removal), THE Selected_Actor resource SHALL be cleared to `None`.
5. THE click-to-select system SHALL reuse the existing cursor-to-grid-cell coordinate mapping from the hover tooltip system.

### Requirement 4: Actor Inspector Panel Display

**User Story:** As a simulation observer, I want to see the full state of a selected actor, so that I can understand individual behavior and trait values.

#### Acceptance Criteria

1. WHILE an actor is selected, THE Actor_Inspector panel SHALL display the actor's four Heritable_Traits values formatted to four decimal places.
2. WHILE an actor is selected, THE Actor_Inspector panel SHALL display the actor's energy (two decimal places), grid position (column, row), and state (active or inert).
3. WHEN no actor is selected, THE Actor_Inspector panel SHALL be hidden.
4. WHEN the selected actor's state changes between ticks, THE Actor_Inspector panel SHALL reflect the updated values.
5. THE Actor_Inspector panel SHALL be positioned so it does not overlap with the Stats_Panel, overlay label, rate label, or hover tooltip.
6. THE Actor_Inspector panel SHALL render with a semi-transparent dark background for readability.

### Requirement 5: Selected Actor Highlight

**User Story:** As a simulation observer, I want the selected actor to be visually distinct on the grid, so that I can track its position.

#### Acceptance Criteria

1. WHILE an actor is selected, THE update_texture system SHALL render the selected actor's cell in a distinct highlight color (cyan) instead of the default white.
2. WHEN no actor is selected, THE update_texture system SHALL render all actor cells in the default white color.
3. WHEN the selected actor moves to a different cell, THE Highlight SHALL follow the actor to the new cell on the next frame.

### Requirement 6: Input Key Non-Interference

**User Story:** As a simulation observer, I want the new keybindings to coexist with existing controls, so that no functionality is broken.

#### Acceptance Criteria

1. THE `T` key binding for Stats_Panel toggle SHALL NOT interfere with existing key bindings (H, 1-9, I, Space, Arrow keys, R, Escape, Q).
2. THE left-click actor selection SHALL NOT interfere with existing middle-click camera panning.
3. WHEN the `Escape` key is pressed while an actor is selected, THE system SHALL deselect the actor and SHALL NOT trigger application exit.
4. WHEN the `Escape` key is pressed while no actor is selected, THE system SHALL trigger application exit as before.
