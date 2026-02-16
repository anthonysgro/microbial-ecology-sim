# Design Document: Reproduction Energy Conservation Fix

## Overview

The `run_actor_reproduction` function in `src/grid/actor_systems.rs` has an energy conservation bug. The energy gate correctly checks `actor.energy >= reproduction_cost + offspring_energy`, but the deduction line only subtracts `reproduction_cost`. The offspring then spawns with `offspring_energy` units of energy created from nothing.

The fix is a single-line change: replace `actor.energy -= actor.traits.reproduction_cost` with `actor.energy -= actor.traits.reproduction_cost + actor.traits.offspring_energy`.

After the fix, the energy accounting for each fission event is:
- Parent loses: `reproduction_cost + offspring_energy`
- Offspring gains: `offspring_energy`
- Net system energy change: `-reproduction_cost` (the entropy/overhead term)

## Architecture

No architectural changes. The fix is confined to a single line in `run_actor_reproduction`. The function's signature, the spawn buffer protocol, and the deferred spawn system are all unchanged.

The existing tick phasing is unaffected:
1. `run_actor_reproduction` (HOT) — scans actors, deducts energy, fills spawn buffer
2. `run_deferred_spawn` (WARM) — materializes offspring from spawn buffer

The spawn buffer tuple `(cell_index, energy, parent_traits)` continues to carry `offspring_energy` as the second element. The only change is that the parent's energy is reduced by the correct total amount before the tuple is pushed.

## Components and Interfaces

### Modified: `run_actor_reproduction`

**File:** `src/grid/actor_systems.rs`

**Current (buggy):**
```rust
actor.energy -= actor.traits.reproduction_cost;
```

**Fixed:**
```rust
// Energy conservation: deduct both the entropy cost (reproduction_cost)
// and the energy transferred to the offspring (offspring_energy).
// Invariant: parent_before = parent_after + reproduction_cost + offspring_energy
actor.energy -= actor.traits.reproduction_cost + actor.traits.offspring_energy;
```

No other functions, interfaces, or data structures change.

### Unchanged: `run_deferred_spawn`

The spawn buffer already carries `actor.traits.offspring_energy` as the energy value for the offspring. No changes needed.

### Unchanged: `run_actor_metabolism`

The readiness cost formula already correctly uses `reproduction_cost + offspring_energy` as `reproductive_investment`. No changes needed.

## Data Models

No data model changes. `Actor`, `HeritableTraits`, and `ActorConfig` are unchanged.

The energy flow for a fission event becomes:

```
Before:  parent.energy = E
After:   parent.energy = E - reproduction_cost - offspring_energy
         offspring.energy = offspring_energy
         energy_destroyed = reproduction_cost
         
Conservation check: E = (E - rc - oe) + oe + rc  ✓
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Fission energy conservation

*For any* actor with energy above the fission threshold (`reproduction_cost + offspring_energy`) and an available adjacent cell, after a successful fission event, the sum of the parent's remaining energy and the offspring's energy SHALL equal the parent's pre-fission energy minus `reproduction_cost`.

This subsumes gate-deduction consistency (Req 2.2): if conservation holds and offspring receives `offspring_energy`, then the parent deduction must equal `reproduction_cost + offspring_energy`.

**Validates: Requirements 1.1, 1.2, 1.3, 2.2**

### Property 2: Insufficient energy blocks fission

*For any* actor with energy below `reproduction_cost + offspring_energy`, the Reproduction_System SHALL not produce a spawn buffer entry, and the actor's energy SHALL remain unchanged.

**Validates: Requirements 2.1**

## Error Handling

No changes to error handling. The existing NaN/Inf check on `actor.energy` after deduction remains in place and now validates the corrected arithmetic. Since `reproduction_cost` and `offspring_energy` are both finite positive floats (enforced by config validation), the sum cannot introduce NaN or Inf that wasn't already present.

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate (already available in the Rust ecosystem, zero-cost in release builds).

Each property test generates random `HeritableTraits` values (within config clamp ranges) and random actor energy levels, then runs the reproduction logic and verifies the conservation invariant.

Configuration: minimum 100 iterations per property test (proptest default is 256).

Tag format: `Feature: reproduction-energy-conservation, Property N: <title>`

### Unit Testing

- Specific example: actor with known energy, reproduction_cost, and offspring_energy undergoes fission. Assert exact parent energy after deduction.
- Edge case: actor with energy exactly equal to `reproduction_cost + offspring_energy`. Fission should succeed, parent energy should be exactly 0.
- Edge case: actor with energy one epsilon below the threshold. Fission should be blocked.

### Test Placement

Tests live in the existing `#[cfg(test)] mod tests` block in `src/grid/actor_systems.rs`. Property tests can be added alongside or in a separate `proptest` module within the same file.
