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

pub fn get_edge_slope_cost(edge: &EdgePoint) -> f64 {
    match (edge.elevation_start, edge.elevation_end) {
        (Some(elev_start), Some(elev_end)) => {
            let slope_percentage = calculate_slope_percentage(elev_start, elev_end, edge.length);
            sigmoid_transition(slope_percentage)
        }
        _ => 1.0, // No elevation data, no slope penalty
    }
}

fn sigmoid_transition(x: f64) -> f64 {
    // 1. Gérer l'extrême positif avec une croissance très lente (1/10e)
    if x > 20. {
        return (x / 3.0) - 1.0;
    };

    let steepness: f64 = 0.25;
    let midpoint: f64 = 7.0;
    let min_val: f64 = 1.0;

    // 2. Définir le plafond selon la direction
    let max_val: f64 = if x > 0. { 3. } else { 1.5 };

    // 3. Utiliser la valeur absolue de la pente
    let abs_x = x.abs();
    let sig_0 = 1.0 / (1.0 + (steepness * midpoint).exp());
    let sig_x = 1.0 / (1.0 + (-steepness * (abs_x - midpoint)).exp());

    min_val + (max_val - min_val) * (sig_x - sig_0) / (1.0 - sig_0)
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
        assert_eq!(slope_cost_no_elev, 1.0); // Multiplicateur neutre sans données d'élévation

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

        // Test with a steep uphill slope 30%
        let edge_very_steep_uphill = EdgePoint {
            id: 4,
            source: 4,
            target: 5,
            length: 100.0,
            elevation_start: Some(100.0),
            elevation_end: Some(130.0),
            ..Default::default()
        };
        let slope_cost_very_steep_uphill = get_edge_slope_cost(&edge_very_steep_uphill);

        // Test descente 5%
        let edge_downhill = EdgePoint {
            id: 7,
            source: 7,
            target: 8,
            length: 100.0,
            elevation_start: Some(105.0),
            elevation_end: Some(100.0),
            ..Default::default()
        };
        let slope_cost_downhill = get_edge_slope_cost(&edge_downhill);

        // Verify the ordering: higher slopes should have higher costs
        assert!(slope_cost_very_small_uphill < slope_cost_small_uphill);
        assert!(slope_cost_small_uphill < slope_cost);
        assert!(slope_cost < slope_cost_steep_uphill);
        assert!(slope_cost_steep_uphill < slope_cost_very_steep_uphill);

        // Descente should have lower cost than flat
        assert!(slope_cost_downhill < 1.0);
        assert!(slope_cost_downhill > 0.5);
    }
}
