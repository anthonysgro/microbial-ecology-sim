// WARM PATH: Runs every render frame over all grid cells.
// Zero heap allocations — operates on pre-allocated slices only.

/// Normalize a raw `f32` field buffer into a pre-allocated output slice.
///
/// Divides each value by the buffer maximum, producing output in `[0.0, 1.0]`.
/// Returns the max value found in `raw`.
///
/// # Panics
///
/// Panics (debug-only) if `out.len() < raw.len()`.
///
/// # Normalization rules
///
/// - When max is near zero (`< 1e-9`), all outputs are `0.0` (Req 3.2).
/// - When all values are identical and non-zero, all outputs are `1.0` (Req 3.3).
/// - Otherwise, outputs are in `[0.0, 1.0]` with the max element mapping to `1.0` (Req 3.1).
pub fn normalize_field(raw: &[f32], out: &mut [f32]) -> f32 {
    debug_assert!(
        out.len() >= raw.len(),
        "output slice too small: {} < {}",
        out.len(),
        raw.len()
    );

    if raw.is_empty() {
        return 0.0;
    }

    let max_val = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    if max_val.abs() < 1e-9 {
        // Near-zero max: entire field is effectively zero.
        for o in out.iter_mut().take(raw.len()) {
            *o = 0.0;
        }
        return max_val;
    }

    // max_val is non-negligible; divide through.
    let inv_max = 1.0 / max_val;
    for (o, &v) in out.iter_mut().zip(raw.iter()) {
        *o = v * inv_max;
    }

    max_val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_buffer_normalizes_to_zero() {
        let raw = [0.0_f32; 5];
        let mut out = [0.0_f32; 5];
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < 1e-9);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn identical_nonzero_normalizes_to_one() {
        let raw = [3.5_f32; 4];
        let mut out = [0.0_f32; 4];
        let max = normalize_field(&raw, &mut out);
        assert!((max - 3.5).abs() < f32::EPSILON);
        assert!(out.iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn basic_normalization() {
        let raw = [0.0_f32, 5.0, 10.0];
        let mut out = [0.0_f32; 3];
        let max = normalize_field(&raw, &mut out);
        assert!((max - 10.0).abs() < f32::EPSILON);
        assert!((out[0]).abs() < f32::EPSILON);
        assert!((out[1] - 0.5).abs() < f32::EPSILON);
        assert!((out[2] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_buffer() {
        let raw: [f32; 0] = [];
        let mut out: [f32; 0] = [];
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < f32::EPSILON);
    }

    #[test]
    fn output_slice_larger_than_input() {
        let raw = [2.0_f32, 4.0];
        let mut out = [99.0_f32; 10];
        normalize_field(&raw, &mut out);
        // First two elements normalized, rest untouched.
        assert!((out[0] - 0.5).abs() < f32::EPSILON);
        assert!((out[1] - 1.0).abs() < f32::EPSILON);
        assert!((out[2] - 99.0).abs() < f32::EPSILON);
    }
}
