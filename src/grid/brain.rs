use crate::grid::actor::HeritableTraits;

/// Compile-time maximum memory capacity across all actors.
/// The inline array is always this size; effective capacity is governed
/// by the actor's heritable `memory_capacity` trait.
pub const MAX_MEMORY_CAPACITY: usize = 16;

/// Outcome type for a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Compute memory-biased movement target from an actor's Brain.
///
/// WARM PATH: bounded scan of at most MAX_MEMORY_CAPACITY entries.
/// Zero heap allocations. Deterministic for identical inputs.
///
/// Returns `Some(cell_index)` of the best-scoring Von Neumann neighbor,
/// or `None` if no neighbor has a positive score.
///
/// Direction indices: 0=N, 1=S, 2=W, 3=E.
/// Tie-breaking: N, S, W, E (first wins via strict `>`).
#[allow(clippy::too_many_arguments)]
pub fn compute_memory_bias(
    brain: &Brain,
    actor_x: usize,
    actor_y: usize,
    grid_width: usize,
    grid_height: usize,
    current_tick: u64,
    site_fidelity_strength: f32,
    avoidance_sensitivity: f32,
) -> Option<usize> {
    if brain.len == 0 {
        return None;
    }

    // Direction accumulator: [N, S, W, E]
    let mut scores: [f32; 4] = [0.0; 4];

    let len = brain.len as usize;
    for i in 0..len {
        let entry = &brain.entries[i];

        let mem_x = (entry.cell_index as usize) % grid_width;
        let mem_y = (entry.cell_index as usize) / grid_width;

        // Skip entries at actor's current cell — no directional bias possible.
        if mem_x == actor_x && mem_y == actor_y {
            continue;
        }

        // Temporal decay: recent memories weigh more.
        let age = current_tick.saturating_sub(entry.tick) as f32;
        let decay = 1.0 / (1.0 + age);

        // Weight sign depends on outcome type.
        let weight = match entry.outcome {
            MemoryOutcome::Food | MemoryOutcome::PredationSuccess => {
                site_fidelity_strength * decay
            }
            MemoryOutcome::PredationThreat => {
                -avoidance_sensitivity * decay
            }
        };

        // Accumulate into direction(s) that reduce Manhattan distance.
        if mem_y < actor_y {
            scores[0] += weight; // N
        }
        if mem_y > actor_y {
            scores[1] += weight; // S
        }
        if mem_x < actor_x {
            scores[2] += weight; // W
        }
        if mem_x > actor_x {
            scores[3] += weight; // E
        }
    }

    // Find best direction with strict > for tie-breaking (N, S, W, E order).
    let mut best_dir: usize = 0;
    let mut best_score: f32 = scores[0];
    #[allow(clippy::needless_range_loop)]
    for dir in 1..4 {
        if scores[dir] > best_score {
            best_score = scores[dir];
            best_dir = dir;
        }
    }

    if best_score <= 0.0 {
        return None;
    }

    // Convert direction to neighbor cell index, checking grid bounds.
    let (nx, ny) = match best_dir {
        0 => {
            // N: y - 1
            if actor_y == 0 { return None; }
            (actor_x, actor_y - 1)
        }
        1 => {
            // S: y + 1
            if actor_y + 1 >= grid_height { return None; }
            (actor_x, actor_y + 1)
        }
        2 => {
            // W: x - 1
            if actor_x == 0 { return None; }
            (actor_x - 1, actor_y)
        }
        3 => {
            // E: x + 1
            if actor_x + 1 >= grid_width { return None; }
            (actor_x + 1, actor_y)
        }
        // SAFETY: best_dir is always in 0..4 from the loop above.
        _ => unreachable!(),
    };

    Some(ny * grid_width + nx)
}

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
    mix(&mut h, &traits.site_fidelity_strength.to_le_bytes());
    mix(&mut h, &traits.avoidance_sensitivity.to_le_bytes());

    h
}
