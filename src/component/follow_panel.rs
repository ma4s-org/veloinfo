use askama::Template;
use axum::extract::State;
use axum_macros::debug_handler;

use crate::VeloinfoState;

#[derive(Template)]
#[template(path = "follow_panel.html")]
pub struct FollowPanel {
    pub error: String,
}

#[debug_handler]
pub async fn follow(State(_state): State<VeloinfoState>) -> FollowPanel {
    FollowPanel {
        error: "".to_string(),
    }
}
