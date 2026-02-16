# Design Document: Trait Stats Standard Deviation

## Overview

This feature adds a `std_dev` field to `SingleTraitStats` and surfaces it in the Bevy stats panel. The change is confined to three files in the `viz_bevy` module — no simulation logic is affected. The computation is COLD path (runs at most once per `stats_update_interval` ticks), so a second pass over the values buffer for variance accumulation is acceptable.

## Architecture

No architectural changes. The existing data flow is preserved:

```
compute_trait_stats (FixedUpdate)
  → compute_trait_stats_from_actors
    → compute_single_stats (per trait buffer)
  → TraitStats resource updated

update_stats_panel (Update)
  → format_trait_stats(TraitStats, PredationCounter)
  → StatsPanel text updated
```

The only modification is widening `SingleTraitStats` by one `f32` field and extending `compute_single_stats` to compute it.

## Components and Interfaces

### Modified: `SingleTraitStats` (`src/viz_bevy/resources.rs`)

```rust
#[derive(Debug, Clone, Copy)]
pub struct SingleTraitStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
    pub std_dev: f32, // NEW
}
```

### Modified: `compute_single_stats` (`src/viz_bevy/systems.rs`)

Current implementation computes min, max, sum in a single streaming pass, then derives percentiles via `select_nth_unstable_by`. The mean is already available before the percentile step.

The std dev computation adds a second pass after mean is known:

```rust
// After mean is computed:
let variance = values.iter().map(|&v| {
    let diff = v - mean;
    diff * diff
}).sum::<f32>() / n as f32;
let std_dev = variance.sqrt();
```

This is population standard deviation (dividing by `n`, not `n-1`), which is correct here — we are computing statistics over the entire living population, not estimating from a sample.

For `n == 1`, the sum of squared differences is `0.0`, so `std_dev` is `0.0` without special-casing.

### Modified: `format_trait_stats` (`src/viz_bevy/setup.rs`)

Each trait row and the energy row gain a `std: X.XX` column appended to the existing format string.

## Data Models

No new data models. `SingleTraitStats` gains one `f32` field. `TraitStats` is unchanged structurally — it holds `[SingleTraitStats; 12]` and `Option<SingleTraitStats>`, both of which automatically include the new field.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Standard deviation computation matches reference implementation

*For any* non-empty `Vec<f32>` of finite values, the `std_dev` field returned by `compute_single_stats` should equal the population standard deviation computed by an independent reference: `sqrt(sum((x_i - mean)^2) / n)`, within floating-point tolerance (`1e-4` absolute).

**Validates: Requirements 1.2**

### Property 2: Formatted stats output includes std_dev for all rows

*For any* `TraitStats` with `Some` traits and `Some` energy_stats, every non-header line produced by `format_trait_stats` that contains a trait name or "energy" should contain a substring matching the pattern `std: ` followed by a decimal number with two decimal places.

**Validates: Requirements 2.1, 2.2, 2.3**

## Error Handling

No new error paths. `compute_single_stats` is only called when `actor_count > 0`, so division by zero is structurally impossible. The `std_dev` computation uses the same `n` as the existing `mean` computation. NaN/Inf from degenerate float inputs would propagate the same way existing stats do — this is acceptable for a COLD visualization path.

## Testing Strategy

### Property-Based Tests

Use the `proptest` crate (already available in the Rust ecosystem, standard for Rust PBT). Each property test runs a minimum of 100 iterations.

- **Property 1** test: Generate random `Vec<f32>` (length 1..200, values in a reasonable finite range like `-1e6..1e6`). Call `compute_single_stats`, compare `std_dev` against a naive reference implementation. Tolerance: `1e-4` absolute or `1e-3` relative, whichever is larger (to handle floating-point accumulation differences on large inputs).
  - Tag: `Feature: trait-stats-stddev, Property 1: Standard deviation computation matches reference implementation`

- **Property 2** test: Generate random `TraitStats` with populated traits and energy_stats. Call `format_trait_stats`, verify every data row contains `std: ` followed by a two-decimal-place number.
  - Tag: `Feature: trait-stats-stddev, Property 2: Formatted stats output includes std_dev for all rows`

### Unit Tests

- Edge case: single-element input → `std_dev == 0.0`
- Edge case: all-identical values → `std_dev == 0.0`
- Known-value test: `[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]` → `std_dev ≈ 2.0`
