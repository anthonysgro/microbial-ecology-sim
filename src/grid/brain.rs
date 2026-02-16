use crate::grid::actor::HeritableTraits;

/// Compile-time maximum memory capacity across all actors.
/// The inline array is always this size; effective capacity is governed
/// by the actor's heritable `memory_capacity` trait.
pub const MAX_MEMORY_CAPACITY: usize = 16;

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

// Layout: 8 (tick) + 4 (cell_index) + 4 (genome_hash) + 1 (outcome) + 7 (alignment padding) = 24 bytes
// Alignment is 8 due to the u64 tick field.
const _: () = assert!(std::mem::size_of::<MemoryEntry>() == 24);

/// Cognitive component for an actor. Stored in a parallel `Vec<Brain>`
/// indexed by actor slot. Contains a fixed-size circular buffer of
/// memory entries.
///
/// Plain data struct — no methods beyond trivial construction.
#[derive(Debug, Clone, PartialEq)]
pub struct Brain {
    /// Circular buffer of memory entries. Only entries `[0..len)` are valid
    /// when `len < capacity`; when `len == capacity`, `head` points to the
    /// oldest entry (next to be overwritten).
    pub entries: [MemoryEntry; MAX_MEMORY_CAPACITY],
    /// Index of the next write position (wraps around).
    pub head: u8,
    /// Number of valid entries. Capped at the actor's heritable `memory_capacity`.
    pub len: u8,
}

// Size: 24 * 16 + 1 + 1 + 6 (struct padding) = 392 bytes per Brain
const _: () = assert!(std::mem::size_of::<Brain>() == 392);

/// A zeroed memory entry used for initialization.
const ZEROED_ENTRY: MemoryEntry = MemoryEntry {
    tick: 0,
    cell_index: 0,
    genome_hash: 0,
    outcome: MemoryOutcome::Food,
};

/// Create an empty Brain with zeroed entries, head=0, len=0.
pub fn brain_empty() -> Brain {
    Brain {
        entries: [ZEROED_ENTRY; MAX_MEMORY_CAPACITY],
        head: 0,
        len: 0,
    }
}

/// Write a memory entry to a Brain, respecting the actor's heritable `memory_capacity`.
///
/// No-op when `capacity == 0`. Otherwise inserts into the circular buffer:
/// - If `len < capacity`, appends at `head` and increments both `head` and `len`.
/// - If `len == capacity`, overwrites the oldest entry at `head` and advances `head`.
pub fn brain_write(brain: &mut Brain, entry: MemoryEntry, capacity: u8) {
    if capacity == 0 {
        return;
    }
    let cap = capacity as usize;
    brain.entries[brain.head as usize] = entry;
    brain.head = ((brain.head as usize + 1) % cap) as u8;
    if (brain.len as usize) < cap {
        brain.len += 1;
    }
}

/// Compute a deterministic u32 genome hash from heritable traits.
///
/// Uses wrapping arithmetic — intentionally lossy. Identical traits produce
/// identical hashes. Collisions are biologically plausible as "misremembering."
pub fn genome_hash(traits: &HeritableTraits) -> u32 {
    // FNV-1a-inspired wrapping hash over the trait values.
    let mut h: u32 = 2_166_136_261; // FNV offset basis
    let prime: u32 = 16_777_619; // FNV prime

    let mix = |h: &mut u32, bytes: &[u8]| {
        for &b in bytes {
            *h ^= b as u32;
            *h = h.wrapping_mul(prime);
        }
    };

    mix(&mut h, &traits.consumption_rate.to_le_bytes());
    mix(&mut h, &traits.base_energy_decay.to_le_bytes());
    mix(&mut h, &traits.levy_exponent.to_le_bytes());
    mix(&mut h, &traits.reproduction_threshold.to_le_bytes());
    mix(&mut h, &traits.max_tumble_steps.to_le_bytes());
    mix(&mut h, &traits.reproduction_cost.to_le_bytes());
    mix(&mut h, &traits.offspring_energy.to_le_bytes());
    mix(&mut h, &traits.mutation_rate.to_le_bytes());
    mix(&mut h, &traits.kin_tolerance.to_le_bytes());
    mix(&mut h, &traits.kin_group_defense.to_le_bytes());
    mix(&mut h, &traits.optimal_temp.to_le_bytes());
    mix(&mut h, &traits.reproduction_cooldown.to_le_bytes());
    mix(&mut h, &traits.memory_capacity.to_le_bytes());

    h
}
