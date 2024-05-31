use askama::Template;
use axum::extract::{Path, State};

use crate::{
    db::edge::{Edge, Point},
    VeloinfoState,
};

#[derive(Template)]
#[template(path = "route_panel.html", escape = "none")]
pub struct RoutePanel {
    pub coordonates: String,
    route: String,
    pub total_length: f64,
    pub error: String,
}

pub async fn route(
    State(state): State<VeloinfoState>,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
) -> RoutePanel {
    let start = match Edge::find_closest_node(&start_lng, &start_lat, &state.conn).await {
        Ok(start) => start,
        Err(e) => {
            return RoutePanel {
                coordonates: "[]".to_string(),
                route: "[]".to_string(),
                total_length: 0.0,
                error: format!("Error while fetching start node: {}", e),
            };
        }
    };
    let end = match Edge::find_closest_node(&end_lng, &end_lat, &state.conn).await {
        Ok(end) => end,
        Err(e) => {
            return RoutePanel {
                coordonates: "[]".to_string(),
                route: "[]".to_string(),
                total_length: 0.0,
                error: format!("Error while fetching end node: {}", e),
            };
        }
    };
    let edges = Edge::route(&start, &end, &state.conn).await;

    // the route is the edges that are not the same as the previous one
    let mut route: Vec<(Edge, f64)> = vec![(edges[0].clone(), 0.)];
    let mut distance: f64 = 0.;
    edges.iter().for_each(|edge| {
        distance += edge.length;
        match route.last() {
            Some(last) => {
                if last.0.name != edge.name && edge.name != None {
                    route.push((edge.clone(), distance));
                }
            }
            None => {}
        }
    });
    if let 0 = edges.len() {
        println!("No route found");
        return RoutePanel {
            coordonates: "[]".to_string(),
            route: "[]".to_string(),
            total_length: 0.0,
            error: format!("No route found from {start:?} to {end:?}"),
        };
    };

    let mut points: Vec<Point> = edges
        .iter()
        .map(|edge| Point {
            x: edge.x1,
            y: edge.y1,
            length: edge.length,
            way_id: edge.way_id,
            node_id: edge.source,
        })
        .collect();
    points.insert(
        0,
        Point {
            x: start_lng,
            y: start_lat,
            length: 0.0,
            way_id: 0,
            node_id: 0,
        },
    );
    points.push(Point {
        x: end_lng,
        y: end_lat,
        length: 0.0,
        way_id: 0,
        node_id: 0,
    });
    let edges_coordinate: Vec<(f64, f64)> = points.iter().map(|point| (point.x, point.y)).collect();
    let total_length: f64 = points.iter().map(|point| point.length).sum();
    let coordonates = match serde_json::to_string(&edges_coordinate) {
        Ok(edges_coordinate) => edges_coordinate,
        Err(e) => {
            return RoutePanel {
                coordonates: "[]".to_string(),
                route: "[]".to_string(),
                total_length: 0.0,
                error: format!("Error while serializing edges: {}", e),
            };
        }
    };
    RoutePanel {
        coordonates,
        route: match serde_json::to_string(&route) {
            Ok(route) => route,
            Err(e) => {
                return RoutePanel {
                    coordonates: "[]".to_string(),
                    route: "[]".to_string(),
                    total_length: 0.0,
                    error: format!("Error while serializing route: {}", e),
                };
            }
        },
        total_length: (total_length / 10.0).round() / 100.0,
        error: "".to_string(),
    }
}
