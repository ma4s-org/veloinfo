use askama::Template;
use axum::{extract::State, Form};

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

pub async fn follow(State(state): State<VeloinfoState>, Form(values): Form<Values>) -> FollowPanel {
    println!("routes {:?}", values.route);
    FollowPanel {
        route_json: "[]".to_string(),
        total_length: 0.0,
        error: "Error while fetching start node: {}".to_string(),
    }
}
