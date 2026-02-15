// COLD PATH: Terminal renderer implementation.

/// Normalize a raw field buffer into `out`, dividing each element by the buffer maximum.
///
/// Returns the max value found in `raw`.
///
/// - When max is near zero (< 1e-9), all outputs are 0.0 (Req 1.3).
/// - When all values are identical and non-zero, all outputs are 1.0 (Req 9.3).
/// - Otherwise, outputs are in `[0.0, 1.0]` (Req 1.2).
pub fn normalize_field(raw: &[f32], out: &mut Vec<f32>) -> f32 {
    let max_val = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Guard: empty slice or near-zero max → all zeros.
    let max_val = if raw.is_empty() { 0.0 } else { max_val };
    let divisor = if max_val.abs() < 1e-9 { 1.0 } else { max_val };

    out.clear();
    out.reserve(raw.len());
    for &v in raw {
        out.push(if max_val.abs() < 1e-9 { 0.0 } else { v / divisor });
    }
    max_val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_buffer_normalizes_to_zero() {
        let raw = vec![0.0; 5];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < 1e-9);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn identical_nonzero_normalizes_to_one() {
        let raw = vec![3.5, 3.5, 3.5, 3.5];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!((max - 3.5).abs() < f32::EPSILON);
        assert!(out.iter().all(|&v| (v - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn basic_normalization() {
        let raw = vec![0.0, 5.0, 10.0];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!((max - 10.0).abs() < f32::EPSILON);
        assert!((out[0] - 0.0).abs() < f32::EPSILON);
        assert!((out[1] - 0.5).abs() < f32::EPSILON);
        assert!((out[2] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_buffer() {
        let raw: Vec<f32> = vec![];
        let mut out = Vec::new();
        let max = normalize_field(&raw, &mut out);
        assert!(max.abs() < f32::EPSILON);
        assert!(out.is_empty());
    }

    #[test]
    fn reuses_output_buffer() {
        let raw = vec![2.0, 4.0];
        let mut out = vec![99.0; 10]; // pre-filled with junk
        normalize_field(&raw, &mut out);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < f32::EPSILON);
        assert!((out[1] - 1.0).abs() < f32::EPSILON);
    }
}
