use askama::Template;
use axum::extract::State;
use axum_macros::debug_handler;

use crate::{db::edge::Edge, VeloinfoState};

#[derive(Template)]
#[template(path = "follow_panel.html")]
pub struct FollowPanel {
    pub route_json: String,
    pub total_length: f64,
    pub error: String,
}

#[derive(serde::Deserialize)]
pub struct Values {
    pub route: Vec<(Edge, f64)>,
}

#[debug_handler]
pub async fn follow(State(_state): State<VeloinfoState>) -> FollowPanel {
    FollowPanel {
        route_json: "[]".to_string(),
        total_length: 0.0,
        error: "".to_string(),
    }
}
