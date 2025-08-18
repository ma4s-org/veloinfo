use crate::utils::h::get_h_moyen;
use askama::Template;
use axum::{
    debug_handler,
    extract::{ws::WebSocketUpgrade, Path, State},
    response::Response,
};

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
    ws: WebSocketUpgrade,
    State(state): State<VeloinfoState>,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
) -> Response {
    ws.on_upgrade(async move |mut socket| {
        let state = state.clone();
        let start = match Edge::find_closest_node(&start_lng, &start_lat, &state.conn).await {
            Ok(start) => start,
            Err(e) => {
                let error_panel = RoutePanel::error(format!(
                    "Error while fetching start node for {}, {}: {}",
                    start_lat, start_lng, e
                ));
                socket
                    .send(error_panel.render().unwrap().into())
                    .await
                    .unwrap();
                return;
            }
        };
        let end = match Edge::find_closest_node(&end_lng, &end_lat, &state.conn).await {
            Ok(end) => end,
            Err(e) => {
                let error_panel = RoutePanel::error(format!(
                    "Error while fetching start node for {}, {}: {}",
                    start_lat, start_lng, e
                ));
                socket
                    .send(error_panel.render().unwrap().into())
                    .await
                    .unwrap();
                return;
            }
        };
        let mut points = Edge::a_star_route(
            start.node_id,
            end.node_id,
            get_h_moyen(),
            &state.conn,
            Some(&mut socket),
        )
        .await;

        if points.len() == 0 {
            let error_panel =
                RoutePanel::error(format!("No route found from {start:?} to {end:?}"));
            socket
                .send(error_panel.render().unwrap().into())
                .await
                .unwrap();
            return;
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
        let panel = RoutePanel {
            coordinates: serde_json::to_string(&edges_coordinate)
                .unwrap_or_else(|e| format!("Error serializing edges: {}", e)),
            error: "".to_string(),
        }
        .render()
        .unwrap();
        socket.send(panel.into()).await.unwrap();
    })
}

#[debug_handler]
pub async fn recalculate_route(
    ws: WebSocketUpgrade,
    State(state): State<VeloinfoState>,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
) -> Response {
    ws.on_upgrade(async move |mut socket| {
        let state = state.clone();
        let start = match Edge::find_closest_node(&start_lng, &start_lat, &state.conn).await {
            Ok(start) => start,
            Err(e) => {
                let error_panel = format!(
                    "Error while fetching start node for {}, {}: {}",
                    start_lng, start_lat, e
                );
                socket
                    .send(error_panel.into())
                    .await
                    .unwrap();
                return;
            }
        };
        let end = match Edge::find_closest_node(&end_lng, &end_lat, &state.conn).await {
            Ok(end) => end,
            Err(e) => {
                let error_panel = format!(
                    "Error while fetching start node for {}, {}: {}",
                    start_lng, start_lat, e
                );
                socket
                    .send(error_panel.into())
                    .await
                    .unwrap();
                return;
            }
        };
        let mut points = Edge::a_star_route(
            start.node_id,
            end.node_id,
            get_h_moyen(),
            &state.conn,
            Some(&mut socket),
        )
        .await;

        if points.len() == 0 {
            let error_panel =
                RoutePanel::error(format!("No route found from {start:?} to {end:?}"));
            socket
                .send(error_panel.render().unwrap().into())
                .await
                .unwrap();
            return;
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
        let json = match serde_json::to_string(&edges_coordinate) {
            Ok(json) => json,
            Err(e) => {
                socket
                    .send(format!("Error serializing edges: {}", e).into())
                    .await
                    .unwrap();
                return;
            }
        };
        socket.send(format!("{{\"coordinates\": {}}}", json).into()).await.unwrap();
    })
}
