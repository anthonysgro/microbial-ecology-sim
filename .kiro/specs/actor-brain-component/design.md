# Design Document: Actor Brain Component (Memory Infrastructure)

## Overview

This design introduces a `Brain` component that gives actors a circular buffer of past interaction memories and a heritable `memory_capacity` trait with per-tick cognitive metabolic cost. The Brain is stored as a separate cold-data component in a parallel `Vec<Brain>` indexed by actor slot, keeping the hot `Actor` struct unchanged.

This spec covers only the memory infrastructure and cognitive cost. Behavioral use of memories (sensing integration, site fidelity, avoidance biases) is deferred to a follow-up spec. Actors will record memories but not yet act on them — the evolutionary pressure from cognitive cost still applies, so `memory_capacity` will drift toward zero unless a future spec makes memory useful.

## Architecture

The Brain component follows the existing ECS data-oriented pattern: plain data struct, no methods beyond trivial construction, systems are stateless functions operating on component queries.

### Data Flow Through Tick Phases

```
Phase 1: Sensing       — no Brain interaction (deferred to follow-up spec)
Phase 2: Metabolism    — subtract cognitive cost, write food memory entries
Phase 3: Removal       — clear Brain slot for removed actors
Phase 4: Reproduction  — initialize empty Brain for offspring
Phase 4.75: Predation  — write predation memory entries to both actors
Phase 4.8: Pred Removal — clear Brain slot for removed actors
Phase 5: Movement      — no Brain interaction
```

### Storage Layout

```
Grid struct
├── actors: ActorRegistry          (hot: position, energy, tumble state)
├── brains: Vec<Brain>             (cold: memory buffers, indexed by slot)
├── occupancy: Vec<Option<usize>>
├── removal_buffer: Vec<ActorId>
├── spawn_buffer: Vec<(usize, f32, HeritableTraits)>
└── movement_targets: Vec<Option<usize>>
```

The `brains` Vec is managed in lockstep with the `ActorRegistry` slots Vec. When `take_actors` extracts actor data for the tick phases, `brains` is extracted alongside. When `put_actors` returns data, `brains` is returned too.

## Components and Interfaces

### MemoryEntry (Plain Data Struct, ~20 bytes)

```rust
/// Outcome type for a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemoryOutcome {
    /// Food gained during metabolism (positive outcome).
    Food = 0,
    /// Successful predation on another actor (positive outcome).
    PredationSuccess = 1,
    /// Survived a predation attempt as prey (negative outcome).
    PredationThreat = 2,
}

/// A single memory record of a physical interaction.
///
/// Fixed-size, Copy, no heap allocation. Stored inline in Brain's
/// circular buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryEntry {
    /// Simulation tick when the interaction occurred.
    pub tick: u64,
    /// Grid cell index where the interaction occurred.
    pub cell_index: u32,
    /// Hash of the other actor's genome (0 for food/no-actor events).
    pub genome_hash: u32,
    /// Type of interaction outcome.
    pub outcome: MemoryOutcome,
}
// Layout: 8 (tick) + 4 (cell_index) + 4 (genome_hash) + 1 (outcome) + 3 (padding) = 20 bytes
```

Design decision: `cell_index` is stored as `u32` instead of `usize` to keep the struct compact. A 400×400 grid has 160,000 cells, well within u32 range. `genome_hash` is `u32` — a truncated hash of the trait vector, sufficient for memory-level identity (not cryptographic, intentionally lossy — collisions are biologically plausible as "misremembering"). This keeps MemoryEntry at 20 bytes with padding.

### Brain (Plain Data Struct)

```rust
/// Compile-time maximum memory capacity across all actors.
pub const MAX_MEMORY_CAPACITY: usize = 16;

/// Cognitive component for an actor. Stored in a parallel Vec<Brain>
/// indexed by actor slot. Contains a fixed-size circular buffer of
/// memory entries.
///
/// Plain data struct — no methods beyond trivial construction.
#[derive(Debug, Clone, PartialEq)]
pub struct Brain {
    /// Circular buffer of memory entries. Only entries [0..len) are valid
    /// when len < capacity; when len == capacity, head points to the oldest
    /// entry (next to be overwritten).
    pub entries: [MemoryEntry; MAX_MEMORY_CAPACITY],
    /// Index of the next write position (wraps around).
    pub head: u8,
    /// Number of valid entries. Capped at the actor's heritable memory_capacity.
    pub len: u8,
}
// Size: 20 * 16 + 1 + 1 = 322 bytes per Brain
```

Design decision: The buffer is always `MAX_MEMORY_CAPACITY` in size at the storage level, but the effective capacity is governed by the actor's heritable `memory_capacity` trait. This avoids variable-length storage while allowing evolution to tune capacity. Actors with `memory_capacity = 0` never write entries and the Brain is effectively dead weight (but the fixed-size allocation simplifies slot management).

### Brain Helper Functions (Free Functions)

```rust
/// Create an empty Brain with zeroed entries.
pub fn brain_empty() -> Brain { ... }

/// Write a memory entry to a Brain, respecting the actor's memory_capacity.
/// No-op when capacity == 0.
pub fn brain_write(brain: &mut Brain, entry: MemoryEntry, capacity: u8) { ... }

/// Compute a genome hash from heritable traits.
/// Deterministic: identical traits produce identical hashes.
/// Uses wrapping arithmetic — intentionally lossy (u32 collisions are
/// biologically plausible as "misremembering" identity).
pub fn genome_hash(traits: &HeritableTraits) -> u32 { ... }
```

### HeritableTraits Extension

One new field added to `HeritableTraits`:

```rust
pub struct HeritableTraits {
    // ... existing 12 fields ...
    /// Maximum number of memory entries this actor can retain. 0 = memoryless.
    pub memory_capacity: u8,
}
```

The `mutate` function is extended to mutate `memory_capacity` using the same proportional gaussian pattern as `max_tumble_steps`: mutate in f32 space, round, clamp, cast to u8.

The size assertion updates from 44 to the new size (44 + 1 + padding).

### ActorConfig Extensions

New fields in `ActorConfig`:

| Field | Type | Default | Description |
|---|---|---|---|
| `memory_capacity` | `u8` | `4` | Seed genome default for heritable memory_capacity. |
| `trait_memory_capacity_min` | `u8` | `0` | Minimum clamp bound. |
| `trait_memory_capacity_max` | `u8` | `16` | Maximum clamp bound. Must be <= MAX_MEMORY_CAPACITY. |
| `cognitive_cost_per_slot` | `f32` | `0.005` | Energy cost per memory slot per tick. |

### Genetic Distance Extension

The `genetic_distance` function's `TRAIT_COUNT` constant increases from 12 to 13. One new entry is appended to the traits array:

```rust
(a.memory_capacity as f32, b.memory_capacity as f32,
 config.trait_memory_capacity_min as f32, config.trait_memory_capacity_max as f32),
```

### Grid Integration

The `Grid` struct gains a `brains: Vec<Brain>` field. The `take_actors` / `put_actors` pattern is extended to include `brains`:

```rust
pub(crate) fn take_actors(&mut self) -> (
    ActorRegistry, Vec<Brain>, Vec<Option<usize>>,
    Vec<ActorId>, Vec<(usize, f32, HeritableTraits)>, Vec<Option<usize>>
) { ... }

pub(crate) fn put_actors(&mut self,
    actors: ActorRegistry, brains: Vec<Brain>, occupancy: Vec<Option<usize>>,
    removal_buffer: Vec<ActorId>, spawn_buffer: Vec<(usize, f32, HeritableTraits)>,
    movement_targets: Vec<Option<usize>>
) { ... }
```

When `add_actor` is called, a corresponding `brain_empty()` is pushed to `brains` (or the freed slot is reset). When `remove_actor` is called, the corresponding brain slot is zeroed. The `brains` Vec is pre-allocated with `initial_actor_capacity`.

### Metabolism System Changes

After computing the existing energy balance (consumption, decay, thermal cost, readiness cost), the metabolism system adds cognitive cost:

```rust
let cognitive_cost = config.cognitive_cost_per_slot * actor.traits.memory_capacity as f32;
actor.energy += consumed * effective_conversion * fitness
    - actor.traits.base_energy_decay
    - thermal_cost
    - readiness_cost
    - cognitive_cost;
```

After the energy update, if the actor consumed food (consumed > 0.0) and is not inert, write a food memory entry:

```rust
if consumed > 0.0 && !actor.inert {
    let entry = MemoryEntry {
        tick: current_tick,
        cell_index: ci as u32,
        genome_hash: 0,
        outcome: MemoryOutcome::Food,
    };
    brain_write(&mut brains[slot_index], entry, actor.traits.memory_capacity);
}
```

The `run_actor_metabolism` signature gains `brains: &mut [Brain]` and `tick: u64` parameters.

### Predation System Changes

After a successful predation event in pass 2, write memory entries for both predator and prey:

```rust
// Predator remembers successful hunt
let entry = MemoryEntry {
    tick: current_tick,
    cell_index: predator.cell_index as u32,
    genome_hash: genome_hash(&prey_traits),
    outcome: MemoryOutcome::PredationSuccess,
};
brain_write(&mut brains[predator_slot], entry, predator.traits.memory_capacity);

// Prey remembers threat (if still alive / inert)
let entry = MemoryEntry {
    tick: current_tick,
    cell_index: prey.cell_index as u32,
    genome_hash: genome_hash(&predator_traits),
    outcome: MemoryOutcome::PredationThreat,
};
brain_write(&mut brains[prey_slot], entry, prey.traits.memory_capacity);
```

The `run_contact_predation` signature gains `brains: &mut [Brain]` and `tick: u64` parameters.

### Reproduction / Spawn Changes

The `run_deferred_spawn` function initializes a `brain_empty()` for each offspring. When a new slot is allocated (either from the free list or by growing the Vec), the corresponding brain slot is set to `brain_empty()`. No parent memory is transferred — only the heritable `memory_capacity` trait is inherited and mutated.

### Visualization Changes

**TraitStats**: Array size increases from `[SingleTraitStats; 12]` to `[SingleTraitStats; 13]`.

**compute_trait_stats_from_actors**: One new `Vec<f32>` collector for `memory_capacity`, pushed in the single-pass loop, computed via `compute_single_stats`.

**format_trait_stats**: One new row displaying `memory_capacity`.

**format_actor_info**: One new line displaying the brain trait value.

**format_config_info**: New entries displaying brain-related config fields.

## Data Models

### MemoryEntry Layout

| Field | Type | Offset | Size |
|---|---|---|---|
| tick | u64 | 0 | 8 |
| cell_index | u32 | 8 | 4 |
| genome_hash | u32 | 12 | 4 |
| outcome | MemoryOutcome (u8) | 16 | 1 |
| _padding | - | 17 | 3 |
| **Total** | | | **20** |

### Brain Layout

| Field | Type | Size |
|---|---|---|
| entries | [MemoryEntry; 16] | 320 |
| head | u8 | 1 |
| len | u8 | 1 |
| **Total** | | **322** |

### HeritableTraits Layout (Updated)

Previous size: 44 bytes (12 fields).
New field: `memory_capacity` (u8, 1 byte).
New size: 48 bytes (with alignment padding). The exact size depends on field ordering — the size assertion in code will enforce the actual value.

## Correctness Properties

### Property 1: Brain-ActorRegistry Parallel Invariant

*For any* sequence of actor add and remove operations on an ActorRegistry with associated Brain storage, the Brain Vec length SHALL always be >= the ActorRegistry slot count, and every active (non-removed) actor slot SHALL have a corresponding Brain entry at the same index.

**Validates: Requirements 1.4, 1.5, 1.6, 7.4**

### Property 2: Mutation Clamp Bounds for memory_capacity

*For any* parent `HeritableTraits` (with values within clamp bounds) and any deterministic RNG seed, applying `mutate()` SHALL produce offspring traits where `memory_capacity` is within `[trait_memory_capacity_min, trait_memory_capacity_max]`.

**Validates: Requirements 2.2, 2.5, 5.2**

### Property 3: Zero-Capacity Brain Remains Empty

*For any* sequence of `brain_write` calls on a Brain with `capacity = 0`, the Brain's `len` field SHALL remain 0 and no entries SHALL be modified.

**Validates: Requirements 2.4, 4.5**

### Property 4: Cognitive Cost Correctness

*For any* active actor with `memory_capacity = M` and config `cognitive_cost_per_slot = C`, the per-tick energy deduction from cognitive cost SHALL equal `M * C`. When either `M = 0` or `C = 0.0`, the cognitive cost SHALL be exactly `0.0`.

**Validates: Requirements 3.1, 3.3, 3.4**

### Property 5: Circular Buffer Write Semantics

*For any* Brain with capacity `C > 0` and any sequence of `N` write operations, the Brain SHALL contain exactly `min(N, C)` valid entries, and the `min(N, C)` most recently written entries SHALL be present in the buffer. When `N > C`, the oldest `N - C` entries SHALL have been overwritten.

**Validates: Requirements 4.3, 4.4**

### Property 6: Food Memory Write on Consumption

*For any* active actor that consumes a positive amount of chemical during metabolism (consumed > 0.0), the Metabolism_System SHALL write a `MemoryEntry` with outcome `Food`, the current tick, and the actor's cell index to the actor's Brain.

**Validates: Requirements 4.1**

### Property 7: Predation Memory Write for Both Actors

*For any* successful predation event between a predator and prey, the Predation_System SHALL write a `PredationSuccess` entry to the predator's Brain and a `PredationThreat` entry to the prey's Brain, both recording the current tick, the respective cell indices, and the genome hash of the other actor.

**Validates: Requirements 4.2**

### Property 8: Offspring Brain Is Empty

*For any* offspring created during deferred spawn, the offspring's Brain SHALL have `len = 0` and `head = 0`, regardless of the parent's Brain contents.

**Validates: Requirements 5.1**

### Property 9: Genetic Distance Includes memory_capacity

*For any* two actors whose HeritableTraits differ only in `memory_capacity`, the `genetic_distance` function SHALL return a value strictly greater than 0.0. The distance SHALL remain in the range [0.0, 1.0].

**Validates: Requirements 5.3**

## Error Handling

All error handling follows the existing project patterns:

- **NaN/Inf checks**: After computing cognitive cost and updating energy in metabolism, the existing NaN/Inf check catches any numerical instability introduced by the cognitive cost term. No new error paths needed.
- **Brain slot out-of-bounds**: The parallel Vec invariant (Property 1) ensures brain slots are always valid for active actors. Debug assertions (`debug_assert!(slot_index < brains.len())`) guard against logic bugs in development builds.
- **Overflow in genome_hash**: The hash computation uses wrapping arithmetic — no overflow panic possible.
- **Memory write to capacity-0 brain**: `brain_write` is a no-op when capacity is 0. No error returned.
- **Config validation**: New fields are validated at parse time using the existing `deny_unknown_fields` + validation function pattern. Invalid clamp bounds (min >= max for non-zero ranges), out-of-range defaults, or `cognitive_cost_per_slot < 0.0` produce descriptive error messages at startup.

No new error enum variants are needed. The existing `TickError::NumericalError` and `ActorError` variants cover all failure modes.

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate for property-based testing. Each property test runs a minimum of 100 iterations with generated inputs.

| Property | Test Focus | Generator Strategy |
|---|---|---|
| P1 | Brain-Registry parallel invariant | Random sequences of add/remove operations |
| P2 | Mutation clamp bounds for memory_capacity | Random HeritableTraits within bounds + random RNG seeds |
| P3 | Zero-capacity brain stays empty | Random MemoryEntry sequences with capacity=0 |
| P4 | Cognitive cost correctness | Random (memory_capacity, cognitive_cost_per_slot) pairs |
| P5 | Circular buffer semantics | Random entry sequences of varying length vs capacity |
| P8 | Offspring brain empty | Random parent states |
| P9 | Genetic distance with memory_capacity | Random trait pairs differing only in memory_capacity |

### Unit Testing

- **Circular buffer edge cases**: Write exactly `capacity` entries, then one more. Verify head wraps correctly.
- **Metabolism integration**: Single actor with known chemical level, verify cognitive cost is subtracted.
- **Predation integration**: Two adjacent actors, verify both brains receive entries after predation.
- **Genome hash determinism**: Same traits produce same hash. Different traits produce different hash (with high probability).
- **Config validation**: Invalid clamp bounds rejected at parse time.

### Test Tagging Convention

```rust
// Feature: actor-brain-component, Property 5: Circular buffer write semantics
// Validates: Requirements 4.3, 4.4
```
