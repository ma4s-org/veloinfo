/// Elevation utilities for SRTM integration
/// Calculates slope-based cost multipliers for the routing algorithm
use crate::db::edge::EdgePoint;

/// Calculate the slope percentage between two elevations over a distance
pub fn calculate_slope_percentage(
    elevation_start: f64,
    elevation_end: f64,
    distance_meters: f64,
) -> f64 {
    if distance_meters <= 0.0 {
        return 0.0;
    }
    ((elevation_end - elevation_start) / distance_meters) * 100.0
}

/// Get a cost multiplier based on slope
/// Positive slope (uphill) increases cost, negative (downhill) decreases it
/// Returns a value >= 0 (never negative)
pub fn get_slope_cost_multiplier(slope_percentage: f64) -> f64 {
    if slope_percentage >= 0.0 {
        // Uphill: cost increases linearly
        1.0 + 0.1 * slope_percentage
    } else {
        // Downhill: cost decreases linearly, but never below 0
        1. - 0.05 * slope_percentage.abs().min(6.0)
    }
}

/// Calculate slope cost multiplier for an EdgePoint
/// Returns 1.0 if elevation data is not available
pub fn get_edge_slope_cost_multiplier(edge: &EdgePoint) -> f64 {
    match (edge.elevation_start, edge.elevation_end) {
        (Some(elev_start), Some(elev_end)) => {
            let slope_percentage = calculate_slope_percentage(elev_start, elev_end, edge.length);
            get_slope_cost_multiplier(slope_percentage)
        }
        _ => 1.0, // No elevation data, no slope penalty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_slope() {
        assert!((calculate_slope_percentage(100.0, 101.0, 50.0) - 2.0).abs() < 0.01);
        assert!((calculate_slope_percentage(100.0, 100.0, 100.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_slope_cost_multiplier_flat() {
        assert!((get_slope_cost_multiplier(0.0) - 1.0).abs() < 0.01);
        assert!((get_slope_cost_multiplier(1.5) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_slope_cost_multiplier_uphill() {
        let cost_3_percent = get_slope_cost_multiplier(3.0);
        let cost_6_percent = get_slope_cost_multiplier(6.0);
        let cost_12_percent = get_slope_cost_multiplier(12.0);

        // Steeper slopes should have higher cost
        assert!(cost_3_percent < cost_6_percent);
        assert!(cost_6_percent < cost_12_percent);
    }

    #[test]
    fn test_slope_cost_multiplier_downhill() {
        let cost_minus_3 = get_slope_cost_multiplier(-3.0);
        let cost_minus_8 = get_slope_cost_multiplier(-8.0);

        // Downhill should be easier (lower cost)
        assert!(cost_minus_3 < 1.0);
        assert!(cost_minus_8 < cost_minus_3);
        assert!(cost_minus_8 > 0.7); // but not too easy
    }
}
