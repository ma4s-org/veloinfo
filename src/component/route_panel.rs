use crate::utils::cost::{get_h_moyen, get_h_rapid};
use askama::Template;
use axum::{
    debug_handler,
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    response::Response,
};
use serde::Deserialize;
use tokio::join;

use crate::{
    db::edge::{Edge, Point},
    VeloinfoState,
};

#[derive(Template)]
#[template(path = "route_panel.html", escape = "none")]
pub struct RoutePanel {
    pub coordinates: String,
    pub error: String,
    pub ferry: bool,
}

impl RoutePanel {
    pub fn error(error: String) -> RoutePanel {
        RoutePanel {
            coordinates: "[]".to_string(),
            error,
            ferry: false,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RouteParams {
    allow_ferry: Option<bool>,
}

#[debug_handler]
pub async fn route(
    ws: WebSocketUpgrade,
    State(state): State<VeloinfoState>,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
    route_params: Query<RouteParams>,
) -> Response {
    ws.on_upgrade(async move |mut socket| {
        let allow_ferry = route_params.allow_ferry.unwrap_or(true);
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
        let (mut points, mut points_rapide) = join!(
            Edge::a_star_bidirectional(
                start.node_id,
                end.node_id,
                get_h_moyen(),
                &state.conn,
                Some(&mut socket),
                allow_ferry,
            ),
            Edge::a_star_bidirectional(
                start.node_id,
                end.node_id,
                get_h_rapid(),
                &state.conn,
                None,
                allow_ferry,
            )
        );

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
                ferry: false,
            },
        );
        points.push(Point {
            lng: end_lng,
            lat: end_lat,
            length: 0.0,
            way_id: 0,
            node_id: 0,
            ferry: false,
        });
        points_rapide.insert(
            0,
            Point {
                lng: start_lng,
                lat: start_lat,
                length: 0.0,
                way_id: 0,
                node_id: 0,
                ferry: false,
            },
        );
        points_rapide.push(Point {
            lng: end_lng,
            lat: end_lat,
            length: 0.0,
            way_id: 0,
            node_id: 0,
            ferry: false,
        });

        let edges_coordinate_safe: Vec<(f64, f64)> =
            points.iter().map(|point| (point.lng, point.lat)).collect();
        let edges_coordinate_fast: Vec<(f64, f64)> = points_rapide
            .iter()
            .map(|point| (point.lng, point.lat))
            .collect();
        let panel = RoutePanel {
            coordinates: serde_json::to_string(&[edges_coordinate_safe, edges_coordinate_fast])
                .unwrap_or_else(|e| format!("Error serializing edges: {}", e)),
            error: "".to_string(),
            ferry: points.iter().any(|point| point.ferry),
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
    Path((route, start_lng, start_lat, end_lng, end_lat)): Path<(
        String,
        f64,
        f64,
        f64,
        f64,
    )>,
    route_params: Query<RouteParams>,
) -> Response {
    ws.on_upgrade(async move |mut socket| {
        let allow_ferry = route_params.allow_ferry.unwrap_or(true);
        let state = state.clone();
        let start = match Edge::find_closest_node(&start_lng, &start_lat, &state.conn).await {
            Ok(start) => start,
            Err(e) => {
                let error_panel = format!(
                    "Error while fetching start node for {}, {}: {}",
                    start_lng, start_lat, e
                );
                socket.send(error_panel.into()).await.unwrap();
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
                socket.send(error_panel.into()).await.unwrap();
                return;
            }
        };
        let mut points = match route.as_str() {
            "safe" => Edge::a_star_bidirectional(
                start.node_id,
                end.node_id,
                get_h_moyen(),
                &state.conn,
                None,
                allow_ferry,
            ),
            "fast" => Edge::a_star_bidirectional(
                start.node_id,
                end.node_id,
                get_h_rapid(),
                &state.conn,
                None,
                allow_ferry,
            ),
            _ => {
                let error_panel = format!("Invalid route type: {}", route);
                socket.send(error_panel.into()).await.unwrap();
                return;
            }
        }
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
                ferry: false,
            },
        );
        points.push(Point {
            lng: end_lng,
            lat: end_lat,
            length: 0.0,
            way_id: 0,
            node_id: 0,
            ferry: false,
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
        socket
            .send(format!("{{\"coordinates\": {}}}", json).into())
            .await
            .unwrap();
    })
}
