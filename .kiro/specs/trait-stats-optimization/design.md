# Design Document: Trait Stats Optimization

## Overview

This design optimizes the `compute_trait_stats` system — the largest self-authored CPU hotspot at 3.72% of total profile time. Three independent optimizations are applied:

1. Replace 8× O(n log n) full sorts with O(n) streaming min/max/mean + O(n) `select_nth_unstable_by` for percentiles.
2. Throttle recomputation to a configurable tick interval (default: every 10 ticks).
3. Consolidate 8 separate `Vec` collections into a single actor iteration with pre-allocated buffers.

All changes are confined to the COLD visualization path. Simulation logic is untouched. The `TraitStats` and `SingleTraitStats` public interfaces remain unchanged.

## Architecture

The optimization touches three layers:

```
┌─────────────────────────────────────────────────┐
│  Config Layer (io/config_file.rs, main.rs)      │
│  BevyExtras + BevyVizConfig gain               │
│  stats_update_interval field                     │
├─────────────────────────────────────────────────┤
│  Resource Layer (viz_bevy/resources.rs)          │
│  New: StatsTickCounter resource                  │
│  Unchanged: TraitStats, SingleTraitStats         │
├─────────────────────────────────────────────────┤
│  System Layer (viz_bevy/systems.rs)              │
│  compute_trait_stats: throttle gate              │
│  compute_trait_stats_from_actors: single-pass    │
│  compute_single_stats: O(n) selection            │
├─────────────────────────────────────────────────┤
│  Documentation Layer                             │
│  example_config.toml, format_config_info(),      │
│  config-documentation.md steering file           │
└─────────────────────────────────────────────────┘
```

Data flow remains identical: `compute_trait_stats` reads `SimulationState` (actors), writes `TraitStats`. The throttle gate adds a `StatsTickCounter` resource check before the existing flow.

## Components and Interfaces

### Modified: `compute_single_stats`

Current signature preserved:

```rust
fn compute_single_stats(values: &mut [f32]) -> SingleTraitStats
```

New algorithm:
1. Single streaming pass: compute `min`, `max`, `sum` in one iteration. Derive `mean = sum / n`.
2. Use `values.select_nth_unstable_by(idx, f32::total_cmp)` for each percentile index. This partitions the slice in O(n) average time such that the element at `idx` is in its sorted position, elements before are ≤, elements after are ≥.
3. Compute p50 first (median), then p25 on the left partition, then p75 on the right partition. Each `select_nth_unstable_by` call operates on a progressively smaller slice because the previous call already partitioned the data.

Percentile index calculation (unchanged): `(n - 1) * P / 100` for P ∈ {25, 50, 75}.

Degenerate cases (n < 4): when the slice is very small, percentile indices collapse. The index formula still produces valid indices for n ≥ 1. For n = 0, the caller (`compute_trait_stats_from_actors`) already returns `traits: None` before calling this function.

### Modified: `compute_trait_stats_from_actors`

Current signature preserved:

```rust
pub fn compute_trait_stats_from_actors<'a>(
    actors: impl Iterator<Item = &'a Actor>,
    tick: u64,
) -> TraitStats
```

Changes:
- Pre-allocate 8 `Vec<f32>` buffers using a size hint from the iterator (or a reasonable default).
- Single iteration: for each non-inert actor, push all 8 trait values in one loop body.
- This is functionally identical to the current implementation but avoids relying on the optimizer to fuse 8 pushes — the explicit single loop makes the intent clear and guarantees a single pass.

### New Resource: `StatsTickCounter`

```rust
#[derive(Resource)]
pub struct StatsTickCounter {
    pub ticks_since_update: u64,
    pub interval: u64,
}
```

- `interval`: loaded from `BevyVizConfig.stats_update_interval`. Default: 10.
- `ticks_since_update`: incremented each tick. Reset to 0 when stats are recomputed.
- When `interval <= 1`, stats recompute every tick (no throttling).

### Modified: `compute_trait_stats` (Bevy system)

```rust
pub fn compute_trait_stats(
    sim: Res<SimulationState>,
    mut stats: ResMut<TraitStats>,
    mut counter: ResMut<StatsTickCounter>,
)
```

Logic:
1. Increment `counter.ticks_since_update`.
2. If `counter.interval > 1 && counter.ticks_since_update < counter.interval`, return early (retain previous `TraitStats`).
3. Otherwise, reset counter to 0, recompute stats via `compute_trait_stats_from_actors`.

### Modified: `BevyExtras` (config)

Add field:

```rust
#[serde(default = "default_stats_update_interval")]
pub stats_update_interval: u64,
```

Default: `10`. Propagated through `BevyVizConfig` → `StatsTickCounter` at startup.

### Modified: `BevyVizConfig`

Add field:

```rust
pub stats_update_interval: u64,
```

Wired from `BevyExtras` in `main.rs`.

### Modified: `setup::setup`

Insert `StatsTickCounter` resource during Bevy startup, reading `stats_update_interval` from `BevyVizConfig`.

### Modified: `format_config_info`

Add `stats_update_interval` display to the Bevy section of the info panel. Requires the value to be accessible — either passed as a parameter or read from a resource.

## Data Models

### `SingleTraitStats` (unchanged)

```rust
#[derive(Debug, Clone, Copy)]
pub struct SingleTraitStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
}
```

### `TraitStats` (unchanged)

```rust
#[derive(Resource, Debug, Clone)]
pub struct TraitStats {
    pub actor_count: usize,
    pub tick: u64,
    pub traits: Option<[SingleTraitStats; 8]>,
}
```

### `StatsTickCounter` (new)

```rust
#[derive(Resource)]
pub struct StatsTickCounter {
    pub ticks_since_update: u64,
    pub interval: u64,
}
```

Plain data resource. No methods beyond construction. Inserted at startup, mutated by `compute_trait_stats` system only.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: `compute_single_stats` equivalence (model-based)

*For any* non-empty `Vec<f32>` of finite values, the optimized `compute_single_stats` (streaming min/max/mean + `select_nth_unstable_by` percentiles) SHALL produce a `SingleTraitStats` with identical `min`, `max`, `mean`, `p25`, `p50`, `p75` values as the reference sort-based implementation.

The reference implementation is: sort the slice, then read min as `values[0]`, max as `values[n-1]`, mean as `sum/n`, and percentiles at indices `(n-1)*P/100`.

Edge cases to include in the generator: slices of length 1, 2, 3 (degenerate percentile indices), slices with all identical values, slices with negative values, slices with values near f32 epsilon.

**Validates: Requirements 1.1, 1.2, 1.4**

### Property 2: Throttle gate correctness

*For any* `StatsTickCounter` with `interval > 1`, and *for any* sequence of tick increments, the stats system SHALL recompute if and only if `ticks_since_update >= interval`. Specifically:
- After `interval - 1` increments from reset, `ticks_since_update < interval` and stats are NOT recomputed.
- After `interval` increments from reset, `ticks_since_update >= interval`, stats ARE recomputed, and the counter resets to 0.

**Validates: Requirements 2.2, 2.3, 2.6**

### Property 3: Inert actor exclusion

*For any* collection of actors where some are marked `inert = true` and others `inert = false`, `compute_trait_stats_from_actors` SHALL produce a `TraitStats` where `actor_count` equals the number of non-inert actors, and the computed statistics reflect only the trait values of non-inert actors.

This is verified by comparing against a reference that manually filters out inert actors before computing stats.

**Validates: Requirements 3.3, 3.4, 4.3**

## Error Handling

This optimization is entirely within the COLD visualization path. Error conditions are minimal:

- **Empty actor set**: Already handled — `compute_trait_stats_from_actors` returns `TraitStats { actor_count: 0, tick, traits: None }` when no non-inert actors exist. No change needed.
- **Invalid config value**: `stats_update_interval` is a `u64`, so negative values are impossible. A value of 0 is treated as "every tick" (same as 1). No validation error needed.
- **f32 edge cases**: `select_nth_unstable_by` with `f32::total_cmp` handles NaN and infinity correctly (NaN sorts to the end). Since trait values are clamped by `ActorConfig` bounds, NaN/infinity should not appear in practice, but the algorithm is robust regardless.

No new error types are introduced. No `Result` return types are needed — this is infallible computation on validated data.

## Testing Strategy

### Property-Based Tests

Use the `proptest` crate (already idiomatic for Rust property testing). Each property test runs a minimum of 100 iterations.

- **Property 1** (`compute_single_stats` equivalence): Generate random `Vec<f32>` of varying lengths (1..1000) with finite f32 values. Compare optimized output against sort-based reference. Tag: `Feature: trait-stats-optimization, Property 1: compute_single_stats equivalence`.

- **Property 2** (throttle gate): Generate random `interval` values (2..100) and random tick sequences. Simulate the counter logic and verify recomputation occurs at the correct boundaries. Tag: `Feature: trait-stats-optimization, Property 2: throttle gate correctness`.

- **Property 3** (inert actor exclusion): Generate random actor collections with mixed inert/active status. Compare `compute_trait_stats_from_actors` output against a filtered reference. Tag: `Feature: trait-stats-optimization, Property 3: inert actor exclusion`.

### Unit Tests

- Default `stats_update_interval` is 10.
- TOML parsing of `stats_update_interval` field.
- `compute_single_stats` with n=1, n=2, n=3 (degenerate percentile indices).
- `compute_single_stats` with all-identical values.
- Empty actor set returns `traits: None`.
- `stats_update_interval = 0` and `= 1` both result in every-tick recomputation.
- `format_config_info` output includes `stats_update_interval`.

### Testing Library

- `proptest` for property-based testing (standard Rust PBT library).
- Standard `#[cfg(test)] mod tests` with `#[test]` for unit tests.
