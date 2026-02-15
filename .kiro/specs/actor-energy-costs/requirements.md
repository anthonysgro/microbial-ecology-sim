# Requirements Document

## Introduction

This feature completes the energy expenditure mechanics for actors in the simulation. Actors already pay a basal metabolic cost per tick (`base_energy_decay` in `ActorConfig`, subtracted during `run_actor_metabolism`), and actors with energy ≤ 0 are already removed via deferred removal. The remaining gaps are:

1. Movement is free — actors traverse cells with no energy cost.
2. Death is instantaneous — there is no intermediate inert/dormant state before removal.

This feature adds movement energy costs and an inert actor state, creating real survival pressure that rewards efficient foraging over aimless wandering.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, with an internal energy reserve. ECS entity stored in `ActorRegistry`.
- **ActorConfig**: Immutable per-tick configuration struct holding metabolic rates, sensing parameters, and energy constants.
- **Basal_Metabolic_Cost**: Energy subtracted from every Actor each tick regardless of activity. Already implemented as `base_energy_decay` in `ActorConfig`.
- **Movement_Energy_Cost**: Energy subtracted from an Actor when it successfully moves to an adjacent cell.
- **Inert_State**: A dormant condition entered by an Actor whose energy reaches zero. An inert Actor occupies its cell but does not sense, metabolize, or move.
- **Tick_Orchestrator**: The `TickOrchestrator::step` function that sequences all simulation phases per tick.
- **Removal_Buffer**: Pre-allocated `Vec<ActorId>` used for deferred removal of dead actors after iteration completes.

## Requirements

### Requirement 1: Movement Energy Cost

**User Story:** As a simulation designer, I want actors to spend energy when they move, so that movement creates a real trade-off against staying put and conserving energy.

#### Acceptance Criteria

1. WHEN an Actor successfully moves to an adjacent cell, THE Movement_System SHALL subtract the configured `movement_cost` from the Actor's energy reserve.
2. WHEN an Actor does not move during a tick (target cell occupied or no movement target), THE Movement_System SHALL leave the Actor's energy unchanged by movement cost.
3. THE ActorConfig SHALL contain a `movement_cost` field of type `f32` representing energy spent per cell traversed.
4. WHEN the Actor's energy after movement cost subtraction is less than or equal to zero, THE Movement_System SHALL mark the Actor as inert.

### Requirement 2: Inert Actor State

**User Story:** As a simulation designer, I want actors that run out of energy to become inert before being removed, so that energy depletion has a visible intermediate state and other actors can interact with dormant organisms.

#### Acceptance Criteria

1. WHEN an Actor's energy reaches zero or below during metabolism, THE Metabolism_System SHALL mark the Actor as inert instead of immediately scheduling removal.
2. WHILE an Actor is inert, THE Sensing_System SHALL skip the Actor during gradient sensing.
3. WHILE an Actor is inert, THE Metabolism_System SHALL continue to subtract Basal_Metabolic_Cost from the Actor each tick but SHALL NOT consume chemicals from the environment.
4. WHILE an Actor is inert, THE Movement_System SHALL skip the Actor during movement.
5. WHEN an inert Actor's energy falls below a configured `removal_threshold` (a negative value), THE Metabolism_System SHALL schedule the Actor for deferred removal.
6. THE ActorConfig SHALL contain a `removal_threshold` field of type `f32` representing the energy level below which an inert Actor is permanently removed.
7. THE Actor struct SHALL contain an `inert` field of type `bool` indicating whether the Actor is in the inert state.

### Requirement 3: Energy Accounting Determinism

**User Story:** As a simulation engineer, I want all energy deductions to be deterministic and numerically validated, so that simulation replays produce identical results.

#### Acceptance Criteria

1. THE Movement_System SHALL process actors in deterministic slot-index order when applying movement energy costs.
2. IF an Actor's energy becomes NaN or infinite after any energy deduction, THEN THE system that performed the deduction SHALL return a `TickError::NumericalError`.
3. THE Tick_Orchestrator SHALL apply energy costs in a fixed phase order: basal metabolism first, then movement cost, with no reordering between ticks.

### Requirement 4: Configuration Validation

**User Story:** As a simulation operator, I want invalid energy cost configurations to be rejected at construction time, so that misconfigured simulations fail fast rather than producing nonsensical results.

#### Acceptance Criteria

1. WHEN `movement_cost` is negative, THE Grid construction SHALL return an error.
2. WHEN `removal_threshold` is positive, THE Grid construction SHALL return an error.
3. WHEN `base_energy_decay` is negative, THE Grid construction SHALL return an error.
