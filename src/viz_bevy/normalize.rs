// WARM PATH: Runs every render frame over all grid cells.
// Zero heap allocations — operates on pre-allocated slices only.

/// Normalize a raw `f32` field buffer into a pre-allocated output slice.
///
/// When `fixed_max` is positive, divides each value by `fixed_max` and clamps
/// to `[0.0, 1.0]`. This keeps the color scale stable across frames.
///
/// When `fixed_max` is zero or negative, falls back to dynamic normalization
/// (divide by the buffer maximum), matching the original behavior.
///
/// Returns the actual max value found in `raw`.
///
/// # Panics
///
/// Panics (debug-only) if `out.len() < raw.len()`.
///
/// # Normalization rules
///
/// - When max is near zero (`< 1e-9`), all outputs are `0.0` (Req 3.2).
/// - When all values are identical and non-zero, all outputs are `1.0` (Req 3.3).
/// - Otherwise, outputs are in `[0.0, 1.0]` (Req 3.1).
pub fn normalize_field(raw: &[f32], out: &mut [f32], fixed_max: f32) -> f32 {
    debug_assert!(
        out.len() >= raw.len(),
        "output slice too small: {} < {}",
        out.len(),
        raw.len()
    );

    if raw.is_empty() {
        return 0.0;
    }

    let actual_max = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Choose divisor: fixed_max if positive, otherwise dynamic max.
    let divisor = if fixed_max > 0.0 { fixed_max } else { actual_max };

    if divisor.abs() < 1e-9 {
        for o in out.iter_mut().take(raw.len()) {
            *o = 0.0;
        }
        return actual_max;
    }

    let inv = 1.0 / divisor;
    for (o, &v) in out.iter_mut().zip(raw.iter()) {
        *o = (v * inv).clamp(0.0, 1.0);
    }

    actual_max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_buffer_normalizes_to_zero() {
        let raw = [0.0_f32; 5];
        let mut out = [0.0_f32; 5];
        let max = normalize_field(&raw, &mut out, 0.0);
        assert!(max.abs() < 1e-9);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn identical_nonzero_normalizes_to_one() {
        let raw = [3.5_f32; 4];
        let mut out = [0.0_f32; 4];
        let max = normalize_field(&raw, &mut out, 0.0);
        assert!((max - 3.5).abs() < f32::EPSILON);
        assert!(out.iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn basic_normalization() {
        let raw = [0.0_f32, 5.0, 10.0];
        let mut out = [0.0_f32; 3];
        let max = normalize_field(&raw, &mut out, 0.0);
        assert!((max - 10.0).abs() < f32::EPSILON);
        assert!((out[0]).abs() < f32::EPSILON);
        assert!((out[1] - 0.5).abs() < f32::EPSILON);
        assert!((out[2] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_buffer() {
        let raw: [f32; 0] = [];
        let mut out: [f32; 0] = [];
        let max = normalize_field(&raw, &mut out, 0.0);
        assert!(max.abs() < f32::EPSILON);
    }

    #[test]
    fn output_slice_larger_than_input() {
        let raw = [2.0_f32, 4.0];
        let mut out = [99.0_f32; 10];
        normalize_field(&raw, &mut out, 0.0);
        assert!((out[0] - 0.5).abs() < f32::EPSILON);
        assert!((out[1] - 1.0).abs() < f32::EPSILON);
        assert!((out[2] - 99.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fixed_max_clamps_to_one() {
        let raw = [0.0_f32, 5.0, 20.0];
        let mut out = [0.0_f32; 3];
        let max = normalize_field(&raw, &mut out, 10.0);
        assert!((max - 20.0).abs() < f32::EPSILON);
        assert!((out[0]).abs() < f32::EPSILON);
        assert!((out[1] - 0.5).abs() < f32::EPSILON);
        // 20.0 / 10.0 = 2.0, clamped to 1.0
        assert!((out[2] - 1.0).abs() < f32::EPSILON);
    }
}
