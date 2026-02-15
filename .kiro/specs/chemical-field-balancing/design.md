# Design Document: Chemical Field Balancing

## Overview

This design introduces two changes to restore chemical field equilibrium:

1. A per-species exponential decay system that runs after diffusion each tick, removing a configurable fraction of chemical concentration from every cell.
2. Rebalanced default `consumption_rate` in `ActorConfig` (0.1 → 1.5) so actors are meaningful sinks.

The decay system follows the existing pattern: stateless function, double-buffered read→write, validate, swap. It slots into `TickOrchestrator::step()` between diffusion-swap and heat.

## Architecture

### Tick Phase Ordering (Updated)

```
Phase 0: Emission        (WARM) — inject source values
Phase 1–4: Actor phases  (WARM) — sensing, metabolism, removal, movement
Phase 5: Diffusion       (HOT)  — redistribute chemicals, validate, swap
Phase 6: Chemical Decay  (HOT)  — apply per-species decay, validate, swap  ← NEW
Phase 7: Heat            (HOT)  — radiate heat, validate, swap
```

Decay runs after diffusion because:
- Diffusion redistributes mass but doesn't remove it. Decay is the removal step.
- Running decay after diffusion means the decayed field is what heat and the next tick's emission see — clean separation of concerns.
- The decay phase reads from the post-diffusion read buffer (after diffusion swap) and writes to the write buffer, following the same double-buffer discipline.

### Data Flow for Decay Phase

```mermaid
graph LR
    A[Diffusion swap completes] --> B[Copy read → write for each species]
    B --> C[Apply decay: write[i] *= 1.0 - decay_rate]
    C --> D[Clamp write[i] to >= 0.0]
    D --> E[Validate write buffer: no NaN/Inf]
    E --> F[Swap chemical buffers]
    F --> G[Heat phase reads post-decay state]
```

## Components and Interfaces

### Modified: `GridConfig` (`src/grid/config.rs`)

Add a `chemical_decay_rates: Vec<f32>` field. One entry per chemical species. Each value in [0.0, 1.0].

```rust
pub struct GridConfig {
    // ... existing fields ...
    /// Per-species chemical decay rate. Length must equal `num_chemicals`.
    /// Each value in [0.0, 1.0]. Applied as `concentration *= (1.0 - rate)` per tick.
    pub chemical_decay_rates: Vec<f32>,
}
```

### Modified: `Grid::new()` (`src/grid/mod.rs`)

Validate `chemical_decay_rates.len() == num_chemicals` and each rate in [0.0, 1.0]. Return `GridError` on violation.

### New: `GridError` variants (`src/grid/error.rs`)

```rust
#[error("chemical_decay_rates length {got} does not match num_chemicals {expected}")]
DecayRateCountMismatch { got: usize, expected: usize },

#[error("chemical decay rate for species {species} is {rate}, must be in [0.0, 1.0]")]
InvalidDecayRate { species: usize, rate: f32 },
```

### New: `run_decay()` (`src/grid/decay.rs`)

```rust
// HOT PATH: Executes per tick over all grid cells for each chemical species.
// Allocation forbidden. Dynamic dispatch forbidden.
// Deterministic execution required.

/// Apply exponential decay to all chemical species.
///
/// For each species with a non-zero decay rate:
/// 1. Copy read buffer → write buffer
/// 2. Multiply each cell in the write buffer by (1.0 - decay_rate)
/// 3. Clamp to >= 0.0
///
/// Species with decay_rate == 0.0 are skipped entirely (no copy, no write).
/// Caller is responsible for validation and swap after this function returns.
pub fn run_decay(grid: &mut Grid, config: &GridConfig) -> Result<(), TickError>
```

The function iterates species in index order. For each species where `decay_rate > 0.0`:
- `copy_read_to_write()` on the chemical buffer
- Iterate the write slice: `val *= 1.0 - decay_rate; if val < 0.0 { val = 0.0; }`

This is a HOT path because it touches every cell for every decaying species. The inner loop is branchless (the clamp compiles to a `maxss` on x86 / `fmax` on ARM). No allocations, no dynamic dispatch.

### Modified: `TickOrchestrator::step()` (`src/grid/tick.rs`)

Insert decay phase between diffusion-swap and heat:

```rust
// Phase 5: Chemical diffusion
run_diffusion(grid, config)?;
// ... validate, swap ...

// Phase 6: Chemical decay (NEW)
run_decay(grid, config)?;
for species in 0..config.num_chemicals {
    let write_buf = grid.write_chemical(species).expect("...");
    validate_buffer(write_buf, "decay", field_name)?;
}
grid.swap_chemicals();

// Phase 7: Heat radiation
run_heat(grid, config)?;
// ... validate, swap ...
```

### Modified: `ActorConfig` defaults

Update `consumption_rate` from `0.1` to `1.5` in:
- `src/main.rs`
- `src/bin/bevy_viz.rs`

No structural change to `ActorConfig` — just the literal value at construction sites.

### Modified: `WorldInitConfig` defaults

No change needed. The `WorldInitConfig::default()` emission range (0.1–5.0) remains. The rebalancing comes from the consumption side and decay.

## Data Models

### GridConfig (updated)

| Field | Type | Description |
|---|---|---|
| `width` | `u32` | Grid columns |
| `height` | `u32` | Grid rows |
| `num_chemicals` | `usize` | Chemical species count |
| `diffusion_rate` | `f32` | Diffusion coefficient |
| `thermal_conductivity` | `f32` | Heat radiation coefficient |
| `ambient_heat` | `f32` | Boundary heat value |
| `tick_duration` | `f32` | Simulated seconds per tick |
| `num_threads` | `usize` | Spatial partition count |
| `chemical_decay_rates` | `Vec<f32>` | Per-species decay rate [0.0, 1.0] — **NEW** |

### Decay Rate Semantics

- `0.0` = no decay (chemical is persistent)
- `0.01` = 1% removed per tick (slow decay, half-life ≈ 69 ticks)
- `0.1` = 10% removed per tick (moderate decay, half-life ≈ 6.6 ticks)
- `1.0` = 100% removed per tick (instant removal — useful for testing)

Half-life formula: `t_half = ln(2) / ln(1 / (1 - rate))` ≈ `ln(2) / rate` for small rates.

### Steady-State Analysis

At equilibrium, emission = decay + consumption per cell (averaged):
- `E = C * r + A * c` where E = emission rate, C = concentration, r = decay rate, A = actors consuming, c = consumption rate
- Solving: `C = (E - A*c) / r` (when `E > A*c`, otherwise concentration → 0)

With defaults: E ≈ 2.5 avg per source cell, r = 0.05, c = 1.5, a few actors:
- A source cell with no actors: `C ≈ 2.5 / 0.05 = 50.0` (bounded)
- A source cell with 2 actors: `C ≈ (2.5 - 3.0) / 0.05` → concentration driven to 0 (actors deplete it)

This confirms the design achieves the goal: bounded concentrations with meaningful actor impact.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Properties were derived from the acceptance criteria prework analysis. Redundant properties were consolidated:
- Config validation (1.2, 1.3, 1.4) combined into one property covering all invalid config shapes.
- Zero-rate skip (2.5) subsumes the all-zeros no-op case (1.5).
- Mass conservation (4.3) is a direct consequence of the decay computation (2.1) — if each cell is multiplied by `(1 - r)`, total removed = `sum(c[i] * r)`. Kept 2.1 as the primary property.
- Gradient preservation (4.2) is implied by bounded convergence (4.1) when sources are present. Dropped 4.2.

### Property 1: Invalid decay config rejects construction

*For any* `GridConfig` where `chemical_decay_rates.len() != num_chemicals` OR any element is outside [0.0, 1.0], `Grid::new()` SHALL return an error and refuse construction.

**Validates: Requirements 1.2, 1.3, 1.4**

### Property 2: Decay computation correctness

*For any* grid with non-negative chemical concentrations and valid decay rates in (0.0, 1.0], after `run_decay` executes, each cell's concentration for species `s` SHALL equal `original_concentration * (1.0 - decay_rate[s])`, clamped to >= 0.0, within floating-point tolerance.

**Validates: Requirements 2.1, 2.6, 4.3**

### Property 3: Zero-rate species are unchanged

*For any* grid with mixed decay rates where some species have `decay_rate == 0.0`, after `run_decay` executes, the concentrations for zero-rate species SHALL be bitwise identical to their pre-decay values, while non-zero-rate species SHALL be decayed.

**Validates: Requirements 1.5, 2.5**

### Property 4: Bounded convergence under constant emission

*For any* grid with at least one source emitting at a constant rate and a non-zero decay rate, running N ticks (N >= 100) SHALL produce chemical concentrations that are bounded above by `emission_rate / decay_rate` (per source cell) within floating-point tolerance, and the maximum concentration SHALL not grow monotonically after an initial transient.

**Validates: Requirements 4.1, 4.2**

## Error Handling

### Configuration Errors

| Error | Condition | Response |
|---|---|---|
| `GridError::DecayRateCountMismatch` | `chemical_decay_rates.len() != num_chemicals` | Return error from `Grid::new()`, refuse construction |
| `GridError::InvalidDecayRate` | Any rate < 0.0 or > 1.0 | Return error from `Grid::new()`, refuse construction |

### Runtime Errors

| Error | Condition | Response |
|---|---|---|
| `TickError::NumericalError` | NaN or infinity in chemical write buffer after decay | Halt tick, preserve read buffer as last-known-good state |

All errors use `thiserror` and propagate via `Result`. No panics in simulation logic. The decay system follows the same error discipline as diffusion and heat: validate write buffers before swap, return `TickError` on detection.

## Testing Strategy

### Property-Based Testing

Use `proptest` (already in `dev-dependencies`). Each property test runs a minimum of 100 iterations with generated inputs.

| Property | Test Approach | Generator Strategy |
|---|---|---|
| Property 1: Invalid config rejection | Generate `num_chemicals` in [0, 8], then generate `chemical_decay_rates` with either wrong length or out-of-range values. Verify `Grid::new()` returns the expected error variant. | `proptest::collection::vec` for rates, `prop::num::f32` for out-of-range values |
| Property 2: Decay computation | Generate a small grid (4×4 to 16×16), random concentrations in [0.0, 100.0], random decay rates in (0.0, 1.0]. Run `run_decay`, compare each cell to `original * (1 - rate)` within `f32::EPSILON * original` tolerance. | `proptest::collection::vec` for concentrations, `0.001..=1.0f32` for rates |
| Property 3: Zero-rate unchanged | Generate a grid with 2+ chemical species, set some decay rates to 0.0 and others to non-zero. Run `run_decay`, verify zero-rate species are bitwise identical, non-zero species are decayed. | Mix of `Just(0.0)` and `0.001..=1.0f32` for rates |
| Property 4: Bounded convergence | Generate a small grid with 1–3 sources, constant emission, non-zero decay. Run 200 ticks. Verify max concentration <= `max_emission / min_decay_rate` and that max concentration at tick 200 <= max concentration at tick 100 (not growing). | `0.1..=5.0f32` for emission, `0.01..=0.5f32` for decay |

Tag format for each test: `// Feature: chemical-field-balancing, Property {N}: {title}`

### Unit Tests

Unit tests complement property tests for specific examples and edge cases:

- Decay with rate 1.0 zeroes all concentrations (edge case for Property 2)
- Decay with rate 0.0 for all species is a no-op (edge case for Property 3)
- NaN concentration input triggers `TickError::NumericalError` (error condition for Requirement 2.3)
- Single-cell grid with one source reaches expected steady state within tolerance (concrete example for Property 4)
- `ActorConfig` construction in `main.rs` and `bevy_viz.rs` uses `consumption_rate = 1.5` (Requirement 3.1, 3.3)

### Test Organization

- Property tests: `tests/chemical_decay_props.rs` (integration test file, accesses public API)
- Unit tests: inline `#[cfg(test)] mod tests` in `src/grid/decay.rs` and `src/grid/config.rs`
- Each property-based test is a single `proptest!` macro invocation referencing one design property
- Minimum 100 cases per property (`PROPTEST_CASES=100` or `ProptestConfig::with_cases(100)`)
