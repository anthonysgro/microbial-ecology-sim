// COLD PATH: Glyph mapping for terminal visualization.
// Maps normalized field values [0.0, 1.0] to ASCII characters for display.

/// Map a normalized value in [0.0, 1.0] to a display character.
///
/// Threshold sequence:
/// - `' '` (space) for values below 0.01
/// - `'.'` for [0.01, 0.25)
/// - `':'` for [0.25, 0.50)
/// - `'*'` for [0.50, 0.75)
/// - `'#'` for [0.75, 1.0]
pub fn value_to_glyph(normalized: f32) -> char {
    if normalized < 0.01 {
        ' '
    } else if normalized < 0.25 {
        '.'
    } else if normalized < 0.50 {
        ':'
    } else if normalized < 0.75 {
        '*'
    } else {
        '#'
    }
}

/// Return the threshold range `(lo, hi)` for a given glyph character.
///
/// The range is `[lo, hi)` for all glyphs except `'#'` which covers `[0.75, 1.0]`.
/// Returns `None` for unrecognized characters.
pub fn glyph_to_range(ch: char) -> Option<(f32, f32)> {
    match ch {
        ' ' => Some((0.0, 0.01)),
        '.' => Some((0.01, 0.25)),
        ':' => Some((0.25, 0.50)),
        '*' => Some((0.50, 0.75)),
        '#' => Some((0.75, 1.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_boundary_values() {
        assert_eq!(value_to_glyph(0.0), ' ');
        assert_eq!(value_to_glyph(0.005), ' ');
        assert_eq!(value_to_glyph(0.01), '.');
        assert_eq!(value_to_glyph(0.24), '.');
        assert_eq!(value_to_glyph(0.25), ':');
        assert_eq!(value_to_glyph(0.49), ':');
        assert_eq!(value_to_glyph(0.50), '*');
        assert_eq!(value_to_glyph(0.74), '*');
        assert_eq!(value_to_glyph(0.75), '#');
        assert_eq!(value_to_glyph(1.0), '#');
    }

    #[test]
    fn glyph_to_range_known_glyphs() {
        assert_eq!(glyph_to_range(' '), Some((0.0, 0.01)));
        assert_eq!(glyph_to_range('.'), Some((0.01, 0.25)));
        assert_eq!(glyph_to_range(':'), Some((0.25, 0.50)));
        assert_eq!(glyph_to_range('*'), Some((0.50, 0.75)));
        assert_eq!(glyph_to_range('#'), Some((0.75, 1.0)));
    }

    #[test]
    fn glyph_to_range_unknown_returns_none() {
        assert_eq!(glyph_to_range('x'), None);
        assert_eq!(glyph_to_range('0'), None);
    }

    #[test]
    fn round_trip_consistency() {
        // For a set of representative values, the glyph's range must contain the value.
        let test_values = [0.0, 0.005, 0.01, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 1.0];
        for &v in &test_values {
            let glyph = value_to_glyph(v);
            let (lo, hi) = glyph_to_range(glyph).expect("known glyph");
            assert!(
                v >= lo && (v < hi || (glyph == '#' && v <= hi)),
                "value {v} mapped to '{glyph}' but range [{lo}, {hi}) doesn't contain it"
            );
        }
    }
}
