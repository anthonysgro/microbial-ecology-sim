// COLD PATH: Stats computation and formatting for terminal visualization.

use super::OverlayMode;

/// Aggregated statistics for a single field buffer snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldStats {
    pub total: f32,
    pub min: f32,
    pub max: f32,
    pub center: f32,
}

/// Compute aggregate statistics from a raw field buffer.
///
/// `center_index` is the flat index of the grid's center cell.
/// If the buffer is empty, all fields are 0.0.
/// If `center_index` is out of bounds, `center` is 0.0.
pub fn compute_stats(buffer: &[f32], center_index: usize) -> FieldStats {
    if buffer.is_empty() {
        return FieldStats {
            total: 0.0,
            min: 0.0,
            max: 0.0,
            center: 0.0,
        };
    }

    let mut total: f32 = 0.0;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;

    for &v in buffer {
        total += v;
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }

    let center = buffer.get(center_index).copied().unwrap_or(0.0);

    FieldStats {
        total,
        min,
        max,
        center,
    }
}

/// Format the stats bar string for display below the grid.
///
/// Contains: tick number, overlay mode label, total, min, max, center.
/// (Req 5.4, 6.5, 7.1)
pub fn format_stats_bar(tick: u64, overlay: &OverlayMode, stats: &FieldStats) -> String {
    format!(
        "tick:{} [{}] total:{:.2} min:{:.2} max:{:.2} center:{:.2}",
        tick,
        overlay.label(),
        stats.total,
        stats.min,
        stats.max,
        stats.center,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_stats_basic() {
        let buf = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_stats(&buf, 2);
        assert!((stats.total - 15.0).abs() < f32::EPSILON);
        assert!((stats.min - 1.0).abs() < f32::EPSILON);
        assert!((stats.max - 5.0).abs() < f32::EPSILON);
        assert!((stats.center - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_stats_empty_buffer() {
        let stats = compute_stats(&[], 0);
        assert_eq!(stats.total, 0.0);
        assert_eq!(stats.min, 0.0);
        assert_eq!(stats.max, 0.0);
        assert_eq!(stats.center, 0.0);
    }

    #[test]
    fn compute_stats_center_out_of_bounds() {
        let buf = vec![10.0, 20.0];
        let stats = compute_stats(&buf, 99);
        assert_eq!(stats.center, 0.0);
        assert!((stats.total - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn format_stats_bar_contains_all_fields() {
        let stats = FieldStats {
            total: 42.5,
            min: 0.1,
            max: 9.9,
            center: 3.14,
        };
        let bar = format_stats_bar(100, &OverlayMode::Heat, &stats);
        assert!(bar.contains("100"), "should contain tick number");
        assert!(bar.contains("Heat"), "should contain overlay label");
        assert!(bar.contains("42.50"), "should contain total");
        assert!(bar.contains("0.10"), "should contain min");
        assert!(bar.contains("9.90"), "should contain max");
        assert!(bar.contains("3.14"), "should contain center");
    }

    #[test]
    fn format_stats_bar_chemical_overlay() {
        let stats = FieldStats {
            total: 0.0,
            min: 0.0,
            max: 0.0,
            center: 0.0,
        };
        let bar = format_stats_bar(0, &OverlayMode::Chemical(2), &stats);
        assert!(bar.contains("0"), "should contain tick 0");
        assert!(bar.contains("Chemical 2"), "should contain chemical label");
    }

    #[test]
    fn format_stats_bar_moisture_overlay() {
        let stats = FieldStats {
            total: 1.0,
            min: 0.5,
            max: 1.0,
            center: 0.75,
        };
        let bar = format_stats_bar(999, &OverlayMode::Moisture, &stats);
        assert!(bar.contains("999"));
        assert!(bar.contains("Moisture"));
    }
}
