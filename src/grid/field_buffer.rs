/// Double-buffered contiguous array for a single physical field.
///
/// One buffer is the "read" buffer (current state), the other is the
/// "write" buffer (next state). `swap()` flips them via XOR on an index
/// — no data is copied.
pub struct FieldBuffer<T: Copy> {
    buffers: [Vec<T>; 2],
    current: usize,
}

impl<T: Copy> FieldBuffer<T> {
    /// Allocate a new double buffer with `len` elements, each set to `default`.
    pub fn new(len: usize, default: T) -> Self {
        Self {
            buffers: [vec![default; len], vec![default; len]],
            current: 0,
        }
    }

    /// Read-only slice of the current (read) buffer.
    pub fn read(&self) -> &[T] {
        &self.buffers[self.current]
    }

    /// Mutable slice of the write buffer.
    pub fn write(&mut self) -> &mut [T] {
        &mut self.buffers[self.current ^ 1]
    }

    /// Simultaneous read and write access to both buffers.
    ///
    /// Returns `(read_slice, write_slice)`. The two slices reference
    /// distinct `Vec` allocations (indices 0 and 1), so no aliasing.
    pub fn read_write(&mut self) -> (&[T], &mut [T]) {
        let read_idx = self.current;
        // split_at_mut(1) gives us two disjoint sub-slices of the array.
        let (first, second) = self.buffers.split_at_mut(1);
        if read_idx == 0 {
            (&first[0], &mut second[0])
        } else {
            (&second[0], &mut first[0])
        }
    }

    /// Copy the read buffer contents into the write buffer.
    ///
    /// Used by the emission phase to ensure emission adds to the current
    /// state rather than to stale or zeroed data. Single `memcpy` — for a
    /// 100×100 grid this is 40 KB per field, well within L1 cache.
    pub fn copy_read_to_write(&mut self) {
        let read_idx = self.current;
        // split_at_mut gives disjoint borrows, avoiding the simultaneous
        // mutable + immutable borrow on self.buffers.
        let (first, second) = self.buffers.split_at_mut(1);
        if read_idx == 0 {
            second[0].copy_from_slice(&first[0]);
        } else {
            first[0].copy_from_slice(&second[0]);
        }
    }

    /// Write a value to both read and write buffers at the given index.
    ///
    /// COLD: Used by the editor to paint values that are immediately visible
    /// (read buffer) and persist after the next swap (write buffer).
    pub fn write_both(&mut self, index: usize, value: T) {
        self.buffers[0][index] = value;
        self.buffers[1][index] = value;
    }

    /// Swap read and write buffers. No data copy — just flips the index.
    pub fn swap(&mut self) {
        self.current ^= 1;
    }
}
