# Requirements Document

## Introduction

This feature introduces a `Brain` component for actors — a cold-data SoA storage layer that holds a fixed-size circular buffer of past physical interaction memories. Memory capacity is a heritable trait subject to evolutionary pressure: larger memory buffers cost more energy per tick via a cognitive metabolic cost. The Brain is stored as a separate parallel `Vec<Brain>` indexed by actor slot, keeping the hot Actor struct unchanged.

This spec covers only the memory infrastructure and cognitive cost. Behavioral use of memories (sensing integration, site fidelity, avoidance biases) is deferred to a follow-up spec.

## Glossary

- **Actor**: The atomic biological agent in the simulation. An ECS entity with physical components occupying exactly one grid cell.
- **Brain**: A plain data component stored separately from Actor in a parallel `Vec`, indexed by actor slot. Contains a circular buffer of memory entries.
- **MemoryEntry**: A fixed-size record (~20 bytes) of a single physical interaction: tick, cell position, genome hash of encountered actor, and outcome type.
- **Memory_Buffer**: A fixed-size inline circular buffer (`[MemoryEntry; MAX_MEMORY_CAPACITY]`) with a head pointer and effective length capped by the heritable `memory_capacity` trait.
- **MAX_MEMORY_CAPACITY**: A compile-time constant (e.g., 16) bounding the maximum possible memory buffer size across all actors.
- **HeritableTraits**: The struct on Actor carrying per-actor evolvable trait values. Extended with `memory_capacity`.
- **ActorConfig**: The TOML-configurable struct controlling actor behavior defaults and heritable trait clamp bounds.
- **Metabolism_System**: The `run_actor_metabolism` function that computes per-tick energy balance.
- **Predation_System**: The `run_contact_predation` function that evaluates and executes contact predation between adjacent actors.
- **Reproduction_System**: The `run_actor_reproduction` and `run_deferred_spawn` functions that handle binary fission and offspring creation.
- **Cognitive_Cost**: The per-tick energy drain proportional to an actor's heritable `memory_capacity`, computed as `cognitive_cost_per_slot * memory_capacity`.
- **Genome_Hash**: A compact hash of an actor's heritable traits, used in memory entries to identify encountered actors without storing full trait vectors.

## Requirements

### Requirement 1: Brain Component Data Structure

**User Story:** As a simulation engine, I want a Brain component stored in a parallel SoA Vec indexed by actor slot, so that cold cognitive data is separated from the hot Actor struct for cache efficiency.

#### Acceptance Criteria

1. THE Brain component SHALL be a plain data struct containing a fixed-size inline circular buffer of MemoryEntry values, a head index, and an effective length field.
2. THE MemoryEntry struct SHALL contain a tick number (u64), a cell index (u32), a genome hash (u32), and an outcome type enum, with a total size not exceeding 24 bytes.
3. THE Memory_Buffer SHALL use a compile-time constant MAX_MEMORY_CAPACITY to bound the inline array size.
4. WHEN an actor is added to the ActorRegistry, THE Brain storage SHALL allocate a corresponding Brain slot at the same index.
5. WHEN an actor is removed from the ActorRegistry, THE Brain storage SHALL clear the corresponding Brain slot.
6. THE Brain storage SHALL be a parallel `Vec<Brain>` managed alongside the ActorRegistry, indexed by the same slot indices.

### Requirement 2: Memory Capacity as a Heritable Trait

**User Story:** As a simulation designer, I want memory capacity to be a heritable, evolvable trait on each actor, so that natural selection determines how much memory a lineage retains.

#### Acceptance Criteria

1. THE HeritableTraits struct SHALL include a `memory_capacity` field of type `u8`.
2. WHEN an offspring is created during reproduction, THE Reproduction_System SHALL inherit the parent's `memory_capacity` with proportional gaussian mutation, clamped to `[trait_memory_capacity_min, trait_memory_capacity_max]`.
3. THE ActorConfig SHALL include `memory_capacity` (seed genome default), `trait_memory_capacity_min` (minimum clamp, >= 0), and `trait_memory_capacity_max` (maximum clamp, <= MAX_MEMORY_CAPACITY) fields.
4. WHEN `memory_capacity` is 0, THE Brain SHALL store zero memory entries and impose zero cognitive cost, making the actor fully memoryless.
5. THE `memory_capacity` mutation SHALL operate in f32 space (convert u8 → f32, apply proportional mutation, round, clamp, cast back to u8), consistent with the existing `max_tumble_steps` mutation pattern.

### Requirement 3: Cognitive Metabolic Cost

**User Story:** As a simulation designer, I want brain upkeep to scale with memory capacity, so that larger brains impose evolutionary pressure through increased energy drain.

#### Acceptance Criteria

1. WHEN the Metabolism_System computes per-tick energy balance for an active actor, THE Metabolism_System SHALL subtract a cognitive cost equal to `cognitive_cost_per_slot * actor.traits.memory_capacity` from the actor's energy.
2. THE ActorConfig SHALL include a `cognitive_cost_per_slot` field (f32, >= 0.0, finite) controlling the energy cost per memory slot per tick.
3. WHEN `cognitive_cost_per_slot` is 0.0, THE Metabolism_System SHALL impose zero cognitive cost regardless of memory capacity.
4. WHEN `memory_capacity` is 0, THE Metabolism_System SHALL impose zero cognitive cost regardless of `cognitive_cost_per_slot`.

### Requirement 4: Memory Entry Recording

**User Story:** As a simulation engine, I want actors to record physical interaction outcomes into their memory buffer, so that past experiences are available for future use.

#### Acceptance Criteria

1. WHEN an active actor consumes chemical during the metabolism phase, THE Metabolism_System SHALL write a memory entry with the current tick, the actor's cell index, a zero genome hash (no other actor involved), and a food-outcome type to the actor's Brain.
2. WHEN a predation event occurs, THE Predation_System SHALL write a memory entry to both the predator's Brain (predation-success outcome) and the prey's Brain (predation-threat outcome), recording the current tick, the cell index, and the genome hash of the other actor.
3. WHEN a memory entry is written to a Brain whose effective length equals the actor's `memory_capacity`, THE Brain SHALL overwrite the oldest entry in circular buffer order.
4. WHEN a memory entry is written to a Brain whose effective length is less than the actor's `memory_capacity`, THE Brain SHALL append the entry and increment the effective length.
5. WHEN an actor's `memory_capacity` is 0, THE Brain SHALL reject all memory writes and remain empty.
6. THE memory write order within each system phase SHALL follow deterministic slot-index order, consistent with existing iteration patterns.

### Requirement 5: Reproduction Integration

**User Story:** As a simulation engine, I want offspring to start with an empty memory buffer and inherit memory capacity, so that cognitive evolution proceeds through trait inheritance rather than memory transfer.

#### Acceptance Criteria

1. WHEN an offspring is created during deferred spawn, THE Reproduction_System SHALL initialize the offspring's Brain with an empty memory buffer (effective length 0, head index 0).
2. WHEN an offspring is created, THE Reproduction_System SHALL inherit and mutate `memory_capacity` from the parent's HeritableTraits, following the existing proportional gaussian mutation pattern.
3. THE genetic distance computation SHALL include `memory_capacity` in the normalized Euclidean distance calculation, using its clamp bounds for normalization.

### Requirement 6: Configuration and Documentation Updates

**User Story:** As a simulation operator, I want all new configuration fields documented and visible in the visualization panel, so that I can tune brain parameters and observe their effects.

#### Acceptance Criteria

1. WHEN new heritable traits are added to HeritableTraits, THE configuration documentation SHALL be updated: `example_config.toml` with commented entries, `format_config_info` in the viz panel, and the config-documentation steering file reference table.
2. WHEN new heritable traits are added, THE trait visualization SHALL be updated: `compute_trait_stats_from_actors` SHALL collect statistics for the new trait, `format_trait_stats` SHALL display the new trait row, and `format_actor_info` SHALL display the new trait value.
3. THE `TraitStats.traits` array size SHALL be increased to accommodate the new heritable trait (memory_capacity).
4. THE ActorConfig SHALL validate all new fields at parse time: clamp bounds must satisfy `min < max`, seed defaults must be within clamp bounds, `cognitive_cost_per_slot` must be >= 0.0 and finite, and `trait_memory_capacity_max` must be <= MAX_MEMORY_CAPACITY.

### Requirement 7: Determinism and Performance

**User Story:** As a simulation engineer, I want all brain operations to be deterministic and allocation-free in hot paths, so that simulation replay and performance constraints are preserved.

#### Acceptance Criteria

1. THE Brain memory buffer SHALL use a fixed-size inline array with no heap allocation for memory entry storage.
2. THE memory write operations SHALL follow deterministic slot-index order within each system phase.
3. THE Brain storage Vec SHALL be pre-allocated at grid construction time using `initial_actor_capacity`, so that no heap allocation occurs during tick execution for the Brain Vec itself.
4. WHEN the ActorRegistry grows its slot Vec, THE Brain storage Vec SHALL grow in lockstep to maintain index correspondence.
