/// Configuration parameters for Actor metabolism, sensing, and spawning.
///
/// Plain data struct — immutable after construction. All rates are per-tick.
/// Provided at grid construction time via `Option<ActorConfig>`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorConfig {
    /// Chemical units consumed per tick from the Actor's current cell (species 0).
    pub consumption_rate: f32,
    /// Energy gained per unit of chemical consumed.
    pub energy_conversion_factor: f32,
    /// Energy subtracted from every Actor each tick (basal metabolic cost).
    pub base_energy_decay: f32,
    /// Energy assigned to newly spawned Actors.
    pub initial_energy: f32,
    /// Pre-allocated slot capacity for the ActorRegistry.
    pub initial_actor_capacity: usize,
}
