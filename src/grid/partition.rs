/// A rectangular region of the grid assigned to one thread.
///
/// Row-band partitions span the full grid width and a contiguous subset
/// of rows, preserving row-major access patterns within each partition.
#[derive(Debug, Clone, PartialEq)]
pub struct Partition {
    pub start_row: u32,
    pub end_row: u32, // exclusive
    pub start_col: u32,
    pub end_col: u32, // exclusive
}

impl Partition {
    /// Iterate flat cell indices in row-major order for this partition.
    pub fn cell_indices(&self, grid_width: u32) -> impl Iterator<Item = usize> + '_ {
        (self.start_row..self.end_row).flat_map(move |y| {
            (self.start_col..self.end_col).map(move |x| (y * grid_width + x) as usize)
        })
    }

    /// Number of cells in this partition.
    pub fn cell_count(&self) -> usize {
        let rows = (self.end_row - self.start_row) as usize;
        let cols = (self.end_col - self.start_col) as usize;
        rows * cols
    }
}

/// Divide the grid into non-overlapping row-band partitions.
///
/// Each partition spans the full width. Rows are distributed as evenly as
/// possible: the first `remainder` partitions get one extra row.
///
/// # Panics
///
/// Debug-asserts that `width`, `height`, and `num_threads` are all > 0.
/// These preconditions are enforced by `Grid::new` via `GridError::InvalidDimensions`.
pub fn compute_partitions(width: u32, height: u32, num_threads: usize) -> Vec<Partition> {
    debug_assert!(width > 0 && height > 0 && num_threads > 0);

    // Clamp thread count to height — can't have more partitions than rows.
    let n = (num_threads as u32).min(height);

    let rows_per = height / n;
    let remainder = height % n;

    let mut partitions = Vec::with_capacity(n as usize);
    let mut current_row: u32 = 0;

    for i in 0..n {
        // First `remainder` partitions each absorb one extra row.
        let extra = if i < remainder { 1 } else { 0 };
        let end_row = current_row + rows_per + extra;

        partitions.push(Partition {
            start_row: current_row,
            end_row,
            start_col: 0,
            end_col: width,
        });

        current_row = end_row;
    }

    partitions
}
