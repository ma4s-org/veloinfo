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
/// Positive slope (uphill) increases cost, negative (downhill) decreases it slightly
/// Based on research on cycling energy expenditure
/// Returns a value >= 0.1 (never negative or zero)
pub fn get_slope_cost_multiplier(slope_percentage: f64) -> f64 {
    // Flat terrain (±2% slope) has minimal impact
    if slope_percentage.abs() <= 2.0 {
        return 1.0;
    }

    let multiplier = if slope_percentage > 0.0 {
        // For uphill slopes, cost increases exponentially
        // Using empirical formula: cost increases ~10% per 1% slope
        // For steep slopes (>10%), cost increases more dramatically
        if slope_percentage <= 5.0 {
            1.0 + (slope_percentage * 0.12)
        } else if slope_percentage <= 10.0 {
            1.0 + (5.0 * 0.12) + ((slope_percentage - 5.0) * 0.20)
        } else {
            // Very steep slopes: 1 + 0.6 + (slope-10)*0.3 = 1.6 + (slope-10)*0.3
            1.6 + ((slope_percentage - 10.0) * 0.30)
        }
    } else {
        // Downhill slopes are easier but still require control
        // -2% to -5% slope: slight decrease in cost (10% reduction)
        // -5% to -10% slope: moderate decrease (20% reduction)
        // Below -10%: be cautious (less than 30% reduction)
        if slope_percentage >= -5.0 {
            1.0 + (slope_percentage * 0.05) // small bonus for downhill
        } else if slope_percentage >= -10.0 {
            0.90 + ((slope_percentage + 5.0) * 0.02)
        } else {
            0.80 + ((slope_percentage + 10.0) * 0.01)
        }
    };

    // Ensure multiplier is never negative or zero (minimum 0.1 for very steep descents)
    multiplier.max(0.1)
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
