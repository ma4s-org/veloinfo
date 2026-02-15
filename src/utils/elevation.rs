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
pub fn get_slope_cost(slope_percentage: f64) -> f64 {
    if slope_percentage >= 0.0 {
        return sigmoid_transition(slope_percentage);
    } else {
        sigmoid_transition(slope_percentage.abs()) * 0.2
    }
}

/// Calculate slope cost for an EdgePoint
/// Returns 0.0 if elevation data is not available
pub fn get_edge_slope_cost(edge: &EdgePoint) -> f64 {
    match (edge.elevation_start, edge.elevation_end) {
        (Some(elev_start), Some(elev_end)) => {
            let slope_percentage = calculate_slope_percentage(elev_start, elev_end, edge.length);
            get_slope_cost(slope_percentage)
        }
        _ => 0.0, // No elevation data, no slope penalty
    }
}

fn sigmoid_transition(x: f64) -> f64 {
    let steepness = 0.4;
    let midpoint = 15.0;

    1.0 / (1.0 + (-steepness * (x - midpoint)).exp())
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
    fn test_slope_cost_multiplier_uphill() {
        let cost_3_percent = get_slope_cost(3.0);
        let cost_6_percent = get_slope_cost(6.0);
        let cost_12_percent = get_slope_cost(12.0);

        // Steeper slopes should have higher cost
        assert!(cost_3_percent < cost_6_percent);
        assert!(cost_6_percent < cost_12_percent);
    }

    #[test]
    fn test_slope_cost() {
        // Test with no elevation data
        let edge_no_elev = EdgePoint {
            id: 2,
            source: 2,
            target: 3,
            length: 100.0,
            elevation_start: None,
            elevation_end: None,
            ..Default::default()
        };
        let slope_cost_no_elev = get_edge_slope_cost(&edge_no_elev);
        assert_eq!(slope_cost_no_elev, 0.0);

        // test very small uphill slope 1%
        let edge_very_small_uphill = EdgePoint {
            id: 6,
            source: 6,
            target: 7,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(101.0),
            ..Default::default()
        };
        let slope_cost_very_small_uphill = get_edge_slope_cost(&edge_very_small_uphill);
        assert_eq!(slope_cost_very_small_uphill, 0.003684239899435986); // Should be a small cost for very small uphill

        // Test small uphill slope 5%
        let edge_small_uphill = EdgePoint {
            id: 5,
            source: 5,
            target: 6,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(105.0),
            ..Default::default()
        };
        let slope_cost_small_uphill = get_edge_slope_cost(&edge_small_uphill);
        assert_eq!(slope_cost_small_uphill, 0.01798620996209156); // Should be a moderate cost for small uphill

        // Test with 10% elevations and distance
        let edge = EdgePoint {
            id: 1,
            source: 1,
            target: 2,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(110.0),
            ..Default::default()
        };
        let slope_cost = get_edge_slope_cost(&edge);
        assert_eq!(slope_cost, 0.11920292202211755); // Based on the sigmoid function for a 10% slope

        // Test with a steep uphill slope 20%
        let edge_steep_uphill = EdgePoint {
            id: 4,
            source: 4,
            target: 5,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(120.0),
            ..Default::default()
        };
        let slope_cost_steep_uphill = get_edge_slope_cost(&edge_steep_uphill);
        assert_eq!(slope_cost_steep_uphill, 0.8807970779778823); // Should be a high cost for steep uphill

        // Test with a steep uphill slope 30%
        let edge_steep_uphill = EdgePoint {
            id: 4,
            source: 4,
            target: 5,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(130.0),
            ..Default::default()
        };
        let slope_cost_steep_uphill = get_edge_slope_cost(&edge_steep_uphill);
        assert_eq!(slope_cost_steep_uphill, 0.9975273768433653); // Should be a high cost for steep uphill
    }
}
