# Implementation Plan: Actor Energy Cap

## Overview

Add a `max_energy` field to `ActorConfig`, convert metabolism to demand-driven consumption, update config validation, and update documentation. All changes are localized to `actor_config.rs`, `actor_systems.rs`, `config_file.rs`, `example_config.toml`, and `README.md`.

## Tasks

- [-] 1. Add `max_energy` field to `ActorConfig`
  - [x] 1.1 Add `max_energy: f32` field to `ActorConfig` struct in `src/grid/actor_config.rs`
    - Add the field with doc comment: maximum energy an Actor can hold, clamped after each metabolic tick
    - Set default to `50.0` in the `Default` impl
    - _Requirements: 1.1, 1.4_

  - [ ]* 1.2 Write property test: invalid max_energy rejected
    - **Property 1: Invalid max_energy rejected**
    - Generate `f32` values in `(-Inf, 0.0]` ∪ `{NaN, Inf}`, construct `ActorConfig`, run `validate_world_config`, assert error
    - **Validates: Requirements 1.2, 5.3**

  - [ ]* 1.3 Write property test: initial_energy exceeds max_energy rejected
    - **Property 2: Initial energy within cap**
    - Generate valid `max_energy > 0.0`, then `initial_energy` in `(max_energy, max_energy * 10.0]`, run validation, assert error
    - **Validates: Requirements 1.3**

- [x] 2. Add config validation for `max_energy`
  - [x] 2.1 Add validation checks in `validate_world_config` in `src/io/config_file.rs`
    - Reject `max_energy <= 0.0`, NaN, or infinite
    - Reject `initial_energy > max_energy`
    - _Requirements: 1.2, 1.3, 5.3_

- [-] 3. Implement demand-driven consumption and energy clamping in metabolism
  - [-] 3.1 Modify the active-actor branch in `run_actor_metabolism` in `src/grid/actor_systems.rs`
    - Compute `headroom = (config.max_energy - actor.energy).max(0.0)`
    - Compute `max_useful = headroom / config.energy_conversion_factor`
    - Change consumed from `config.consumption_rate.min(available)` to `config.consumption_rate.min(available).min(max_useful)`
    - After energy update, add `actor.energy = actor.energy.min(config.max_energy)` as safety clamp
    - Inert actor branch remains unchanged (no cap, no consumption, only basal decay)
    - _Requirements: 2.1, 2.3, 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2_

  - [ ]* 3.2 Write property test: post-metabolism energy invariant
    - **Property 3: Post-metabolism energy invariant**
    - Generate random `(energy, max_energy, cell_chemical, consumption_rate, conversion_factor, decay)` tuples
    - Create single-actor registry, run `run_actor_metabolism`, assert `actor.energy <= max_energy`
    - **Validates: Requirements 2.1, 2.2**

  - [ ]* 3.3 Write property test: demand-driven consumption and environmental conservation
    - **Property 4: Demand-driven consumption and environmental conservation**
    - Same generator as Property 3, additionally verify:
      - `consumed == min(rate, available, max(0, (max_energy - energy) / factor))`
      - `chemical_write[ci] == chemical_read[ci] - consumed`
      - When `energy >= max_energy`, consumed == 0 and chemical unchanged
    - **Validates: Requirements 3.3, 3.4, 3.5**

  - [ ]* 3.4 Write property test: inert actors unaffected by energy cap
    - **Property 5: Inert actors unaffected by energy cap**
    - Generate random inert actors with arbitrary energy and cell chemical
    - Run metabolism, verify `energy == e - base_energy_decay` and chemical unchanged
    - **Validates: Requirements 4.1, 4.2**

- [ ] 4. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Update existing tests for `max_energy`
  - [ ] 5.1 Update existing metabolism tests in `src/grid/actor_systems.rs`
    - Add `max_energy: 50.0` (or sufficiently high value) to all `default_config()` and inline `ActorConfig` constructors in existing tests so they continue to pass without behavior change
    - _Requirements: 2.1_

- [ ] 6. Update documentation and example config
  - [ ] 6.1 Add `max_energy` to `example_config.toml`
    - Add `max_energy = 50.0` to the `[actor]` section with a comment explaining purpose and constraints
    - _Requirements: 6.1_

  - [ ] 6.2 Update `README.md` if actor config is documented
    - Add `max_energy` parameter description to any actor configuration documentation section
    - _Requirements: 6.2_

- [ ] 7. Write TOML round-trip property test
  - [ ]* 7.1 Write property test: ActorConfig TOML round-trip
    - **Property 6: ActorConfig TOML round-trip**
    - Generate random valid `ActorConfig` structs (all fields in valid ranges, `max_energy > 0`, `initial_energy <= max_energy`)
    - Serialize to TOML, deserialize back, assert equality
    - **Validates: Requirements 5.1**

- [~] 8. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests use `proptest` crate with minimum 100 iterations
- Existing tests in `actor_systems.rs` need `max_energy` added to their configs to avoid compilation errors — task 5.1 handles this
