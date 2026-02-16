# Implementation Plan: Reproduction Energy Conservation Fix

## Overview

Single-line bugfix in `run_actor_reproduction` to deduct `reproduction_cost + offspring_energy` from the parent instead of only `reproduction_cost`. Update comments and add tests to verify the conservation invariant.

## Tasks

- [x] 1. Fix energy deduction in `run_actor_reproduction`
  - [x] 1.1 Correct the parent energy deduction line
    - In `src/grid/actor_systems.rs`, `run_actor_reproduction`, change `actor.energy -= actor.traits.reproduction_cost;` to `actor.energy -= actor.traits.reproduction_cost + actor.traits.offspring_energy;`
    - Update the comment above the deduction to explain the full energy accounting: `reproduction_cost` is entropy/overhead, `offspring_energy` is transferred to offspring
    - Add conservation invariant comment: `parent_before = parent_after + reproduction_cost + offspring_energy`
    - _Requirements: 1.1, 1.2, 1.3, 3.1, 3.2_

  - [ ]* 1.2 Write unit test for correct fission energy deduction
    - Add a test in the existing `#[cfg(test)] mod tests` block in `src/grid/actor_systems.rs`
    - Set up a single actor on a 3×3 grid with known energy, reproduction_cost, and offspring_energy
    - Run `run_actor_reproduction` and assert parent energy equals `energy_before - reproduction_cost - offspring_energy`
    - Assert spawn buffer contains one entry with the correct offspring_energy
    - _Requirements: 1.1, 1.2, 4.2_

  - [ ]* 1.3 Write unit test for energy gate blocking insufficient energy
    - Add a test where actor energy is one epsilon below `reproduction_cost + offspring_energy`
    - Run `run_actor_reproduction` and assert spawn buffer is empty and actor energy is unchanged
    - _Requirements: 2.1_

  - [ ]* 1.4 Write unit test for exact-threshold fission
    - Add a test where actor energy equals exactly `reproduction_cost + offspring_energy`
    - Run `run_actor_reproduction` and assert fission succeeds and parent energy is exactly 0.0
    - _Requirements: 2.1, 2.2_

- [x] 2. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 3. Property-based tests for energy conservation
  - [ ]* 3.1 Write property test for fission energy conservation
    - **Property 1: Fission energy conservation**
    - **Validates: Requirements 1.1, 1.2, 1.3, 2.2**
    - Add `proptest` as a dev-dependency in `Cargo.toml` if not already present
    - Generate random `reproduction_cost` and `offspring_energy` within config clamp ranges
    - Generate random actor energy above the threshold
    - Set up a 3×3 grid with the actor at center and at least one empty neighbor
    - Run `run_actor_reproduction` and assert: `parent_energy_after + offspring_energy == parent_energy_before - reproduction_cost` (within f32 epsilon)
    - Tag: `Feature: reproduction-energy-conservation, Property 1: Fission energy conservation`

  - [ ]* 3.2 Write property test for insufficient energy blocking
    - **Property 2: Insufficient energy blocks fission**
    - **Validates: Requirements 2.1**
    - Generate random `reproduction_cost` and `offspring_energy` within config clamp ranges
    - Generate random actor energy strictly below `reproduction_cost + offspring_energy` (but above 0)
    - Run `run_actor_reproduction` and assert spawn buffer is empty and actor energy is unchanged
    - Tag: `Feature: reproduction-energy-conservation, Property 2: Insufficient energy blocks fission`

- [ ] 4. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- The core fix is task 1.1 — a single-line change
- Property tests use `proptest` crate with minimum 100 iterations
- No config changes needed — the gate already checks the correct amount
- No data model changes — `Actor`, `HeritableTraits`, `ActorConfig` are unchanged
- This is a HOT path system — the fix adds one f32 addition, no allocations or dispatch changes
