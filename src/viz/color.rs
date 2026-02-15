// COLD PATH: Color mapping for terminal visualization.
// Maps normalized field values [0.0, 1.0] to crossterm ANSI colors.

use crossterm::style::Color;

/// Linearly interpolate between two u8 values.
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let result = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
    result.round().clamp(0.0, 255.0) as u8
}

/// Map a normalized value [0.0, 1.0] to a foreground color for the heat overlay.
///
/// Interpolates across 5 stops:
/// - [0.00, 0.25): Blue (0,0,255) → Cyan (0,255,255)
/// - [0.25, 0.50): Cyan (0,255,255) → Green (0,255,0)
/// - [0.50, 0.75): Green (0,255,0) → Yellow (255,255,0)
/// - [0.75, 1.00]: Yellow (255,255,0) → Red (255,0,0)
///
/// Requirements: 2.1 (blue-to-red gradient), 2.2 (deterministic mapping)
pub fn heat_color(normalized: f32) -> Color {
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

    Color::Rgb { r, g, b }
}

/// Map a normalized value [0.0, 1.0] to a background color for the moisture overlay.
///
/// Single-hue blue gradient: dark blue (0,0,30) at 0.0 → bright blue (0,0,255) at 1.0.
/// Red and green channels stay at zero to maintain blue-channel dominance (Req 3.2).
///
/// Requirements: 3.1 (background color shading), 3.2 (single-hue blue palette)
pub fn moisture_bg_color(normalized: f32) -> Color {
    let v = normalized.clamp(0.0, 1.0);
    let b = lerp_u8(30, 255, v);
    Color::Rgb { r: 0, g: 0, b }
}

/// Map a normalized value [0.0, 1.0] to a foreground color for the chemical overlay.
///
/// Green-scale gradient: dark green (0,30,0) at 0.0 → bright green (0,255,0) at 1.0.
pub fn chemical_color(normalized: f32) -> Color {
    let v = normalized.clamp(0.0, 1.0);
    let g = lerp_u8(30, 255, v);
    Color::Rgb { r: 0, g, b: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::Rgb { r, g, b } => (r, g, b),
            _ => panic!("expected Color::Rgb"),
        }
    }

    #[test]
    fn heat_color_endpoints() {
        // At 0.0: pure blue
        let (r, g, b) = extract_rgb(heat_color(0.0));
        assert_eq!((r, g, b), (0, 0, 255));

        // At 1.0: pure red
        let (r, g, b) = extract_rgb(heat_color(1.0));
        assert_eq!((r, g, b), (255, 0, 0));
    }

    #[test]
    fn heat_color_midpoints() {
        // At 0.25: cyan
        let (r, g, b) = extract_rgb(heat_color(0.25));
        assert_eq!((r, g, b), (0, 255, 255));

        // At 0.50: green
        let (r, g, b) = extract_rgb(heat_color(0.50));
        assert_eq!((r, g, b), (0, 255, 0));

        // At 0.75: yellow
        let (r, g, b) = extract_rgb(heat_color(0.75));
        assert_eq!((r, g, b), (255, 255, 0));
    }

    #[test]
    fn heat_color_clamps_out_of_range() {
        let (r, g, b) = extract_rgb(heat_color(-0.5));
        assert_eq!((r, g, b), (0, 0, 255));

        let (r, g, b) = extract_rgb(heat_color(1.5));
        assert_eq!((r, g, b), (255, 0, 0));
    }

    #[test]
    fn moisture_blue_dominance() {
        for i in 0..=10 {
            let v = i as f32 / 10.0;
            let (r, g, b) = extract_rgb(moisture_bg_color(v));
            assert!(b >= r, "blue {b} < red {r} at v={v}");
            assert!(b >= g, "blue {b} < green {g} at v={v}");
        }
    }

    #[test]
    fn moisture_gradient_range() {
        let (_, _, b_lo) = extract_rgb(moisture_bg_color(0.0));
        let (_, _, b_hi) = extract_rgb(moisture_bg_color(1.0));
        assert_eq!(b_lo, 30);
        assert_eq!(b_hi, 255);
    }

    #[test]
    fn chemical_green_gradient() {
        let (r0, g0, b0) = extract_rgb(chemical_color(0.0));
        let (r1, g1, b1) = extract_rgb(chemical_color(1.0));
        assert_eq!((r0, b0), (0, 0));
        assert_eq!((r1, b1), (0, 0));
        assert_eq!(g0, 30);
        assert_eq!(g1, 255);
    }
}
