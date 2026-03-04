// COLD PATH: Serialization/deserialization for world state snapshots and patterns.
// Allocation permitted. Used only on explicit user save/load actions.

use crate::grid::actor::HeritableTraits;
use crate::grid::actor_config::ActorConfig;
use crate::grid::brain::MemoryEntry;
use crate::grid::config::GridConfig;
use crate::grid::source::Source;

/// Magic bytes identifying a snapshot file.
const SNAPSHOT_MAGIC: &[u8; 4] = b"MSIM";
/// Magic bytes identifying a pattern file.
const PATTERN_MAGIC: &[u8; 4] = b"MPAT";
/// Current snapshot format version.
const SNAPSHOT_VERSION: u32 = 1;
/// Current pattern format version.
const PATTERN_VERSION: u32 = 1;

/// Errors arising from snapshot/pattern serialization and deserialization.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("deserialization error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },

    #[error("grid dimension mismatch: snapshot is {snap_w}x{snap_h}, current grid is {grid_w}x{grid_h}")]
    DimensionMismatch {
        snap_w: u32,
        snap_h: u32,
        grid_w: u32,
        grid_h: u32,
    },

    #[error("chemical species mismatch: pattern has {pattern_count}, grid has {grid_count}")]
    ChemicalMismatch {
        pattern_count: usize,
        grid_count: usize,
    },

    #[error("actor cell index {cell_index} out of bounds (grid has {cell_count} cells)")]
    ActorOutOfBounds {
        cell_index: usize,
        cell_count: usize,
    },
}

/// Compact serializable representation of an Actor for snapshot I/O.
///
/// Uses `Vec<MemoryEntry>` for brain entries instead of the fixed-size
/// array to avoid serializing unused slots.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActorSnapshot {
    pub cell_index: usize,
    pub energy: f32,
    pub inert: bool,
    pub tumble_direction: u8,
    pub tumble_remaining: u16,
    pub traits: HeritableTraits,
    pub cooldown_remaining: u16,
    pub brain: BrainSnapshot,
}

/// Compact serializable representation of a Brain.
///
/// Only the valid entries (`len` of them) are stored, avoiding
/// serialization of the full 16-entry fixed array.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrainSnapshot {
    pub entries: Vec<MemoryEntry>,
    pub head: u8,
    pub len: u8,
}

/// Deserialized snapshot result container.
///
/// Holds all data needed to restore a complete world state.
/// The caller is responsible for rebuilding `Grid` internals
/// (occupancy, partitions, etc.) from this data.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotData {
    pub grid_config: GridConfig,
    pub actor_config: Option<ActorConfig>,
    pub tick: u64,
    pub heat: Vec<f32>,
    pub chemicals: Vec<Vec<f32>>,
    pub sources: Vec<Source>,
    pub actors: Vec<ActorSnapshot>,
}

/// A rectangular sub-region of the world, stored with relative coordinates.
///
/// Used for copy/paste of patterns (oscillators, signal propagators, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub width: u32,
    pub height: u32,
    pub num_chemicals: usize,
    pub heat: Vec<f32>,
    pub chemicals: Vec<Vec<f32>>,
    pub actors: Vec<PatternActor>,
}

/// An actor within a Pattern, positioned relative to the pattern origin.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PatternActor {
    /// Relative x within the pattern (0-based).
    pub rel_x: u32,
    /// Relative y within the pattern (0-based).
    pub rel_y: u32,
    pub energy: f32,
    pub traits: HeritableTraits,
}


// ── Bincode configuration ──────────────────────────────────────────

/// Standard bincode config used for all encode/decode operations.
/// Little-endian, variable-length integers, fixed-length arrays.
const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

// ── Snapshot serialization ─────────────────────────────────────────

/// Serialize a complete world state to bytes.
///
/// Binary format:
/// - Magic `b"MSIM"` (4 bytes)
/// - Version `1u32` (4 bytes, little-endian)
/// - bincode-encoded `GridConfig`
/// - `has_actor_config: bool` (1 byte)
/// - bincode-encoded `ActorConfig` (if present)
/// - `tick: u64` (8 bytes, little-endian)
/// - `num_chemicals: u32` (4 bytes, little-endian)
/// - Heat read buffer: `[f32; W*H]`
/// - Chemical buffers: `[[f32; W*H]; num_chemicals]`
/// - `source_count: u32` (4 bytes, little-endian)
/// - Sources: bincode-encoded `[Source; source_count]`
/// - `actor_count: u32` (4 bytes, little-endian)
/// - Actors: bincode-encoded `[ActorSnapshot; actor_count]`
pub fn serialize_snapshot(
    grid_config: &GridConfig,
    actor_config: Option<&ActorConfig>,
    tick: u64,
    heat_read: &[f32],
    chemical_reads: &[&[f32]],
    sources: &[Source],
    actors: &[ActorSnapshot],
) -> Result<Vec<u8>, SnapshotError> {
    let mut buf = Vec::new();

    // Magic + version
    buf.extend_from_slice(SNAPSHOT_MAGIC);
    buf.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());

    // GridConfig
    let encoded = bincode::serde::encode_to_vec(grid_config, BINCODE_CONFIG)?;
    buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    buf.extend_from_slice(&encoded);

    // ActorConfig presence + data
    let has_actor_config = actor_config.is_some();
    buf.push(u8::from(has_actor_config));
    if let Some(ac) = actor_config {
        let encoded = bincode::serde::encode_to_vec(ac, BINCODE_CONFIG)?;
        buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        buf.extend_from_slice(&encoded);
    }

    // Tick
    buf.extend_from_slice(&tick.to_le_bytes());

    // num_chemicals
    let num_chemicals = chemical_reads.len() as u32;
    buf.extend_from_slice(&num_chemicals.to_le_bytes());

    // Heat buffer (raw f32 bytes)
    for &val in heat_read {
        buf.extend_from_slice(&val.to_le_bytes());
    }

    // Chemical buffers (raw f32 bytes)
    for chem_buf in chemical_reads {
        for &val in *chem_buf {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    // Sources
    let source_count = sources.len() as u32;
    buf.extend_from_slice(&source_count.to_le_bytes());
    for source in sources {
        let encoded = bincode::serde::encode_to_vec(source, BINCODE_CONFIG)?;
        buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        buf.extend_from_slice(&encoded);
    }

    // Actors
    let actor_count = actors.len() as u32;
    buf.extend_from_slice(&actor_count.to_le_bytes());
    for actor in actors {
        let encoded = bincode::serde::encode_to_vec(actor, BINCODE_CONFIG)?;
        buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        buf.extend_from_slice(&encoded);
    }

    Ok(buf)
}

/// Deserialize a complete world state from bytes.
///
/// Validates magic bytes, version, and actor cell indices against grid dimensions.
pub fn deserialize_snapshot(bytes: &[u8]) -> Result<SnapshotData, SnapshotError> {
    let mut cursor = 0;

    // Magic bytes
    if bytes.len() < 8 {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "snapshot too short for header",
        )));
    }
    if &bytes[0..4] != SNAPSHOT_MAGIC {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "invalid snapshot magic bytes",
        )));
    }
    cursor += 4;

    // Version
    let version = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?);
    if version != SNAPSHOT_VERSION {
        return Err(SnapshotError::VersionMismatch {
            expected: SNAPSHOT_VERSION,
            got: version,
        });
    }

    // GridConfig
    let grid_config: GridConfig = decode_length_prefixed(bytes, &mut cursor)?;

    // ActorConfig
    let has_actor_config = read_u8(bytes, &mut cursor)?;
    let actor_config: Option<ActorConfig> = if has_actor_config != 0 {
        Some(decode_length_prefixed(bytes, &mut cursor)?)
    } else {
        None
    };

    // Tick
    let tick = u64::from_le_bytes(read_bytes::<8>(bytes, &mut cursor)?);

    // num_chemicals
    let num_chemicals = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?) as usize;

    let cell_count = (grid_config.width as usize) * (grid_config.height as usize);

    // Heat buffer
    let heat = read_f32_vec(bytes, &mut cursor, cell_count)?;

    // Chemical buffers
    let mut chemicals = Vec::with_capacity(num_chemicals);
    for _ in 0..num_chemicals {
        chemicals.push(read_f32_vec(bytes, &mut cursor, cell_count)?);
    }

    // Sources
    let source_count = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?) as usize;
    let mut sources = Vec::with_capacity(source_count);
    for _ in 0..source_count {
        sources.push(decode_length_prefixed::<Source>(bytes, &mut cursor)?);
    }

    // Actors
    let actor_count = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?) as usize;
    let mut actors = Vec::with_capacity(actor_count);
    for _ in 0..actor_count {
        let actor: ActorSnapshot = decode_length_prefixed(bytes, &mut cursor)?;
        if actor.cell_index >= cell_count {
            return Err(SnapshotError::ActorOutOfBounds {
                cell_index: actor.cell_index,
                cell_count,
            });
        }
        actors.push(actor);
    }

    Ok(SnapshotData {
        grid_config,
        actor_config,
        tick,
        heat,
        chemicals,
        sources,
        actors,
    })
}

// ── Internal helpers ───────────────────────────────────────────────

/// Read exactly N bytes from the buffer at the cursor position.
fn read_bytes<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], SnapshotError> {
    if *cursor + N > bytes.len() {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "unexpected end of data",
        )));
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(&bytes[*cursor..*cursor + N]);
    *cursor += N;
    Ok(arr)
}

/// Read a single u8 from the buffer.
fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, SnapshotError> {
    let arr = read_bytes::<1>(bytes, cursor)?;
    Ok(arr[0])
}

/// Read `count` f32 values from raw little-endian bytes.
fn read_f32_vec(
    bytes: &[u8],
    cursor: &mut usize,
    count: usize,
) -> Result<Vec<f32>, SnapshotError> {
    let byte_count = count * 4;
    if *cursor + byte_count > bytes.len() {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "unexpected end of data reading f32 buffer",
        )));
    }
    let mut vec = Vec::with_capacity(count);
    for i in 0..count {
        let offset = *cursor + i * 4;
        let val = f32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        vec.push(val);
    }
    *cursor += byte_count;
    Ok(vec)
}

/// Decode a length-prefixed bincode-encoded value.
///
/// Format: `u32 length` (little-endian) followed by `length` bytes of bincode data.
fn decode_length_prefixed<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<T, SnapshotError> {
    let len = u32::from_le_bytes(read_bytes::<4>(bytes, cursor)?) as usize;
    if *cursor + len > bytes.len() {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "unexpected end of data in length-prefixed block",
        )));
    }
    let (value, _) =
        bincode::serde::decode_from_slice(&bytes[*cursor..*cursor + len], BINCODE_CONFIG)?;
    *cursor += len;
    Ok(value)
}


// ── Pattern serialization ──────────────────────────────────────────

/// Serialize a rectangular sub-region pattern to bytes.
///
/// Binary format:
/// - Magic `b"MPAT"` (4 bytes)
/// - Version `1u32` (4 bytes, little-endian)
/// - `width: u32` (4 bytes, little-endian)
/// - `height: u32` (4 bytes, little-endian)
/// - `num_chemicals: u32` (4 bytes, little-endian)
/// - Heat values: `[f32; width * height]`
/// - Chemical values: `[[f32; width * height]; num_chemicals]`
/// - `actor_count: u32` (4 bytes, little-endian)
/// - Actors: bincode-encoded `[PatternActor; actor_count]`
pub fn serialize_pattern(pattern: &Pattern) -> Result<Vec<u8>, SnapshotError> {
    let mut buf = Vec::new();

    // Magic + version
    buf.extend_from_slice(PATTERN_MAGIC);
    buf.extend_from_slice(&PATTERN_VERSION.to_le_bytes());

    // Dimensions
    buf.extend_from_slice(&pattern.width.to_le_bytes());
    buf.extend_from_slice(&pattern.height.to_le_bytes());
    buf.extend_from_slice(&(pattern.num_chemicals as u32).to_le_bytes());

    // Heat values (raw f32 bytes)
    for &val in &pattern.heat {
        buf.extend_from_slice(&val.to_le_bytes());
    }

    // Chemical values (raw f32 bytes)
    for chem_buf in &pattern.chemicals {
        for &val in chem_buf {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    // Actors
    let actor_count = pattern.actors.len() as u32;
    buf.extend_from_slice(&actor_count.to_le_bytes());
    for actor in &pattern.actors {
        let encoded = bincode::serde::encode_to_vec(actor, BINCODE_CONFIG)?;
        buf.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        buf.extend_from_slice(&encoded);
    }

    Ok(buf)
}

/// Deserialize a pattern from bytes.
///
/// Validates magic bytes and version. Does not validate against a target
/// grid — the caller should check `num_chemicals` compatibility.
pub fn deserialize_pattern(bytes: &[u8]) -> Result<Pattern, SnapshotError> {
    let mut cursor = 0;

    // Magic bytes
    if bytes.len() < 8 {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "pattern too short for header",
        )));
    }
    if &bytes[0..4] != PATTERN_MAGIC {
        return Err(SnapshotError::Decode(bincode::error::DecodeError::Other(
            "invalid pattern magic bytes",
        )));
    }
    cursor += 4;

    // Version
    let version = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?);
    if version != PATTERN_VERSION {
        return Err(SnapshotError::VersionMismatch {
            expected: PATTERN_VERSION,
            got: version,
        });
    }

    // Dimensions
    let width = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?);
    let height = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?);
    let num_chemicals = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?) as usize;

    let cell_count = (width as usize) * (height as usize);

    // Heat values
    let heat = read_f32_vec(bytes, &mut cursor, cell_count)?;

    // Chemical values
    let mut chemicals = Vec::with_capacity(num_chemicals);
    for _ in 0..num_chemicals {
        chemicals.push(read_f32_vec(bytes, &mut cursor, cell_count)?);
    }

    // Actors
    let actor_count = u32::from_le_bytes(read_bytes::<4>(bytes, &mut cursor)?) as usize;
    let mut actors = Vec::with_capacity(actor_count);
    for _ in 0..actor_count {
        actors.push(decode_length_prefixed::<PatternActor>(bytes, &mut cursor)?);
    }

    Ok(Pattern {
        width,
        height,
        num_chemicals,
        heat,
        chemicals,
        actors,
    })
}
