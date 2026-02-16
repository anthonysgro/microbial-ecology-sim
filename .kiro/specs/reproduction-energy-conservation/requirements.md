# Requirements Document

## Introduction

The `run_actor_reproduction` system contains an energy conservation violation: during binary fission, the parent actor is only charged `reproduction_cost` but the offspring spawns with `offspring_energy` units of energy. The offspring energy is created from nothing, violating the simulation's conservation invariant. This spec corrects the energy accounting so that the parent pays the full cost of fission: `reproduction_cost + offspring_energy`.

## Glossary

- **Reproduction_System**: The `run_actor_reproduction` function in `src/grid/actor_systems.rs` that handles binary fission for all actors each tick.
- **Parent**: The actor undergoing binary fission.
- **Offspring**: The new actor created by fission, placed in an adjacent cell.
- **reproduction_cost**: A heritable trait representing the pure overhead/entropy cost of fission (energy destroyed, not transferred).
- **offspring_energy**: A heritable trait representing the energy transferred from parent to offspring at fission.
- **Total_Fission_Cost**: The sum `reproduction_cost + offspring_energy`, representing the total energy deducted from the parent during fission.
- **Energy_Conservation**: The invariant that total system energy before a fission event equals total system energy after, minus `reproduction_cost` (the entropy term).

## Requirements

### Requirement 1: Correct Parent Energy Deduction

**User Story:** As a simulation engineer, I want the parent actor to be charged the full cost of fission (reproduction_cost + offspring_energy), so that energy is conserved across reproduction events.

#### Acceptance Criteria

1. WHEN a parent actor undergoes binary fission, THE Reproduction_System SHALL deduct `reproduction_cost + offspring_energy` from the parent's energy.
2. WHEN a parent actor undergoes binary fission, THE Reproduction_System SHALL spawn the offspring with exactly `offspring_energy` units of energy (unchanged from current behavior).
3. FOR ALL fission events, the total energy in the system after fission SHALL equal the total energy before fission minus `reproduction_cost` (the entropy/overhead term).

### Requirement 2: Energy Gate Consistency

**User Story:** As a simulation engineer, I want the energy gate check to remain consistent with the actual deduction, so that actors are never driven to negative energy by reproduction.

#### Acceptance Criteria

1. THE Reproduction_System SHALL retain the existing energy gate: `actor.energy >= reproduction_cost + offspring_energy`.
2. WHEN the energy gate passes and fission occurs, THE Reproduction_System SHALL deduct exactly the same amount checked by the gate (`reproduction_cost + offspring_energy`).

### Requirement 3: Comment and Documentation Accuracy

**User Story:** As a developer, I want the code comments to accurately describe the energy accounting, so that the conservation invariant is clear to future maintainers.

#### Acceptance Criteria

1. WHEN the energy deduction line is modified, THE Reproduction_System SHALL include a comment explaining that `reproduction_cost` is the entropy/overhead cost and `offspring_energy` is the energy transferred to the offspring.
2. THE Reproduction_System SHALL include a comment stating the conservation invariant: `parent_energy_before = parent_energy_after + reproduction_cost + offspring_energy`.

### Requirement 4: Existing Test Compatibility

**User Story:** As a developer, I want existing tests to be updated to reflect the corrected energy accounting, so that the test suite validates the fix.

#### Acceptance Criteria

1. IF existing reproduction-related tests assert on parent energy after fission, THEN THE test assertions SHALL be updated to expect the corrected deduction amount (`reproduction_cost + offspring_energy`).
2. WHEN a new test is added for the fix, THE test SHALL verify that parent energy after fission equals `energy_before - reproduction_cost - offspring_energy`.
