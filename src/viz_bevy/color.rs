// WARM PATH: Color mapping for Bevy GPU texture visualization.
// Maps normalized field values [0.0, 1.0] to RGBA [u8; 4].
// Zero allocations — pure arithmetic only.

/// Linearly interpolate between two `u8` values.
///
/// `t` is clamped internally by the callers to [0.0, 1.0] segment fractions,
/// so no explicit clamp here — the rounding + cast handles edge precision.
#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let result = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
    result.round().clamp(0.0, 255.0) as u8
}

/// Map a normalized value to RGBA using a blue→cyan→green→yellow→red gradient.
///
/// Gradient stops (Req 4.1):
/// - 0.00 → Blue   (0, 0, 255)
/// - 0.25 → Cyan   (0, 255, 255)
/// - 0.50 → Green  (0, 255, 0)
/// - 0.75 → Yellow (255, 255, 0)
/// - 1.00 → Red    (255, 0, 0)
///
/// Input is clamped to [0.0, 1.0] before mapping (Req 4.3).
/// Alpha is always 255 (Req 4.4).
pub fn heat_color_rgba(normalized: f32) -> [u8; 4] {
    let v = normalized.clamp(0.0, 1.0);

    let (r, g, b) = if v < 0.25 {
        let t = v / 0.25;
        (0, lerp_u8(0, 255, t), 255)
    } else if v < 0.50 {
        let t = (v - 0.25) / 0.25;
        (0, 255, lerp_u8(255, 0, t))
    } else if v < 0.75 {
        let t = (v - 0.50) / 0.25;
        (lerp_u8(0, 255, t), 255, 0)
    } else {
        let t = (v - 0.75) / 0.25;
        (255, lerp_u8(255, 0, t), 0)
    };

    [r, g, b, 255]
}

/// Map a normalized value to RGBA using a dark-green→bright-green gradient.
///
/// Green channel interpolates from 30 at 0.0 to 255 at 1.0 (Req 4.2).
/// Red and blue channels are always 0.
/// Input is clamped to [0.0, 1.0] before mapping (Req 4.3).
/// Alpha is always 255 (Req 4.4).
pub fn chemical_color_rgba(normalized: f32) -> [u8; 4] {
    let v = normalized.clamp(0.0, 1.0);
    let g = lerp_u8(30, 255, v);
    [0, g, 0, 255]
}

/// Write color-mapped RGBA data into a pre-allocated pixel buffer.
///
/// For each element in `norm_buffer`, applies `color_fn` and writes the
/// resulting 4 bytes at the corresponding offset in `pixel_buffer`.
///
/// # Panics
///
/// Panics (debug-only) if `pixel_buffer.len() < norm_buffer.len() * 4`.
///
/// Zero allocations — indexes directly into the pre-allocated buffer (Req 5.2, 9.3).
pub fn fill_pixel_buffer(
    norm_buffer: &[f32],
    pixel_buffer: &mut [u8],
    color_fn: fn(f32) -> [u8; 4],
) {
    debug_assert!(
        pixel_buffer.len() >= norm_buffer.len() * 4,
        "pixel buffer too small: {} < {}",
        pixel_buffer.len(),
        norm_buffer.len() * 4
    );

    for (i, &val) in norm_buffer.iter().enumerate() {
        let rgba = color_fn(val);
        let offset = i * 4;
        pixel_buffer[offset] = rgba[0];
        pixel_buffer[offset + 1] = rgba[1];
        pixel_buffer[offset + 2] = rgba[2];
        pixel_buffer[offset + 3] = rgba[3];
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_gradient_stops() {
        assert_eq!(heat_color_rgba(0.0), [0, 0, 255, 255]);
        assert_eq!(heat_color_rgba(0.25), [0, 255, 255, 255]);
        assert_eq!(heat_color_rgba(0.50), [0, 255, 0, 255]);
        assert_eq!(heat_color_rgba(0.75), [255, 255, 0, 255]);
        assert_eq!(heat_color_rgba(1.0), [255, 0, 0, 255]);
    }

    #[test]
    fn heat_clamps_out_of_range() {
        assert_eq!(heat_color_rgba(-1.0), heat_color_rgba(0.0));
        assert_eq!(heat_color_rgba(2.0), heat_color_rgba(1.0));
    }

    #[test]
    fn chemical_endpoints() {
        assert_eq!(chemical_color_rgba(0.0), [0, 30, 0, 255]);
        assert_eq!(chemical_color_rgba(1.0), [0, 255, 0, 255]);
    }

    #[test]
    fn chemical_clamps_out_of_range() {
        assert_eq!(chemical_color_rgba(-1.0), chemical_color_rgba(0.0));
        assert_eq!(chemical_color_rgba(2.0), chemical_color_rgba(1.0));
    }

    #[test]
    fn fill_pixel_buffer_basic() {
        let norm = [0.0_f32, 0.5, 1.0];
        let mut pixels = [0_u8; 12];
        fill_pixel_buffer(&norm, &mut pixels, heat_color_rgba);

        // Cell 0: blue
        assert_eq!(&pixels[0..4], &[0, 0, 255, 255]);
        // Cell 1: green
        assert_eq!(&pixels[4..8], &[0, 255, 0, 255]);
        // Cell 2: red
        assert_eq!(&pixels[8..12], &[255, 0, 0, 255]);
    }

    #[test]
    fn fill_pixel_buffer_empty() {
        let norm: [f32; 0] = [];
        let mut pixels: [u8; 0] = [];
        fill_pixel_buffer(&norm, &mut pixels, heat_color_rgba);
        // No panic, no-op.
    }

    #[test]
    fn alpha_always_255() {
        // Spot-check several values across the range.
        for i in 0..=20 {
            let v = i as f32 / 20.0;
            assert_eq!(heat_color_rgba(v)[3], 255);
            assert_eq!(chemical_color_rgba(v)[3], 255);
        }
    }
}
