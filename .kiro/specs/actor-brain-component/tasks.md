# Implementation Plan: Actor Brain Component (Memory Infrastructure)

## Overview

Incremental implementation of the Brain memory component: data types first, then heritable trait extension, then Grid integration, then system-by-system integration (metabolism, predation, reproduction), then visualization. Each task builds on the previous, with property tests validating correctness at each stage.

## Tasks

- [x] 1. Define Brain data types and helper functions
  - [x] 1.1 Create `src/grid/brain.rs` with `MemoryOutcome` enum, `MemoryEntry` struct, `MAX_MEMORY_CAPACITY` constant, `Brain` struct, and static size assertions
    - `MemoryEntry`: tick (u64), cell_index (u32), genome_hash (u32), outcome (MemoryOutcome), ~20 bytes
    - `Brain`: entries ([MemoryEntry; MAX_MEMORY_CAPACITY]), head (u8), len (u8)
    - Add `pub mod brain;` to `src/grid/mod.rs`
    - _Requirements: 1.1, 1.2, 1.3_

  - [x] 1.2 Implement `brain_empty()`, `brain_write()`, and `genome_hash()` free functions in `src/grid/brain.rs`
    - `brain_empty()`: returns Brain with zeroed entries, head=0, len=0
    - `brain_write(brain, entry, capacity)`: no-op when capacity=0, circular buffer insert otherwise
    - `genome_hash(traits)`: deterministic u32 hash of HeritableTraits using wrapping arithmetic
    - _Requirements: 1.1, 4.3, 4.4, 4.5_

  - [ ]* 1.3 Write property tests for circular buffer semantics
    - **Property 5: Circular buffer write semantics**
    - **Validates: Requirements 4.3, 4.4**
    - Use `proptest` to generate random entry sequences and verify min(N, C) entries present

  - [ ]* 1.4 Write property test for zero-capacity brain
    - **Property 3: Zero-capacity brain remains empty**
    - **Validates: Requirements 2.4, 4.5**

- [x] 2. Extend HeritableTraits and ActorConfig with memory_capacity
  - [x] 2.1 Add `memory_capacity` (u8) to `HeritableTraits` in `src/grid/actor.rs`
    - Update the size assertion
    - Update `from_config()` to read new field from ActorConfig
    - Update `mutate()` to mutate memory_capacity (in f32 space like max_tumble_steps)
    - _Requirements: 2.1, 2.5_

  - [x] 2.2 Add new config fields to `ActorConfig` in `src/grid/actor_config.rs`
    - `memory_capacity`, `trait_memory_capacity_min`, `trait_memory_capacity_max`
    - `cognitive_cost_per_slot`
    - Add default functions, serde attributes, and validation logic
    - _Requirements: 2.3, 3.2, 6.4_

  - [x] 2.3 Update `genetic_distance()` in `src/grid/actor_systems.rs`
    - Increase `TRAIT_COUNT` from 12 to 13
    - Append memory_capacity entry to the traits array
    - _Requirements: 5.3_

  - [ ]* 2.4 Write property test for mutation clamp bounds
    - **Property 2: Mutation clamp bounds for memory_capacity**
    - **Validates: Requirements 2.2, 2.5, 5.2**

  - [ ]* 2.5 Write property test for genetic distance with memory_capacity
    - **Property 9: Genetic distance includes memory_capacity**
    - **Validates: Requirements 5.3**

- [x] 3. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Integrate Brain storage into Grid and ActorRegistry lifecycle
  - [x] 4.1 Add `brains: Vec<Brain>` field to `Grid` struct in `src/grid/mod.rs`
    - Pre-allocate with `initial_actor_capacity` in `Grid::new()`
    - Update `take_actors()` to include `brains` (mem::take)
    - Update `put_actors()` to accept and restore `brains`
    - Update `add_actor()` to push/reset a `brain_empty()` at the new slot
    - Update `remove_actor()` to clear the brain slot
    - Add `pub fn brains(&self) -> &[Brain]` accessor
    - _Requirements: 1.4, 1.5, 1.6, 7.3_

  - [x] 4.2 Update `run_actor_phases()` in `src/grid/tick.rs` to extract and pass `brains` through all actor phase calls
    - Destructure brains from `take_actors()`, pass to metabolism, predation, spawn
    - Return brains via `put_actors()`
    - _Requirements: 1.6, 7.4_

  - [ ]* 4.3 Write property test for Brain-ActorRegistry parallel invariant
    - **Property 1: Brain-ActorRegistry parallel invariant**
    - **Validates: Requirements 1.4, 1.5, 1.6, 7.4**

- [x] 5. Integrate Brain into Metabolism system
  - [x] 5.1 Update `run_actor_metabolism()` signature to accept `brains: &mut [Brain]` and `tick: u64`
    - Add cognitive cost: `cognitive_cost_per_slot * memory_capacity as f32` to energy deduction
    - After consumption, write food memory entry when consumed > 0.0 and actor is not inert
    - Update call site in `run_actor_phases()`
    - _Requirements: 3.1, 4.1_

  - [ ]* 5.2 Write property test for cognitive cost correctness
    - **Property 4: Cognitive cost correctness**
    - **Validates: Requirements 3.1, 3.3, 3.4**

  - [ ]* 5.3 Write unit tests for metabolism integration
    - Test: actor with memory_capacity=4 and cognitive_cost_per_slot=0.01 loses 0.04 extra energy
    - Test: actor with memory_capacity=0 has identical energy to pre-Brain behavior
    - Test: food memory entry written after consumption
    - _Requirements: 3.1, 3.3, 3.4, 4.1_

- [x] 6. Integrate Brain into Predation system
  - [x] 6.1 Update `run_contact_predation()` signature to accept `brains: &mut [Brain]` and `tick: u64`
    - After successful predation in pass 2, write PredationSuccess to predator brain and PredationThreat to prey brain
    - Compute genome_hash of the other actor's traits for the memory entry
    - Update call site in `run_actor_phases()`
    - _Requirements: 4.2_

  - [ ]* 6.2 Write unit tests for predation memory writes
    - Test: two adjacent actors, predation succeeds, both brains have entries
    - Test: predator brain has PredationSuccess, prey brain has PredationThreat
    - _Requirements: 4.2_

- [x] 7. Integrate Brain into Reproduction / Spawn system
  - [x] 7.1 Update `run_deferred_spawn()` to initialize offspring Brain
    - When a new actor slot is created or reused, set `brains[slot] = brain_empty()`
    - The memory_capacity trait is already mutated by the updated `mutate()` from task 2.1
    - Update call site in `run_actor_phases()` to pass `brains`
    - _Requirements: 5.1, 5.2_

  - [ ]* 7.2 Write property test for offspring brain empty
    - **Property 8: Offspring brain is empty**
    - **Validates: Requirements 5.1**

- [x] 8. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Update visualization and configuration documentation
  - [x] 9.1 Update `TraitStats` in `src/viz_bevy/resources.rs`
    - Change `[SingleTraitStats; 12]` to `[SingleTraitStats; 13]`
    - Update array order comment to include memory_capacity
    - _Requirements: 6.3_

  - [x] 9.2 Update `compute_trait_stats_from_actors` in `src/viz_bevy/systems.rs`
    - Add one new `Vec<f32>` collector for memory_capacity
    - Push value in the single-pass loop, compute stats via `compute_single_stats`
    - _Requirements: 6.2_

  - [x] 9.3 Update `TRAIT_NAMES`, `format_trait_stats`, and `format_actor_info` in `src/viz_bevy/setup.rs`
    - Extend `TRAIT_NAMES` array to 13 entries
    - Add one new line to `format_actor_info` for memory_capacity
    - _Requirements: 6.2_

  - [x] 9.4 Update `format_config_info` in `src/viz_bevy/setup.rs`
    - Add brain-related config fields to the info panel display
    - _Requirements: 6.1_

  - [x] 9.5 Update `example_config.toml` with brain configuration fields
    - Add commented entries for all new ActorConfig fields under the `[actor]` section
    - _Requirements: 6.1_

  - [x] 9.6 Update existing tests that construct `HeritableTraits` or `ActorConfig` directly
    - Add new fields to all test helper functions (e.g., `default_config()` in actor_systems tests)
    - Update any `Actor` construction in tests to include new trait field
    - _Requirements: 6.4_

- [ ] 10. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- This spec covers only memory infrastructure and cognitive cost — no sensing integration
- Behavioral use of memories (site fidelity, avoidance) is deferred to a follow-up spec
- Without sensing integration, memory_capacity will likely evolve toward 0 (cost with no benefit) — this validates the cognitive cost mechanic
- The `config-documentation.md` steering rule requires updating example_config.toml, viz panel, and steering file for any new config fields — task 9 covers all three
- The `Heritable Trait Update Rule` requires updating HeritableTraits, trait viz stats, stats panel, actor inspector, and trait clamp config — covered across tasks 2 and 9
