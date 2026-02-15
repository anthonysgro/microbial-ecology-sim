# Design Document: Grid Energy Sources

## Overview

This spec has two phases:

**Phase 1 — Moisture Removal**: Strip moisture as a fundamental grid field. Moisture is a second-level emergent property that does not belong in the grid substrate. This removes the `moisture` FieldBuffer from Grid, the entire evaporation system, all moisture accessors, the `heat_read_moisture_rw` borrow-checker workaround, moisture config fields, and all moisture references in the viz layer.

**Phase 2 — Energy Sources**: Introduce a `SourceRegistry` that stores persistent emitters (heat, chemical) and an emission phase that injects values into field write buffers each tick. The emission phase is a WARM precomputation step that runs before the existing HOT parallel systems (diffusion, heat radiation), creating sustained gradients that drive actor behavior.

## Architecture

### Phase 1: Moisture Removal

Moisture touches six areas. Each is a surgical deletion:

| Area | File(s) | Change |
|---|---|---|
| Grid struct | `src/grid/mod.rs` | Remove `moisture: FieldBuffer<f32>` field, all moisture accessors (`read_moisture`, `write_moisture`, `swap_moisture`, `read_write_moisture`), and `heat_read_moisture_rw` |
| Config | `src/grid/config.rs` | Remove `evaporation_coefficient` from `GridConfig`, remove `moisture` from `CellDefaults` |
| Evaporation system | `src/grid/evaporation.rs` | Delete entire file |
| Module declaration | `src/grid/mod.rs` | Remove `pub mod evaporation;` |
| Tick orchestrator | `src/grid/tick.rs` | Remove `use evaporation::run_evaporation`, remove Phase 3 (evaporation call, moisture validation, moisture swap) |
| Viz layer | `src/viz/` | Remove `OverlayMode::Moisture` variant, `moisture_bg_color` function, moisture rendering branch, `'m'` keybinding, moisture test references |
| Entry point | `src/main.rs` | Remove `moisture: 1.0` from CellDefaults, remove `evaporation_coefficient: 0.01` from GridConfig |

After removal, the tick sequence simplifies to:

```
Phase 1: Diffusion  → validate chemical write buffers → swap chemicals
Phase 2: Heat       → validate heat write buffer      → swap heat
```

### Phase 2: Tick Phasing (Updated with Emission)

The emission phase inserts before the existing system sequence. For each field type that has active sources, the orchestrator:
1. Copies the read buffer into the write buffer (so emission adds to current state)
2. Applies emissions to the write buffer
3. Clamps chemical values to ≥ 0.0
4. Validates the write buffer (NaN/infinity check)
5. Swaps buffers

Then the existing HOT systems run against the post-emission state.

```
┌─────────────────────────────────────────────────────────┐
│                    Tick N                                │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │ WARM: Emission Phase (new)                      │    │
│  │  1. Copy read → write for each affected field   │    │
│  │  2. Iterate source list, inject into write bufs │    │
│  │  3. Clamp chemicals to ≥ 0                      │    │
│  │  4. Validate write buffers                      │    │
│  │  5. Swap affected field buffers                 │    │
│  └─────────────────────────────────────────────────┘    │
│                         │                               │
│                         ▼                               │
│  ┌─────────────────────────────────────────────────┐    │
│  │ HOT: Diffusion → validate → swap chemicals      │    │
│  └─────────────────────────────────────────────────┘    │
│                         │                               │
│                         ▼                               │
│  ┌─────────────────────────────────────────────────┐    │
│  │ HOT: Heat radiation → validate → swap heat       │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Module Structure

New file: `src/grid/source.rs`

This module contains the `Source`, `SourceField`, `SourceId`, and `SourceRegistry` types, plus the `run_emission` function. It is a peer to `diffusion.rs` and `heat.rs` within `src/grid/`.

No other modules are added. `tick.rs` gains a call to `run_emission` before the existing system calls.

## Components and Interfaces

### `SourceField` — Target field discriminant

```rust
/// Identifies which grid field a source emits into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceField {
    Heat,
    Chemical(usize), // species index
}
```

### `SourceId` — Stable handle for removal

```rust
/// Opaque identifier for a registered source.
/// Internally a generational index to detect stale removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId {
    index: usize,
    generation: u64,
}
```

A generational index avoids the ABA problem: if a source is removed and a new one reuses the slot, the old `SourceId` will have a stale generation and be rejected on removal.

### `Source` — Single emitter record

```rust
/// A persistent emitter that injects a value into a grid field each tick.
/// Plain data struct — no methods beyond construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub cell_index: usize,
    pub field: SourceField,
    pub emission_rate: f32,
}
```

### `SourceRegistry` — Collection with generational slot management

```rust
/// Stores all active sources in a contiguous Vec with generational slots.
///
/// Slot-based storage: each slot holds an `Option<Source>` and a generation
/// counter. Removed slots become `None` and are reused on the next insert.
/// A free list tracks available slots to avoid linear scans on insertion.
pub struct SourceRegistry {
    slots: Vec<SourceSlot>,
    free_list: Vec<usize>,
    active_count: usize,
}

struct SourceSlot {
    source: Option<Source>,
    generation: u64,
}
```


### `SourceRegistry` API

```rust
impl SourceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self;

    /// Add a source. Returns a SourceId for later removal.
    /// Validates cell_index against cell_count and chemical species
    /// against num_chemicals.
    pub fn add(
        &mut self,
        source: Source,
        cell_count: usize,
        num_chemicals: usize,
    ) -> Result<SourceId, SourceError>;

    /// Remove a source by its identifier.
    /// Returns SourceError::InvalidSourceId if the id is stale or out of range.
    pub fn remove(&mut self, id: SourceId) -> Result<(), SourceError>;

    /// Number of active (non-removed) sources.
    pub fn len(&self) -> usize;

    /// Whether the registry has no active sources.
    pub fn is_empty(&self) -> bool;

    /// Iterate active sources in deterministic slot order.
    pub fn iter(&self) -> impl Iterator<Item = &Source>;
}
```

### `SourceError` — Error type

```rust
/// Errors from source registration and management.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SourceError {
    #[error("cell index {cell_index} out of bounds (grid has {cell_count} cells)")]
    CellOutOfBounds {
        cell_index: usize,
        cell_count: usize,
    },

    #[error("chemical species {species} out of range (grid has {num_chemicals} species)")]
    InvalidChemicalSpecies {
        species: usize,
        num_chemicals: usize,
    },

    #[error("invalid source id (index={index}, generation={generation})")]
    InvalidSourceId {
        index: usize,
        generation: u64,
    },
}
```

### `run_emission` — Emission phase system function

```rust
/// WARM PATH: Executes once per tick over the source list.
/// Iterates all active sources, injecting emission_rate into the
/// appropriate field's write buffer. Caller is responsible for
/// copy-read-to-write, validation, and swap.
///
/// This function only performs the additive injection step.
pub fn run_emission(grid: &mut Grid, registry: &SourceRegistry);
```

The orchestrator handles the copy, validate, and swap steps around `run_emission` to keep the emission function focused and testable.

### `Grid` Integration

The `SourceRegistry` is stored as a field on `Grid`:

```rust
pub struct Grid {
    config: GridConfig,
    chemicals: Vec<FieldBuffer<f32>>,
    heat: FieldBuffer<f32>,
    partitions: Vec<Partition>,
    sources: SourceRegistry,  // NEW
}
```

`Grid` exposes:
- `pub fn sources(&self) -> &SourceRegistry`
- `pub fn sources_mut(&mut self) -> &mut SourceRegistry`
- `pub fn add_source(&mut self, source: Source) -> Result<SourceId, SourceError>` — convenience that delegates to `sources.add()` with the grid's cell_count and num_chemicals
- `pub fn remove_source(&mut self, id: SourceId) -> Result<(), SourceError>` — convenience delegating to `sources.remove()`

A new method on `FieldBuffer<T: Copy>`:
- `pub fn copy_read_to_write(&mut self)` — copies the read buffer contents into the write buffer. Used by the emission phase to ensure emission adds to current state.

### `TickOrchestrator` Changes

After moisture removal, `TickOrchestrator::step` has two phases (diffusion, heat). The emission phase inserts before them:

```rust
pub fn step(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError> {
    // Phase 0: Emission (WARM)
    run_emission_phase(grid, config)?;

    // Phase 1: Chemical diffusion (existing, unchanged)
    // Phase 2: Heat radiation (existing, unchanged)
    ...
}
```

The `run_emission_phase` helper:
1. Determines which field types have active sources (by scanning the source list once)
2. For each affected field: copy read→write, then `run_emission` applies the injections
3. Clamps chemical write buffer values to ≥ 0.0
4. Validates all affected write buffers (NaN/infinity check)
5. Swaps affected field buffers

If no sources are active, the emission phase is a no-op (zero cost).

## Data Models

### Source Storage Layout

Sources are stored in a flat `Vec<SourceSlot>` with generational indices. This is effectively a manual slot map without pulling in the `slotmap` crate dependency.

```
SourceRegistry
├── slots: Vec<SourceSlot>     ← contiguous, cache-friendly iteration
│   ├── [0] SourceSlot { source: Some(Source{...}), generation: 1 }
│   ├── [1] SourceSlot { source: None, generation: 2 }  ← freed slot
│   ├── [2] SourceSlot { source: Some(Source{...}), generation: 1 }
│   └── ...
├── free_list: Vec<usize>      ← indices of None slots for O(1) reuse
│   └── [1]
└── active_count: usize        ← 2
```

Iteration skips `None` slots. Deterministic order is guaranteed because iteration follows slot index order (0, 1, 2, ...), which is independent of insertion/removal timing.

### Field Buffer Copy

The new `copy_read_to_write` method on `FieldBuffer`:

```rust
impl<T: Copy> FieldBuffer<T> {
    pub fn copy_read_to_write(&mut self) {
        let read_idx = self.current;
        let write_idx = self.current ^ 1;
        self.buffers[write_idx].copy_from_slice(&self.buffers[read_idx]);
    }
}
```

This is a single `memcpy` for the entire buffer. For a 100×100 grid, that's 10,000 × 4 bytes = 40 KB per field — well within L1 cache on Apple Silicon.

### Emission Injection

The `run_emission` function iterates sources and performs direct indexed writes:

```rust
pub fn run_emission(grid: &mut Grid, registry: &SourceRegistry) {
    for source in registry.iter() {
        match source.field {
            SourceField::Heat => {
                grid.write_heat()[source.cell_index] += source.emission_rate;
            }
            SourceField::Chemical(species) => {
                // Species index validated at registration time.
                if let Ok(buf) = grid.write_chemical(species) {
                    buf[source.cell_index] += source.emission_rate;
                }
            }
        }
    }
}
```

No allocation. No branching beyond the match. Sequential iteration over a small list — WARM classification is appropriate.

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Source validation accepts valid sources and rejects invalid ones

*For any* `Source` and grid configuration, `SourceRegistry::add()` shall succeed if and only if `cell_index < cell_count` AND (if `field` is `Chemical(species)`, then `species < num_chemicals`). Emission rate may be any finite f32 (positive, negative, or zero).

**Validates: Requirements 5.2, 5.3, 5.5**

### Property 2: Emission is additive injection into the correct field

*For any* grid state and any set of registered sources, after `run_emission` executes, the write buffer for each field at each cell shall equal the pre-emission read buffer value at that cell plus the sum of emission rates from all sources targeting that (field, cell) pair. Cells with no sources targeting them shall be unchanged from the read buffer.

This single property covers heat sources (Req 6.1–6.3) and chemical sources (Req 7.1–7.3), including multi-source additivity and negative-rate drains.

**Validates: Requirements 6.1, 6.2, 6.3, 7.1, 7.2, 7.3, 8.2**

### Property 3: Emission-tick integration — downstream systems process post-emission state

*For any* grid state with at least one active source, after `TickOrchestrator::step` completes, the resulting field values shall differ from what a sourceless tick would produce. Specifically, a heat source at cell C should cause the post-tick heat at C and its neighbors to be higher than a tick without that source (for positive emission rates).

**Validates: Requirements 8.1, 8.3**

### Property 4: Non-negative clamping for chemicals

*For any* grid state and any set of sources with negative emission rates (drains), after the emission phase completes, all chemical concentration values in the write buffers shall be ≥ 0.0. Heat values are unclamped (heat can go negative).

**Validates: Requirements 9.2**

### Property 5: Add/remove round-trip with count invariant

*For any* sequence of valid `add` and `remove` operations on a `SourceRegistry`, the following invariants hold:
- After `add(source)` returns `Ok(id)`, `remove(id)` shall return `Ok(())`
- After `remove(id)` succeeds, the source shall no longer appear in `iter()`
- `len()` shall equal the number of successful adds minus the number of successful removes at all times

**Validates: Requirements 10.1, 10.2, 10.4**

### Property 6: Deterministic iteration order

*For any* two `SourceRegistry` instances subjected to the same sequence of `add` and `remove` operations (same sources, same order), `iter()` shall yield sources in the same order. The iteration order is by slot index, independent of insertion timing.

**Validates: Requirements 11.1, 11.2**

## Error Handling

### New Error Type: `SourceError`

Defined in `src/grid/source.rs` using `thiserror`. Three variants:

| Variant | Trigger | Recovery |
|---|---|---|
| `CellOutOfBounds { cell_index, cell_count }` | `add()` with `cell_index >= cell_count` | Caller corrects the cell index |
| `InvalidChemicalSpecies { species, num_chemicals }` | `add()` with `Chemical(species)` where `species >= num_chemicals` | Caller corrects the species index |
| `InvalidSourceId { index, generation }` | `remove()` with stale or out-of-range id | Caller discards the stale id |

### Existing Error Type: `TickError`

The existing `TickError::NumericalError` variant is reused for NaN/infinity detection in the emission phase. The `system` field will be `"emission"` to distinguish from diffusion/heat errors.

### Error Propagation

- `SourceRegistry::add()` → `Result<SourceId, SourceError>`
- `SourceRegistry::remove()` → `Result<(), SourceError>`
- `Grid::add_source()` → `Result<SourceId, SourceError>`
- `Grid::remove_source()` → `Result<(), SourceError>`
- `run_emission_phase()` → `Result<(), TickError>` (NaN/infinity validation)
- `TickOrchestrator::step()` propagates `TickError` from emission phase via `?`

No panics. No `unwrap()` in simulation logic.

## Testing Strategy

### Property-Based Testing

Use the `proptest` crate for property-based testing. Each property test runs a minimum of 256 iterations (proptest default config, increased from 100 for better coverage of edge cases in floating-point arithmetic).

Generators needed:
- `arb_source(cell_count, num_chemicals)` — generates a `Source` with valid cell_index, random `SourceField` (Heat or Chemical), and random finite f32 emission_rate
- `arb_source_list(cell_count, num_chemicals, max_len)` — generates a `Vec<Source>` of random length
- `arb_grid_config()` — generates small grid configs (width/height 1–20, 1–4 chemicals) to keep tests fast
- `arb_add_remove_ops(cell_count, num_chemicals)` — generates a sequence of Add/Remove operations for the registry round-trip property

Each property test is tagged with a comment referencing the design property:
```rust
// Feature: grid-energy-sources, Property 1: Source validation
// Validates: Requirements 5.2, 5.3, 5.5
```

### Unit Tests

Unit tests complement property tests for specific examples and edge cases:

- NaN/infinity emission rates trigger `TickError` (edge case for Req 9.1)
- Double-removal of the same `SourceId` returns `SourceError::InvalidSourceId` (edge case for Req 10.3)
- Empty source registry results in a no-op emission phase (zero-cost path)
- Single source on a 1×1 grid (minimal case)
- `copy_read_to_write` produces identical buffers

### Test Organization

Tests live in `src/grid/source.rs` as `#[cfg(test)] mod tests`. Integration tests for the full tick with sources live in `tests/emission_integration.rs` (or inline in `tick.rs` tests if that module already has a test section).
