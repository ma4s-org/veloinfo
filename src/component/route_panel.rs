use crate::utils::h::get_h_moyen;
use askama::Template;
use axum::extract::{Path, State};
use axum_macros::debug_handler;

use crate::{
    db::edge::{Edge, Point},
    VeloinfoState,
};

#[derive(Template)]
#[template(path = "route_panel.html", escape = "none")]
pub struct RoutePanel {
    pub coordinates: String,
    pub error: String,
}

impl RoutePanel {
    pub fn error(error: String) -> RoutePanel {
        RoutePanel {
            coordinates: "[]".to_string(),
            error,
        }
    }
}

#[debug_handler]
pub async fn route(
    State(state): State<VeloinfoState>,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
) -> RoutePanel {
    let start = match Edge::find_closest_node(&start_lng, &start_lat, &state.conn).await {
        Ok(start) => start,
        Err(e) => {
            return RoutePanel::error(format!(
                "Error while fetching start node for {}, {}: {}",
                start_lat, start_lng, e
            ));
        }
    };
    let end = match Edge::find_closest_node(&end_lng, &end_lat, &state.conn).await {
        Ok(end) => end,
        Err(e) => {
            return RoutePanel::error(format!(
                "Error while fetching end node for {}, {}: {}",
                end_lat, end_lng, e
            ));
        }
    };
    let mut points =
        Edge::a_star_route(start.node_id, end.node_id, get_h_moyen(), &state.conn).await;

    if let 0 = points.len() {
        return RoutePanel::error(format!("No route found from {start:?} to {end:?}"));
    };

    points.insert(
        0,
        Point {
            lng: start_lng,
            lat: start_lat,
            length: 0.0,
            way_id: 0,
            node_id: 0,
        },
    );
    points.push(Point {
        lng: end_lng,
        lat: end_lat,
        length: 0.0,
        way_id: 0,
        node_id: 0,
    });
    let edges_coordinate: Vec<(f64, f64)> =
        points.iter().map(|point| (point.lng, point.lat)).collect();
    RoutePanel {
        coordinates: match serde_json::to_string(&edges_coordinate) {
            Ok(edges_coordinate) => edges_coordinate,
            Err(e) => {
                return RoutePanel::error(format!("Error while serializing edges: {}", e));
            }
        },
        error: "".to_string(),
    }
}
