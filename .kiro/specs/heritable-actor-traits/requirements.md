# Requirements Document

## Introduction

The simulation currently treats all actors identically — every behavioral parameter comes from a shared, immutable `ActorConfig`. This prevents natural selection from operating because there is no phenotypic variation for selection to act on. This feature introduces per-actor heritable traits: a small struct of mutable parameters that each actor carries individually, inherits from its parent during binary fission with small gaussian mutations, and that the simulation systems read instead of the global config for those specific fields. The global `ActorConfig` values serve as the seed genome for actors created at world initialization.

## Glossary

- **Actor**: A mobile biological agent occupying one grid cell, stored in the `ActorRegistry`.
- **ActorConfig**: The global, immutable configuration struct providing default parameters for all actors.
- **Heritable_Traits**: A per-actor struct containing the four mutable trait fields: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`.
- **Trait_Mutation**: A small gaussian perturbation applied to each heritable trait value during binary fission, producing offspring variation.
- **Seed_Actor**: An actor created during world initialization (not via fission).
- **Trait_Clamp_Range**: The valid minimum and maximum bounds for each heritable trait, preventing runaway drift.
- **Mutation_Stddev**: The standard deviation of the gaussian noise applied per trait during fission.
- **Fission**: Binary reproduction where a parent actor splits, producing one offspring actor.

## Requirements

### Requirement 1: Per-Actor Heritable Traits Struct

**User Story:** As a simulation architect, I want each actor to carry its own heritable trait values, so that individual variation exists for natural selection to act on.

#### Acceptance Criteria

1. THE Heritable_Traits struct SHALL contain exactly four `f32` fields: `consumption_rate`, `base_energy_decay`, `levy_exponent`, `reproduction_threshold`.
2. THE Heritable_Traits struct SHALL derive `Debug`, `Clone`, `Copy`, and `PartialEq`.
3. THE Heritable_Traits struct SHALL occupy exactly 16 bytes (four contiguous `f32` values) with no padding.
4. THE Actor struct SHALL contain one Heritable_Traits field.

### Requirement 2: Seed Actor Initialization

**User Story:** As a simulation architect, I want seed actors created at world init to receive their heritable traits from the global ActorConfig defaults, so that the initial population starts with a uniform baseline genome.

#### Acceptance Criteria

1. WHEN a Seed_Actor is created during world initialization, THE world_init system SHALL populate the actor's Heritable_Traits by copying `consumption_rate`, `base_energy_decay`, `levy_exponent`, and `reproduction_threshold` from the ActorConfig.
2. WHEN a Seed_Actor is created, THE Heritable_Traits values SHALL satisfy all Trait_Clamp_Range constraints.

### Requirement 3: Trait Inheritance with Mutation During Fission

**User Story:** As a simulation architect, I want offspring to inherit their parent's traits with small random mutations, so that heritable variation accumulates across generations.

#### Acceptance Criteria

1. WHEN an actor undergoes Fission, THE reproduction system SHALL copy the parent's Heritable_Traits to the offspring.
2. WHEN an actor undergoes Fission, THE reproduction system SHALL apply independent gaussian Trait_Mutation to each of the offspring's four trait fields.
3. WHEN Trait_Mutation is applied, THE reproduction system SHALL use a Mutation_Stddev that is configurable via the ActorConfig.
4. WHEN Trait_Mutation is applied, THE reproduction system SHALL clamp each mutated trait value to its Trait_Clamp_Range.
5. WHEN Trait_Mutation is applied, THE reproduction system SHALL derive the mutation RNG seed deterministically from the simulation master seed, the parent's slot index, and the current tick number.

### Requirement 4: Trait Clamp Ranges

**User Story:** As a simulation architect, I want heritable trait values bounded to sane ranges, so that mutation drift cannot produce physically meaningless parameter values.

#### Acceptance Criteria

1. THE ActorConfig SHALL define minimum and maximum clamp bounds for each of the four heritable traits.
2. WHEN any Heritable_Traits value is set or mutated, THE system SHALL clamp the value to its configured Trait_Clamp_Range.
3. THE default Trait_Clamp_Range for `consumption_rate` SHALL be `[0.1, 10.0]`.
4. THE default Trait_Clamp_Range for `base_energy_decay` SHALL be `[0.001, 1.0]`.
5. THE default Trait_Clamp_Range for `levy_exponent` SHALL be `[1.01, 3.0]`.
6. THE default Trait_Clamp_Range for `reproduction_threshold` SHALL be `[1.0, 100.0]`.

### Requirement 5: Systems Read Per-Actor Traits

**User Story:** As a simulation architect, I want the metabolism, sensing, and reproduction systems to read from each actor's heritable traits instead of the global config for the four heritable fields, so that individual variation actually affects behavior.

#### Acceptance Criteria

1. WHEN the metabolism system processes an active actor, THE metabolism system SHALL read `consumption_rate` and `base_energy_decay` from the actor's Heritable_Traits instead of the ActorConfig.
2. WHEN the sensing system initiates a Lévy flight tumble, THE sensing system SHALL read `levy_exponent` from the actor's Heritable_Traits instead of the ActorConfig.
3. WHEN the reproduction system evaluates fission eligibility, THE reproduction system SHALL compare the actor's energy against the actor's Heritable_Traits `reproduction_threshold` instead of the ActorConfig value.
4. WHEN the metabolism system processes an inert actor, THE metabolism system SHALL read `base_energy_decay` from the actor's Heritable_Traits.
5. WHILE the sensing system computes the break-even concentration, THE sensing system SHALL use the actor's Heritable_Traits `base_energy_decay` instead of the ActorConfig value.

### Requirement 6: Mutation Configuration

**User Story:** As a simulation operator, I want to configure mutation parameters via the TOML config file, so that I can tune evolutionary pressure without recompiling.

#### Acceptance Criteria

1. THE ActorConfig SHALL include a `mutation_stddev` field (f32) controlling the standard deviation of gaussian trait mutation.
2. THE ActorConfig SHALL provide a default `mutation_stddev` value of `0.05`.
3. WHEN `mutation_stddev` is set to `0.0`, THE reproduction system SHALL produce offspring with trait values identical to the parent (no mutation).
4. THE config parser SHALL reject negative `mutation_stddev` values.

### Requirement 7: Deterministic Mutation RNG

**User Story:** As a simulation architect, I want mutation to be fully deterministic and reproducible, so that replaying a simulation from the same seed produces identical evolutionary trajectories.

#### Acceptance Criteria

1. THE reproduction system SHALL derive a per-fission RNG seed from the simulation master seed, the parent actor's registry slot index, and the current tick number.
2. WHEN the same simulation is replayed with the same master seed and config, THE reproduction system SHALL produce identical offspring trait values at every tick.
3. THE mutation RNG SHALL use `rand::SeedableRng` with a seeded `SmallRng` or equivalent lightweight PRNG.

### Requirement 8: Configuration Documentation

**User Story:** As a simulation operator, I want all new configuration fields documented, so that I can understand and tune the heritable traits system.

#### Acceptance Criteria

1. WHEN new fields are added to ActorConfig, THE `example_config.toml` SHALL include the new fields with descriptive comments.
2. WHEN new fields are added to ActorConfig, THE `README.md` SHALL document the new configuration parameters.
3. WHEN new fields are added to ActorConfig, THE Bevy config info panel (`format_config_info()`) SHALL display the new fields.
