# Requirements Document

## Introduction

This feature adds interactive simulation rate control to the Bevy visualization layer. The headless simulation currently advances at a fixed rate determined by `tick_hz` at startup. This feature introduces a Bevy resource that governs simulation pacing — allowing the user to pause, resume, speed up, and slow down the simulation via keyboard input. The simulation tick logic itself (`TickOrchestrator`) remains unchanged; only the Bevy scheduling layer is affected.

## Glossary

- **Simulation_Rate_Controller**: A Bevy resource that holds the current simulation rate state (ticks per second, paused flag) and exposes methods to modify pacing.
- **FixedUpdate_Timestep**: The Bevy `Time<Fixed>` resource that controls how frequently `FixedUpdate` systems execute. Derived from the current ticks-per-second value.
- **Tick_Hz**: The number of simulation ticks executed per second. A positive `f64` value that maps to the `FixedUpdate` period.
- **Pause_State**: A boolean flag indicating whether the simulation is paused by the user (distinct from error-halted via `SimulationState.running`).
- **Rate_Label**: A UI text element displaying the current simulation rate and pause status to the user.

## Requirements

### Requirement 1: Simulation Rate Resource

**User Story:** As a developer observing the simulation, I want a dedicated resource tracking simulation pacing state, so that rate control logic is decoupled from the existing `SimulationState` and `BevyVizConfig`.

#### Acceptance Criteria

1. THE Simulation_Rate_Controller SHALL store the current Tick_Hz as a positive `f64` value
2. THE Simulation_Rate_Controller SHALL store the Pause_State as a boolean flag independent of `SimulationState.running`
3. THE Simulation_Rate_Controller SHALL store the initial Tick_Hz value configured at startup for reset purposes
4. WHEN the Bevy app starts, THE Simulation_Rate_Controller SHALL be initialized from the `tick_hz` field of `BevyVizConfig`

### Requirement 2: Pause and Resume

**User Story:** As a developer observing the simulation, I want to pause and resume the simulation with a single key press, so that I can freeze the simulation to inspect state without losing my place.

#### Acceptance Criteria

1. WHEN the user presses the Space key, THE Simulation_Rate_Controller SHALL toggle the Pause_State between paused and unpaused
2. WHILE the Pause_State is paused, THE tick_simulation system SHALL skip tick advancement regardless of the FixedUpdate schedule
3. WHEN the Pause_State transitions from paused to unpaused, THE tick_simulation system SHALL resume advancing ticks at the current Tick_Hz rate
4. WHILE the Pause_State is paused, THE tick_simulation system SHALL leave `SimulationState.tick` unchanged

### Requirement 3: Speed Up

**User Story:** As a developer observing the simulation, I want to increase the simulation rate, so that I can fast-forward through uninteresting periods.

#### Acceptance Criteria

1. WHEN the user presses the Up Arrow key, THE Simulation_Rate_Controller SHALL multiply the current Tick_Hz by 2.0
2. WHEN the Tick_Hz would exceed a maximum of 480.0 after a speed increase, THE Simulation_Rate_Controller SHALL clamp Tick_Hz to 480.0
3. WHEN the Tick_Hz changes, THE FixedUpdate_Timestep SHALL be updated to reflect the new rate within the same frame

### Requirement 4: Slow Down

**User Story:** As a developer observing the simulation, I want to decrease the simulation rate, so that I can observe fine-grained behavior in slow motion.

#### Acceptance Criteria

1. WHEN the user presses the Down Arrow key, THE Simulation_Rate_Controller SHALL divide the current Tick_Hz by 2.0
2. WHEN the Tick_Hz would fall below a minimum of 0.5 after a speed decrease, THE Simulation_Rate_Controller SHALL clamp Tick_Hz to 0.5
3. WHEN the Tick_Hz changes, THE FixedUpdate_Timestep SHALL be updated to reflect the new rate within the same frame

### Requirement 5: Reset to Default Rate

**User Story:** As a developer observing the simulation, I want to reset the simulation rate to its initial value, so that I can quickly return to the baseline after experimenting with speed changes.

#### Acceptance Criteria

1. WHEN the user presses the R key, THE Simulation_Rate_Controller SHALL set Tick_Hz to the initial value stored at startup
2. WHEN the rate is reset, THE FixedUpdate_Timestep SHALL be updated to reflect the initial Tick_Hz within the same frame

### Requirement 6: Rate Display Label

**User Story:** As a developer observing the simulation, I want to see the current simulation rate and pause status on screen, so that I have immediate feedback on the simulation pacing.

#### Acceptance Criteria

1. THE Rate_Label SHALL display the current Tick_Hz value and the text "PAUSED" when the simulation is paused
2. WHEN the Tick_Hz or Pause_State changes, THE Rate_Label SHALL update its displayed text within the same frame
3. THE Rate_Label SHALL be positioned in a non-overlapping location relative to existing UI elements (overlay label, tooltip, scale bar)

### Requirement 7: Interaction with Error Halt

**User Story:** As a developer observing the simulation, I want rate controls to respect the existing error-halt mechanism, so that pausing and resuming do not mask simulation errors.

#### Acceptance Criteria

1. WHILE `SimulationState.running` is false (error-halted), THE tick_simulation system SHALL skip tick advancement regardless of Pause_State
2. WHILE `SimulationState.running` is false, THE Rate_Label SHALL display "HALTED" to distinguish error state from user pause
