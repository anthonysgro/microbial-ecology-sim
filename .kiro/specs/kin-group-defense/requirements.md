# Requirements Document

## Introduction

The current contact predation system is deterministic: if a predator has energy dominance and sufficient genetic distance, predation always succeeds. This creates a runaway selection pressure where `kin_tolerance` evolves toward its minimum (0.0) because indiscriminate predation carries no cost. Actors that eat everyone gain energy and reproduce more, with no inclusive fitness counterbalance.

This feature introduces a probabilistic group defense mechanic: prey survival chance scales with the defense strength of genetically allied neighbors, analogous to herd defense in biological systems. When a predator attempts to eat prey, the prey's Von Neumann neighbors (excluding the predator) that are genetically close (below the prey's `kin_tolerance`) contribute their heritable `kin_group_defense` trait value to a cumulative defense score that reduces predation success probability.

This mechanic satisfies Hamilton's Rule through purely local interactions: actors that spare nearby relatives (high `kin_tolerance`) form clusters that provide mutual predation defense, while xenophobic actors (low `kin_tolerance`) eat their neighbors, end up isolated, and become easy prey. The heritable `kin_group_defense` trait creates additional evolutionary pressure: prey lineages evolve higher defense values because relatives share traits and defend each other, while lineages that invest nothing in defense provide no protection to kin.

The mechanic degrades gracefully: zero allied neighbors yields 100% success probability, which is identical to current behavior. The mechanic is always active — no feature flag is needed.

## Glossary

- **Predation_System**: The `run_contact_predation` function in `src/grid/actor_systems.rs`.
- **Predator**: An actor attempting to consume an adjacent actor via contact predation.
- **Prey**: An actor targeted by a Predator for consumption.
- **Predation_Attempt**: A single evaluation of predation eligibility between a Predator and an adjacent Prey. Occurs when energy dominance and genetic distance conditions are met.
- **Allied_Neighbor**: An actor occupying a Von Neumann neighbor cell of the Prey (excluding the Predator) whose genetic distance to the Prey is below the Prey's own `kin_tolerance` threshold.
- **Ally_Defense_Sum**: The sum of `kin_group_defense` trait values across all Allied_Neighbors for a given Prey in a given Predation_Attempt.
- **Success_Probability**: The probability that a Predation_Attempt results in successful predation. Computed as `1.0 / (1.0 + ally_defense_sum)`.
- **Occupancy_Map**: The `Vec<Option<usize>>` mapping cell indices to actor slot indices.
- **Tick_RNG**: The per-tick deterministic `ChaCha8Rng` seeded from `grid.seed().wrapping_add(tick)`.

## Requirements

### Requirement 1: Group Defense Mechanic

**User Story:** As a simulation designer, I want prey survival probability to scale with the defense strength of genetically allied neighbors, so that kin clusters provide mutual defense against predation and defense investment is evolvable.

#### Acceptance Criteria

1. WHEN a Predation_Attempt occurs, THE Predation_System SHALL identify Allied_Neighbors by scanning the Prey's Von Neumann 4-neighborhood via the Occupancy_Map, excluding the Predator, and selecting non-inert actors whose genetic distance to the Prey is below the Prey's `kin_tolerance`.
2. THE Predation_System SHALL compute Ally_Defense_Sum as the sum of each Allied_Neighbor's heritable `kin_group_defense` trait value.
3. THE Predation_System SHALL compute Success_Probability as `1.0 / (1.0 + ally_defense_sum)`.
4. THE Predation_System SHALL determine predation outcome by sampling a uniform `[0.0, 1.0)` value from the Tick_RNG and succeeding only if the sampled value is less than Success_Probability.
5. WHEN the Prey has zero Allied_Neighbors, THE Predation_System SHALL compute Success_Probability as `1.0` (isolated prey is always caught).
6. WHEN the Prey occupies a boundary cell with fewer than four Von Neumann neighbors, THE Predation_System SHALL count only valid in-bounds cells when computing Allied_Neighbors (out-of-bounds cells contribute zero defense).
7. THE Predation_System SHALL cap the number of Allied_Neighbors considered to a maximum of 3 (since one Von Neumann neighbor is the Predator).

### Requirement 2: Heritable Kin Group Defense Trait

**User Story:** As a simulation designer, I want `kin_group_defense` to be a heritable trait on each actor, so that defense investment evolves under natural selection and relatives share defense capability.

#### Acceptance Criteria

1. THE `HeritableTraits` struct SHALL include a `kin_group_defense` field of type `f32`.
2. THE `ActorConfig` SHALL include a `kin_group_defense` field as the seed genome default for the heritable trait, with a default value of `0.5`.
3. THE `ActorConfig` SHALL include `trait_kin_group_defense_min` (default `0.0`) and `trait_kin_group_defense_max` (default `1.0`) clamp bounds for the heritable trait.
4. THE `genetic_distance` function SHALL include `kin_group_defense` in its trait vector, incrementing `TRAIT_COUNT` from 11 to 12.
5. THE mutation system SHALL mutate `kin_group_defense` during binary fission using the same Gaussian noise mechanism as other heritable traits, clamped to `[trait_kin_group_defense_min, trait_kin_group_defense_max]`.

### Requirement 3: Determinism

**User Story:** As a simulation developer, I want the probabilistic predation mechanic to be fully deterministic given the same seed, so that simulation replay and debugging remain reliable.

#### Acceptance Criteria

1. THE Predation_System SHALL consume RNG samples in ascending slot-index order, matching the existing iteration order of the predation loop.
2. THE Predation_System SHALL consume exactly one RNG sample per Predation_Attempt, regardless of Ally_Defense_Sum.
3. FOR ALL executions with identical seed, tick number, and actor state, THE Predation_System SHALL produce identical predation outcomes.

### Requirement 4: Function Signature Update

**User Story:** As a simulation developer, I want `run_contact_predation` to accept an RNG parameter, so that the probabilistic check uses the simulation's deterministic per-tick RNG.

#### Acceptance Criteria

1. THE `run_contact_predation` function signature SHALL accept an additional `rng: &mut impl Rng` parameter.
2. THE call site in `run_actor_phases` (`src/grid/tick.rs`) SHALL pass the existing `tick_rng` to `run_contact_predation`.

### Requirement 5: Two-Pass Architecture Preservation

**User Story:** As a simulation developer, I want the two-pass predation architecture (read-only collection, then mutation) to be preserved, so that the system remains correct and maintainable.

#### Acceptance Criteria

1. THE Predation_System SHALL compute Ally_Defense_Sum and determine predation outcome during pass 1 (the read-only collection pass) alongside existing eligibility checks.
2. THE Predation_System SHALL store the predation outcome (success/failure) determined during pass 1 and apply mutations only during pass 2.

### Requirement 6: Numerical Safety

**User Story:** As a simulation developer, I want the predation system to maintain numerical safety, so that energy values remain valid after predation energy transfer.

#### Acceptance Criteria

1. WHEN the Predator's energy after energy gain from successful predation is NaN or infinite, THE Predation_System SHALL return a `TickError::NumericalError`.
2. THE Predation_System SHALL clamp the Predator's final energy to `max_energy` after applying energy gain from successful predation.

### Requirement 7: Configuration Documentation

**User Story:** As a simulation user, I want the new heritable trait to be documented in all configuration surfaces, so that I can understand and tune the `kin_group_defense` parameter.

#### Acceptance Criteria

1. THE `example_config.toml` SHALL include `kin_group_defense`, `trait_kin_group_defense_min`, and `trait_kin_group_defense_max` fields with explanatory comments.
2. THE `format_config_info()` function in `src/viz_bevy/setup.rs` SHALL display the `kin_group_defense` seed default and clamp bounds in the world config info panel.

### Requirement 8: Trait Visualization Update

**User Story:** As a simulation user, I want population-level statistics for `kin_group_defense` displayed in the stats panel and actor inspector, so that I can observe how defense investment evolves across the population.

#### Acceptance Criteria

1. THE `compute_trait_stats_from_actors` function SHALL collect `kin_group_defense` values and compute population statistics (min, p25, p50, p75, max, mean).
2. THE `TraitStats.traits` array size SHALL increase from 11 to 12 to accommodate the new trait.
3. THE `format_trait_stats` function SHALL display a `kin_group_defense` row in the stats panel.
4. THE `format_actor_info` function SHALL display the selected actor's `kin_group_defense` value in the actor inspector panel.
