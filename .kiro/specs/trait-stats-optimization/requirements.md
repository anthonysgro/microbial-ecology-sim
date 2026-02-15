# Requirements Document

## Introduction

The `compute_trait_stats` system in the Bevy visualization layer is the largest self-authored CPU hotspot, consuming 3.72% of total profile time. It runs every simulation tick in `FixedUpdate`, performing 8 full O(n log n) sorts of the entire actor population to compute percentile statistics for a UI panel. This spec targets three optimizations: replacing full sorts with linear-time selection, throttling computation to a configurable interval, and consolidating the 8 separate collection passes into a single iteration.

## Glossary

- **Stats_System**: The `compute_trait_stats` Bevy system that computes population-level statistics for heritable traits each tick.
- **Stats_Computer**: The `compute_trait_stats_from_actors` function that collects trait values from actors and computes per-trait statistics.
- **Single_Stats_Computer**: The `compute_single_stats` function that computes min, max, mean, and percentiles (p25, p50, p75) for a single trait's value slice.
- **TraitStats**: The Bevy resource holding pre-computed population statistics for all 8 heritable traits.
- **SingleTraitStats**: A plain data struct holding min, max, mean, p25, p50, p75 for one trait.
- **Stats_Panel**: The UI panel toggled by pressing `T` that displays population trait statistics.
- **Tick_Counter**: A counter tracking elapsed simulation ticks since the last stats recomputation.
- **Stats_Interval**: A configurable number of ticks between stats recomputations.

## Requirements

### Requirement 1: Linear-Time Percentile Computation

**User Story:** As a developer running the simulation, I want percentile computation to use O(n) selection instead of O(n log n) sorting, so that the stats system consumes less CPU time per recomputation.

#### Acceptance Criteria

1. WHEN computing statistics for a trait value slice, THE Single_Stats_Computer SHALL compute min, max, and mean in a single streaming pass without sorting.
2. WHEN computing percentiles (p25, p50, p75) for a trait value slice, THE Single_Stats_Computer SHALL use `select_nth_unstable_by` (or equivalent O(n) selection algorithm) instead of a full sort.
3. THE Single_Stats_Computer SHALL produce a SingleTraitStats struct with the same fields (min, max, mean, p25, p50, p75) as the current implementation.
4. WHEN the trait value slice contains fewer than 4 elements, THE Single_Stats_Computer SHALL handle the degenerate case without panicking and produce valid statistics.

### Requirement 2: Throttled Recomputation

**User Story:** As a developer running the simulation, I want stats computation to run at a configurable interval rather than every tick, so that CPU time is not wasted recomputing a UI display panel that does not need per-tick updates.

#### Acceptance Criteria

1. THE Stats_System SHALL accept a configurable Stats_Interval specifying how many ticks elapse between recomputations.
2. WHEN the number of ticks since the last recomputation is less than Stats_Interval, THE Stats_System SHALL skip computation and retain the previous TraitStats value.
3. WHEN the number of ticks since the last recomputation equals or exceeds Stats_Interval, THE Stats_System SHALL recompute TraitStats from the current actor population.
4. THE Stats_Interval SHALL default to 10 ticks.
5. THE Stats_Interval SHALL be configurable via the `[bevy]` section of the TOML configuration file as `stats_update_interval`.
6. IF Stats_Interval is set to 0 or 1, THEN THE Stats_System SHALL recompute every tick (no throttling).

### Requirement 3: Single-Pass Collection

**User Story:** As a developer running the simulation, I want trait value collection to iterate over actors once instead of building 8 separate Vecs in a single pass that pushes to all 8, so that iterator overhead and cache pressure are reduced.

#### Acceptance Criteria

1. THE Stats_Computer SHALL collect all 8 trait values per actor in a single iteration over the actor population.
2. THE Stats_Computer SHALL pre-allocate collection buffers using the actor count to avoid incremental reallocation.
3. THE Stats_Computer SHALL skip inert actors during collection, consistent with current behavior.
4. THE Stats_Computer SHALL produce a TraitStats result identical in structure and field semantics to the current implementation.

### Requirement 4: Interface Compatibility

**User Story:** As a developer, I want the optimized stats system to maintain the same public interface, so that the stats panel formatting code and other consumers require no changes.

#### Acceptance Criteria

1. THE TraitStats resource SHALL retain its existing fields: `actor_count`, `tick`, and `traits: Option<[SingleTraitStats; 8]>`.
2. THE SingleTraitStats struct SHALL retain its existing fields: `min`, `max`, `mean`, `p25`, `p50`, `p75`.
3. WHEN no living actors exist, THE Stats_Computer SHALL return a TraitStats with `actor_count: 0` and `traits: None`.
4. THE Stats_System SHALL remain registered in the `FixedUpdate` schedule, ordered after `tick_simulation`.

### Requirement 5: Simulation Determinism Preservation

**User Story:** As a developer, I want the optimization to have zero impact on simulation determinism, so that replay and debugging remain reliable.

#### Acceptance Criteria

1. THE Stats_System SHALL not read from or write to any simulation state (Grid, Actor components, RNG) beyond read-only access to actor trait values.
2. THE Stats_System SHALL not alter the execution order or data flow of any simulation system.

### Requirement 6: Configuration Documentation

**User Story:** As a developer, I want the new `stats_update_interval` configuration field documented, so that it is discoverable and consistent with existing config documentation.

#### Acceptance Criteria

1. WHEN `stats_update_interval` is added to `BevyExtras`, THE example_config.toml SHALL include the new field with a comment explaining its purpose and valid range.
2. WHEN `stats_update_interval` is added, THE `format_config_info()` info panel in `src/viz_bevy/setup.rs` SHALL display the stats update interval value.
